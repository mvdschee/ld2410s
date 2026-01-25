#![no_std]

#[cfg(feature = "serial")]
extern crate std;

#[cfg(feature = "embedded")]
extern crate alloc;

pub use crate::device::LD2410S;
pub use crate::error::Error;
pub use crate::types::*;

mod commands;
mod device;
mod error;
mod parser;
mod protocol;
mod types;
pub mod uart;

use core::time::Duration;

pub trait Timer {
	fn sleep(&mut self, duration: Duration);
	fn now(&self) -> Duration; // Monotonic time since some epoch
}
