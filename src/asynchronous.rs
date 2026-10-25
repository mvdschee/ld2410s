use crate::OutputMode;
use crate::commands;
use crate::device::Config;
use crate::error::Error;
use crate::protocol::Protocol;
use crate::types::{FirmwareVersion, Packet};
use core::time::Duration as CoreDuration;
use embassy_time::{Duration, TimeoutError, Timer, with_timeout};
use embedded_io_async::{Read, Write};
use heapless::Vec;

/// Asynchronous driver for LD2410S
pub struct LD2410SAsync<U: Read + Write> {
	uart: U,
	protocol: Protocol,
	output_mode: OutputMode,
	config: Config,
}

impl<U> LD2410SAsync<U>
where
	U: Read + Write,
{
	pub fn new(uart: U, output_mode: OutputMode) -> Self {
		Self {
			uart,
			protocol: Protocol::new(),
			output_mode,
			config: Config::default(),
		}
	}

	pub fn set_config(&mut self, config: Config) {
		self.config = config;
	}

	fn to_embassy_duration(d: CoreDuration) -> Duration {
		Duration::from_micros(d.as_micros() as u64)
	}

	pub async fn init(&mut self) -> Result<(), Error<U::Error>> {
		let cmd = commands::switch_output_frame(self.output_mode);
		self.run_config_transaction(&cmd).await
	}

	async fn run_config_transaction(&mut self, cmd: &[u8]) -> Result<(), Error<U::Error>> {
		self.send_and_wait_ack(&commands::enter_config_frame()).await?;
		Timer::after(Self::to_embassy_duration(self.config.config_mode_delay)).await;

		self.send_and_wait_ack(cmd).await?;
		Timer::after(Self::to_embassy_duration(self.config.config_mode_delay)).await;

		self.send_and_wait_ack(&commands::exit_config_frame()).await?;
		Timer::after(Self::to_embassy_duration(self.config.config_mode_delay)).await;
		Ok(())
	}

	async fn send_and_wait_ack(&mut self, cmd: &[u8]) -> Result<(), Error<U::Error>> {
		self.uart.write_all(cmd).await.map_err(Error::Uart)?;
		self.uart.flush().await.map_err(Error::Uart)?;

		let expected = commands::command_word(cmd);
		let res = with_timeout(Self::to_embassy_duration(self.config.command_timeout), async {
			loop {
				match self.next_packet().await? {
					// Ignore stale ACKs for earlier commands; the sensor sometimes repeats them.
					Some(Packet::Ack(ack)) if ack.command == expected => {
						return if ack.success {
							Ok(())
						} else {
							Err(Error::CommandFailed)
						};
					}
					Some(_) => {
						// Received a non-ACK packet (e.g. data).
						// Process the next packet immediately; don't sleep.
						continue;
					}
					None => {
						// No packet available yet. Yield/sleep briefly.
						Timer::after(Duration::from_millis(5)).await;
					}
				}
			}
		})
		.await;

		match res {
			Ok(inner) => inner,
			Err(TimeoutError) => Err(Error::Timeout),
		}
	}

	pub async fn next_packet(&mut self) -> Result<Option<Packet>, Error<U::Error>> {
		if let Some(p) = self.protocol.next_packet() {
			return Ok(Some(p));
		}

		let mut buf = [0u8; 128];
		let n = self.uart.read(&mut buf).await.map_err(Error::Uart)?;
		if n > 0 {
			self.protocol.process_bytes(&buf[..n]);
		}

		Ok(self.protocol.next_packet())
	}

	pub async fn read_firmware_version(&mut self) -> Result<FirmwareVersion, Error<U::Error>> {
		self.send_and_wait_ack(&commands::enter_config_frame()).await?;
		Timer::after(Self::to_embassy_duration(self.config.config_mode_delay)).await;

		let frame = commands::read_firmware_frame();
		self.uart.write_all(&frame).await.map_err(Error::Uart)?;
		self.uart.flush().await.map_err(Error::Uart)?;

		let res = with_timeout(Self::to_embassy_duration(self.config.command_timeout), async {
			loop {
				match self.next_packet().await? {
					Some(Packet::Firmware(v)) => return Ok::<FirmwareVersion, Error<U::Error>>(v),
					Some(_) => continue,
					None => Timer::after(Duration::from_millis(5)).await,
				}
			}
		})
		.await;

		let result = match res {
			Ok(inner) => inner?,
			Err(TimeoutError) => return Err(Error::Timeout),
		};

		self.send_and_wait_ack(&commands::exit_config_frame()).await?;
		Timer::after(Self::to_embassy_duration(self.config.config_mode_delay)).await;

		Ok(result)
	}

	pub async fn set_distance_frequency(&mut self, hz: f32) -> Result<(), Error<U::Error>> {
		let val = (hz * 10.0) as u16;
		let cmd = commands::set_distance_freq_x10_frame(val);
		self.run_config_transaction(&cmd).await
	}

	pub async fn set_status_frequency(&mut self, hz: f32) -> Result<(), Error<U::Error>> {
		let val = (hz * 10.0) as u16;
		let cmd = commands::set_status_freq_x10_frame(val);
		self.run_config_transaction(&cmd).await
	}

	/// Set the detection range in distance gates (~0.7 m each); motion outside it is ignored.
	pub async fn set_gate_range(
		&mut self,
		nearest: u8,
		farthest: u8,
	) -> Result<(), Error<U::Error>> {
		let cmd = commands::set_gate_range_frame(nearest, farthest);
		self.run_config_transaction(&cmd).await
	}

	/// Set the delay (10..=120 s) before "no one" is reported after the last detection.
	pub async fn set_no_one_delay(&mut self, seconds: u16) -> Result<(), Error<U::Error>> {
		let cmd = commands::set_no_one_delay_frame(seconds);
		self.run_config_transaction(&cmd).await
	}

	pub async fn set_response_speed(&mut self, speed: u16) -> Result<(), Error<U::Error>> {
		let cmd = commands::set_response_speed_frame(speed);
		self.run_config_transaction(&cmd).await
	}

	pub async fn set_trigger_thresholds(
		&mut self,
		thresholds: &[u16; 16],
	) -> Result<(), Error<U::Error>> {
		let cmd = commands::write_trigger_threshold_frame(thresholds);
		self.run_config_transaction(&cmd).await
	}

	pub async fn set_hold_thresholds(
		&mut self,
		thresholds: &[u16; 16],
	) -> Result<(), Error<U::Error>> {
		let cmd = commands::write_hold_threshold_frame(thresholds);
		self.run_config_transaction(&cmd).await
	}

	/// Configure auto-threshold parameters.
	pub async fn set_auto_threshold(
		&mut self,
		trigger_factor: u16,
		retention_factor: u16,
		scanning_time: u16,
	) -> Result<(), Error<U::Error>> {
		let cmd = commands::auto_threshold_frame(trigger_factor, retention_factor, scanning_time);
		self.send_and_wait_ack(&commands::enter_config_frame()).await?;
		Timer::after(Self::to_embassy_duration(self.config.config_mode_delay)).await;
		self.send_and_wait_ack(&cmd).await?;
		Timer::after(Self::to_embassy_duration(self.config.config_mode_delay)).await;
		// The scan starts on exit and the sensor does not ACK that exit (seen on firmware 1.1.1).
		let exit = commands::exit_config_frame();
		self.uart.write_all(&exit).await.map_err(Error::Uart)?;
		self.uart.flush().await.map_err(Error::Uart)
	}

	pub async fn read_serial_number(&mut self) -> Result<Vec<u8, 32>, Error<U::Error>> {
		self.send_and_wait_ack(&commands::enter_config_frame()).await?;
		Timer::after(Self::to_embassy_duration(self.config.config_mode_delay)).await;

		let frame = commands::read_serial_frame();
		self.uart.write_all(&frame).await.map_err(Error::Uart)?;
		self.uart.flush().await.map_err(Error::Uart)?;

		let res = with_timeout(Self::to_embassy_duration(self.config.command_timeout), async {
			loop {
				match self.next_packet().await? {
					Some(Packet::SerialNumber(sn)) => {
						return Ok::<Vec<u8, 32>, Error<U::Error>>(sn);
					}
					Some(_) => continue,
					None => Timer::after(Duration::from_millis(5)).await,
				}
			}
		})
		.await;

		let result = match res {
			Ok(inner) => inner?,
			Err(TimeoutError) => return Err(Error::Timeout),
		};

		self.send_and_wait_ack(&commands::exit_config_frame()).await?;
		Timer::after(Self::to_embassy_duration(self.config.config_mode_delay)).await;

		Ok(result)
	}
}
