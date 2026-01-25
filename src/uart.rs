use core::fmt::Debug;

// Trait that abstracts UART operations for LD2410S
pub trait UartInterface {
	type Error: Debug;

	fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error>;
	fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
}

// ─── SERIALPORT (Desktop) IMPLEMENTATION ────────────────────────────────

#[cfg(feature = "serial")]
pub use serial_impl::{SerialPortWrapper, StdTimer};

#[cfg(feature = "serial")]
mod serial_impl {
	use super::UartInterface;
	use crate::Timer;
	use serialport::SerialPort;
	use std::boxed::Box;
	use std::fmt;
	use std::io::{Read, Write};
	use std::time::{Duration, Instant};

	/// Wrapper type for `serialport` UART on desktop
	pub struct SerialPortWrapper(pub Box<dyn SerialPort>);

	impl fmt::Debug for SerialPortWrapper {
		fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			write!(f, "SerialPortWrapper")
		}
	}

	impl UartInterface for SerialPortWrapper {
		type Error = std::io::Error;

		fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
			Write::write_all(&mut self.0, data)
		}

		fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
			match Read::read(&mut self.0, buf) {
				Ok(n) => Ok(n),
				Err(e)
					if e.kind() == std::io::ErrorKind::TimedOut
						|| e.kind() == std::io::ErrorKind::WouldBlock =>
				{
					Ok(0)
				}
				Err(e) => Err(e),
			}
		}
	}

	pub struct StdTimer {
		start: Instant,
	}

	impl Default for StdTimer {
		fn default() -> Self {
			Self {
				start: Instant::now(),
			}
		}
	}

	impl Timer for StdTimer {
		fn sleep(&mut self, duration: Duration) {
			std::thread::sleep(duration);
		}

		fn now(&self) -> Duration {
			self.start.elapsed()
		}
	}
}

// ─── ESP-IDF-HAL (embedded) IMPLEMENTATION ────────────────────────────────

#[cfg(feature = "embedded")]
pub use embedded_impl::{EspTimer, EspUartWrapper};

#[cfg(feature = "embedded")]
mod embedded_impl {
	use super::UartInterface;
	use crate::Timer;
	use core::fmt;
	use core::time::Duration;
	use esp_idf_hal::{
		delay::FreeRtos,
		sys::{ESP_ERR_TIMEOUT, EspError},
		uart::UartDriver,
	};

	/// Wrapper type for `esp-idf-hal` UART on ESP32
	pub struct EspUartWrapper<'d>(pub UartDriver<'d>);

	impl<'d> fmt::Debug for EspUartWrapper<'d> {
		fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			write!(f, "EspUartWrapper")
		}
	}

	impl<'d> UartInterface for EspUartWrapper<'d> {
		type Error = EspError;

		fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
			self.0.write(data).map(|_| ())
		}

		fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
			match self.0.read(buf, 0) {
				Ok(n) => Ok(n),
				Err(e) if e.code() == ESP_ERR_TIMEOUT => Ok(0),
				Err(e) => Err(e),
			}
		}
	}

	pub struct EspTimer;

	impl Timer for EspTimer {
		fn sleep(&mut self, duration: Duration) {
			FreeRtos::delay_ms(duration.as_millis() as u32);
		}

		fn now(&self) -> Duration {
			// Use u64 to prevent overflow (ticks * 1000 can easily exceed u32::MAX)
			let ticks = unsafe { esp_idf_hal::sys::xTaskGetTickCount() } as u64;
			let hz = esp_idf_hal::sys::configTICK_RATE_HZ as u64;
			Duration::from_millis((ticks * 1000) / hz)
		}
	}
}
