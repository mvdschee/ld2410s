use core::fmt::Debug;

#[derive(Debug)]
pub enum Error<E: Debug> {
	Uart(E),
	Timeout,
	BufferOverflow,
	InvalidFrame,
	CommandFailed,
}

impl<E: Debug> From<E> for Error<E> {
	fn from(e: E) -> Self {
		Error::Uart(e)
	}
}
