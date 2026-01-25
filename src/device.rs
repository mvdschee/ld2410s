use crate::commands;
use crate::error::Error;
use crate::protocol::Protocol;
use crate::types::{FirmwareVersion, Packet};
use crate::uart::UartInterface;
use crate::{OutputMode, Timer};
use core::time::Duration;
use heapless::Vec;

#[derive(Debug, Clone, Copy)]
pub struct Config {
	pub command_timeout: Duration,
	pub config_mode_delay: Duration,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			command_timeout: Duration::from_millis(1000),
			config_mode_delay: Duration::from_millis(250),
		}
	}
}

/// Driver owning a UART interface and a Timer
pub struct LD2410S<U: UartInterface, T: Timer> {
	uart: U,
	timer: T,
	protocol: Protocol,
	output_mode: OutputMode,
	config: Config,
}

impl<U, T> LD2410S<U, T>
where
	U: UartInterface,
	T: Timer,
{
	/// Create a new LD2410S driver.
	pub fn new(uart: U, timer: T, output_mode: OutputMode) -> Self {
		Self {
			uart,
			timer,
			protocol: Protocol::new(),
			output_mode,
			config: Config::default(),
		}
	}

	pub fn set_config(&mut self, config: Config) {
		self.config = config;
	}

	/// Initialize the device by switching to the desired output mode.
	pub fn init(&mut self) -> Result<(), Error<U::Error>> {
		let cmd = commands::switch_output_frame(self.output_mode);
		self.run_config_transaction(&cmd)
	}

	pub fn set_distance_frequency(&mut self, hz: f32) -> Result<(), Error<U::Error>> {
		let val = (hz * 10.0) as u16;
		let cmd = commands::set_distance_freq_x10_frame(val);
		self.run_config_transaction(&cmd)
	}

	pub fn set_status_frequency(&mut self, hz: f32) -> Result<(), Error<U::Error>> {
		let val = (hz * 10.0) as u16;
		let cmd = commands::set_status_freq_x10_frame(val);
		self.run_config_transaction(&cmd)
	}

	/// Set response speed: 5 (Normal) or 10 (Fast)
	pub fn set_response_speed(&mut self, speed: u16) -> Result<(), Error<U::Error>> {
		let cmd = commands::set_response_speed_frame(speed);
		self.run_config_transaction(&cmd)
	}

	/// Set trigger thresholds for all 16 gates.
	pub fn set_trigger_thresholds(
		&mut self,
		thresholds: &[u16; 16],
	) -> Result<(), Error<U::Error>> {
		let cmd = commands::write_trigger_threshold_frame(thresholds);
		self.run_config_transaction(&cmd)
	}

	/// Set hold (maintenance) thresholds for all 16 gates.
	pub fn set_hold_thresholds(&mut self, thresholds: &[u16; 16]) -> Result<(), Error<U::Error>> {
		let cmd = commands::write_hold_threshold_frame(thresholds);
		self.run_config_transaction(&cmd)
	}

	/// Configure auto-threshold parameters.
	pub fn set_auto_threshold(
		&mut self,
		trigger_factor: u16,
		retention_factor: u16,
		scanning_time: u16,
	) -> Result<(), Error<U::Error>> {
		let cmd = commands::auto_threshold_frame(trigger_factor, retention_factor, scanning_time);
		self.run_config_transaction(&cmd)
	}

	/// Helper to wrap a command in Enter/Exit config mode frames
	fn run_config_transaction(&mut self, cmd: &[u8]) -> Result<(), Error<U::Error>> {
		self.send_and_wait_ack(&commands::enter_config_frame())?;
		self.timer.sleep(self.config.config_mode_delay);

		self.send_and_wait_ack(cmd)?;
		self.timer.sleep(self.config.config_mode_delay);

		self.send_and_wait_ack(&commands::exit_config_frame())?;
		self.timer.sleep(self.config.config_mode_delay);
		Ok(())
	}

	fn send_and_wait_ack(&mut self, cmd: &[u8]) -> Result<(), Error<U::Error>> {
		self.uart.write_all(cmd).map_err(Error::Uart)?;
		let start = self.timer.now();
		while self.timer.now() - start < self.config.command_timeout {
			if let Some(p) = self.next_packet()? {
				if let Packet::Ack(ack) = p {
					return if ack.success {
						Ok(())
					} else {
						Err(Error::CommandFailed)
					};
				}
			}
			self.timer.sleep(Duration::from_millis(10));
		}
		Err(Error::Timeout)
	}

	/// Attempt to read the next packet from UART.
	/// This is non-blocking regarding the UART if the UART implementation is non-blocking.
	pub fn next_packet(&mut self) -> Result<Option<Packet>, Error<U::Error>> {
		let mut read_buffer = [0u8; 128];
		match self.uart.read(&mut read_buffer) {
			Ok(0) => {}
			Ok(n) => self.protocol.process_bytes(&read_buffer[..n]),
			Err(e) => return Err(Error::Uart(e)),
		}

		Ok(self.protocol.next_packet())
	}

	/// Send a raw command to the sensor
	pub fn send_command(&mut self, cmd: &[u8]) -> Result<(), Error<U::Error>> {
		self.uart.write_all(cmd).map_err(Error::Uart)
	}

	pub fn read_firmware_version(&mut self) -> Result<FirmwareVersion, Error<U::Error>> {
		self.send_and_wait_ack(&commands::enter_config_frame())?;
		self.timer.sleep(self.config.config_mode_delay);

		let frame = commands::read_firmware_frame();
		self.uart.write_all(&frame).map_err(Error::Uart)?;

		let mut result = None;
		let start = self.timer.now();
		while self.timer.now() - start < self.config.command_timeout {
			if let Some(p) = self.next_packet()? {
				if let Packet::Firmware(v) = p {
					result = Some(v);
					break;
				}
			}
			self.timer.sleep(Duration::from_millis(10));
		}

		self.send_and_wait_ack(&commands::exit_config_frame())?;
		self.timer.sleep(self.config.config_mode_delay);

		result.ok_or(Error::Timeout)
	}

	pub fn read_serial_number(&mut self) -> Result<Vec<u8, 32>, Error<U::Error>> {
		self.send_and_wait_ack(&commands::enter_config_frame())?;
		self.timer.sleep(self.config.config_mode_delay);

		let frame = commands::read_serial_frame();
		self.uart.write_all(&frame).map_err(Error::Uart)?;

		let mut result = None;
		let start = self.timer.now();
		while self.timer.now() - start < self.config.command_timeout {
			if let Some(p) = self.next_packet()? {
				if let Packet::SerialNumber(sn) = p {
					result = Some(sn);
					break;
				}
			}
			self.timer.sleep(Duration::from_millis(10));
		}

		self.send_and_wait_ack(&commands::exit_config_frame())?;
		self.timer.sleep(self.config.config_mode_delay);

		result.ok_or(Error::Timeout)
	}
}
