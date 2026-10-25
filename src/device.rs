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

	/// Set the detection range in distance gates (~0.7 m each); motion outside it is ignored.
	pub fn set_gate_range(&mut self, nearest: u8, farthest: u8) -> Result<(), Error<U::Error>> {
		let cmd = commands::set_gate_range_frame(nearest, farthest);
		self.run_config_transaction(&cmd)
	}

	/// Set the delay (10..=120 s) before "no one" is reported after the last detection.
	pub fn set_no_one_delay(&mut self, seconds: u16) -> Result<(), Error<U::Error>> {
		let cmd = commands::set_no_one_delay_frame(seconds);
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
		self.send_and_wait_ack(&commands::enter_config_frame())?;
		self.timer.sleep(self.config.config_mode_delay);
		self.send_and_wait_ack(&cmd)?;
		self.timer.sleep(self.config.config_mode_delay);
		// The scan starts on exit and the sensor does not ACK that exit (seen on firmware 1.1.1).
		self.uart.write_all(&commands::exit_config_frame()).map_err(Error::Uart)
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
		let expected = commands::command_word(cmd);
		let start = self.timer.now();
		while self.timer.now() - start < self.config.command_timeout {
			match self.next_packet()? {
				// Ignore stale ACKs for earlier commands; the sensor sometimes repeats them.
				Some(Packet::Ack(ack)) if ack.command == expected => {
					return if ack.success {
						Ok(())
					} else {
						Err(Error::CommandFailed)
					};
				}
				// Drain buffered data frames without sleeping, or a backlog outlasts the timeout.
				Some(_) => {}
				None => self.timer.sleep(Duration::from_millis(10)),
			}
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
			match self.next_packet()? {
				Some(Packet::Firmware(v)) => {
					result = Some(v);
					break;
				}
				Some(_) => {}
				None => self.timer.sleep(Duration::from_millis(10)),
			}
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
			match self.next_packet()? {
				Some(Packet::SerialNumber(sn)) => {
					result = Some(sn);
					break;
				}
				Some(_) => {}
				None => self.timer.sleep(Duration::from_millis(10)),
			}
		}

		self.send_and_wait_ack(&commands::exit_config_frame())?;
		self.timer.sleep(self.config.config_mode_delay);

		result.ok_or(Error::Timeout)
	}
}

#[cfg(test)]
mod tests {
	extern crate std;

	use super::*;
	use core::cell::Cell;
	use std::vec::Vec as StdVec;

	/// Replies to each written command with the frames the closure returns.
	struct ScriptedUart<F: FnMut(u16) -> StdVec<u8>> {
		reply: F,
		rx: StdVec<u8>,
	}

	impl<F: FnMut(u16) -> StdVec<u8>> UartInterface for ScriptedUart<F> {
		type Error = ();
		fn write_all(&mut self, data: &[u8]) -> Result<(), ()> {
			let reply = (self.reply)(commands::command_word(data));
			self.rx.extend(reply);
			Ok(())
		}
		fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
			let n = self.rx.len().min(buf.len());
			buf[..n].copy_from_slice(&self.rx[..n]);
			self.rx.drain(..n);
			Ok(n)
		}
	}

	/// Time only moves when the driver sleeps.
	struct FakeTimer(Cell<Duration>);
	impl Timer for FakeTimer {
		fn sleep(&mut self, d: Duration) {
			self.0.set(self.0.get() + d);
		}
		fn now(&self) -> Duration {
			self.0.get()
		}
	}

	fn ack(cmd: u16) -> StdVec<u8> {
		let [lo, _] = cmd.to_le_bytes();
		StdVec::from([
			0xFD, 0xFC, 0xFB, 0xFA, 0x04, 0x00, lo, 0x01, 0x00, 0x00, 0x04, 0x03, 0x02, 0x01,
		])
	}

	#[test]
	fn stale_ack_does_not_confirm_a_dropped_command() {
		// Seen on real hardware: the sensor repeats the enter-config ACK and ignores the next command.
		let uart = ScriptedUart {
			reply: |cmd| {
				if cmd == 0x0070 {
					ack(0x00FF)
				} else {
					ack(cmd)
				}
			},
			rx: StdVec::new(),
		};
		let mut dev = LD2410S::new(uart, FakeTimer(Cell::new(Duration::ZERO)), OutputMode::Minimal);
		assert!(matches!(dev.set_response_speed(10), Err(Error::Timeout)));
	}

	#[test]
	fn ack_behind_a_backlog_of_data_frames_is_found() {
		// Seen on real hardware: 15 s of unread data frames (8 Hz) sit in front of the ACK.
		let backlog: StdVec<u8> = (0..120).flat_map(|_| [0x6E, 0x02, 0x64, 0x00, 0x62]).collect();
		let uart = ScriptedUart {
			reply: ack,
			rx: backlog,
		};
		let mut dev = LD2410S::new(uart, FakeTimer(Cell::new(Duration::ZERO)), OutputMode::Minimal);
		assert!(dev.set_response_speed(10).is_ok());
	}

	#[test]
	fn auto_threshold_does_not_wait_for_exit_ack() {
		// Seen on real hardware: after 0x0009 the sensor leaves config mode to scan without ACKing the exit.
		let mut calibrating = false;
		let uart = ScriptedUart {
			reply: move |cmd| {
				if cmd == 0x00FE && calibrating {
					return StdVec::new();
				}
				calibrating |= cmd == 0x0009;
				ack(cmd)
			},
			rx: StdVec::new(),
		};
		let mut dev = LD2410S::new(uart, FakeTimer(Cell::new(Duration::ZERO)), OutputMode::Minimal);
		assert!(dev.set_auto_threshold(2, 1, 120).is_ok());
	}

	#[test]
	fn matching_ack_confirms_command() {
		let uart = ScriptedUart {
			reply: ack,
			rx: StdVec::new(),
		};
		let mut dev = LD2410S::new(uart, FakeTimer(Cell::new(Duration::ZERO)), OutputMode::Minimal);
		assert!(dev.set_response_speed(10).is_ok());
	}
}
