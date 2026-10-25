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
		let delay = Self::to_embassy_duration(self.config.config_mode_delay);
		let mut result = Err(Error::Timeout);
		// The sensor sometimes answers the first command after enter with a repeated
		// enter ACK and drops the command (seen after a fresh plug-in), so retry once.
		for _ in 0..2 {
			self.send_and_wait_ack(&commands::enter_config_frame()).await?;
			Timer::after(delay).await;

			result = self.send_and_wait_ack(cmd).await;
			Timer::after(delay).await;
			if !matches!(result, Err(Error::Timeout)) {
				break;
			}
		}

		// Always leave config mode, or the sensor stops reporting data.
		self.send_and_wait_ack(&commands::exit_config_frame()).await?;
		Timer::after(delay).await;
		result
	}

	async fn send_and_wait_ack(&mut self, cmd: &[u8]) -> Result<(), Error<U::Error>> {
		self.uart.write_all(cmd).await.map_err(Error::Uart)?;
		self.uart.flush().await.map_err(Error::Uart)?;

		let expected = commands::command_word(cmd);
		let res = with_timeout(Self::to_embassy_duration(self.config.command_timeout), async {
			loop {
				match self.next_packet().await {
					// Ignore stale ACKs for earlier commands; the sensor sometimes repeats them.
					Ok(Some(Packet::Ack(ack))) if ack.command == expected => {
						return if ack.success {
							Ok(())
						} else {
							Err(Error::CommandFailed)
						};
					}
					Ok(Some(_)) => {
						// Received a non-ACK packet (e.g. data).
						// Process the next packet immediately; don't sleep.
						continue;
					}
					// No packet available yet, or a read error: bytes were lost (e.g. esp-hal's
					// 128-byte RX FIFO overflowed during a pause), but the ACK may still follow.
					// Yield/sleep briefly; the timeout bounds a persistent error.
					Ok(None) | Err(_) => {
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
				match self.next_packet().await {
					Ok(Some(Packet::Firmware(v))) => return v,
					Ok(Some(_)) => continue,
					// A read error means lost bytes, not a lost reply (see send_and_wait_ack).
					Ok(None) | Err(_) => Timer::after(Duration::from_millis(5)).await,
				}
			}
		})
		.await;

		let result = match res {
			Ok(inner) => inner,
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
				match self.next_packet().await {
					Ok(Some(Packet::SerialNumber(sn))) => return sn,
					Ok(Some(_)) => continue,
					// A read error means lost bytes, not a lost reply (see send_and_wait_ack).
					Ok(None) | Err(_) => Timer::after(Duration::from_millis(5)).await,
				}
			}
		})
		.await;

		let result = match res {
			Ok(inner) => inner,
			Err(TimeoutError) => return Err(Error::Timeout),
		};

		self.send_and_wait_ack(&commands::exit_config_frame()).await?;
		Timer::after(Self::to_embassy_duration(self.config.config_mode_delay)).await;

		Ok(result)
	}
}

#[cfg(test)]
mod tests {
	extern crate std;

	use super::*;
	use embedded_io_async::{ErrorKind, ErrorType};
	use std::vec::Vec as StdVec;

	/// ACKs every command; the first `failed_reads` reads fail like an overflowed FIFO.
	struct OverflowingUart {
		failed_reads: usize,
		rx: StdVec<u8>,
	}

	impl ErrorType for OverflowingUart {
		type Error = ErrorKind;
	}

	impl Write for OverflowingUart {
		async fn write(&mut self, data: &[u8]) -> Result<usize, ErrorKind> {
			let [lo, _] = commands::command_word(data).to_le_bytes();
			self.rx.extend([
				0xFD, 0xFC, 0xFB, 0xFA, 0x04, 0x00, lo, 0x01, 0x00, 0x00, 0x04, 0x03, 0x02, 0x01,
			]);
			Ok(data.len())
		}

		async fn flush(&mut self) -> Result<(), ErrorKind> {
			Ok(())
		}
	}

	impl Read for OverflowingUart {
		async fn read(&mut self, buf: &mut [u8]) -> Result<usize, ErrorKind> {
			if self.failed_reads > 0 {
				self.failed_reads -= 1;
				return Err(ErrorKind::Other);
			}
			let n = self.rx.len().min(buf.len());
			buf[..n].copy_from_slice(&self.rx[..n]);
			self.rx.drain(..n);
			Ok(n)
		}
	}

	#[tokio::test]
	async fn read_error_while_waiting_for_ack_is_not_fatal() {
		// Seen on an ESP32-C3 with esp-hal: the 128-byte RX FIFO overflows during the driver's
		// pauses, and the next read reports it. The bytes are lost, the ACK still follows.
		let uart = OverflowingUart {
			failed_reads: 1,
			rx: StdVec::new(),
		};
		let mut dev = LD2410SAsync::new(uart, OutputMode::Minimal);
		assert!(dev.set_response_speed(10).await.is_ok());
	}
}
