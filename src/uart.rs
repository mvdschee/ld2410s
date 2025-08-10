use std::fmt::Debug;

// Trait that abstracts UART operations for LD2410S
pub trait UartInterface {
	type Error: Debug;

	fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error>;
	fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
}

// ─── SERIALPORT (Desktop) IMPLEMENTATION ────────────────────────────────

#[cfg(feature = "serial")]
pub use serial_impl::SerialPortWrapper;

#[cfg(feature = "serial")]
mod serial_impl {
	use super::UartInterface;
	use serialport::SerialPort;
	use std::fmt;
	use std::io::{Read, Write};

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
}

// ─── ESP-IDF-HAL (embedded) IMPLEMENTATION ────────────────────────────────

#[cfg(feature = "embedded")]
pub use embedded_impl::EspUartWrapper;

#[cfg(feature = "embedded")]
mod embedded_impl {
	use super::UartInterface;
	use core::fmt;
	use esp_idf_hal::{
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
			// esp-idf-hal write is already blocking for the slice
			self.0.write(data)
		}

		fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
			// Treat UART timeout as "no data" (Ok(0)), pass through other errors.
			match self.0.read(buf) {
				Ok(n) => Ok(n),
				Err(e) if e.code() == ESP_ERR_TIMEOUT => Ok(0),
				Err(e) => Err(e),
			}
		}
	}
}
