use crate::types::Packet;
use crate::uart::UartInterface;
use crate::{OutputMode, parser::parse_minimal_frames};
use crate::{Reading, commands};
use heapless::{Deque, Vec};
use std::time::{Duration, Instant};

const STREAM_BUF_CAP: usize = 1024; // ring buffer size

/// Driver owning a UART interface
pub struct LD2410S<U: UartInterface> {
	uart: U,
	stream_buf: Deque<u8, STREAM_BUF_CAP>,
	last: Option<Packet>,
	poll_timeout: Duration,
}

impl<U> LD2410S<U>
where
	U: UartInterface,
{
	/// Create a new LD2410S driver.
	/// `poll_timeout` is the maximum time `read_latest()` will wait for a fresh frame.
	pub fn new(uart: U, poll_timeout: Duration) -> Self {
		Self {
			uart,
			stream_buf: Deque::new(),
			last: None,
			poll_timeout,
		}
	}

	pub fn init(&mut self, mode: OutputMode) -> Result<(), U::Error> {
		let frame = commands::switch_output_frame(mode);
		self.uart.write_all(&frame)?;

		Ok(())
	}

	/// Non-blocking poll: reads whatever is available now, parses frames, updates cache.
	pub fn poll(&mut self, read_chunk: usize) -> Result<Vec<Packet, 16>, U::Error> {
		let mut tmp = [0u8; 256];
		let to_read = tmp.len().min(read_chunk);
		let n = self.uart.read(&mut tmp[..to_read])?;

		for &b in &tmp[..n] {
			if self.stream_buf.push_back(b).is_err() {
				let _ = self.stream_buf.pop_front();
				let _ = self.stream_buf.push_back(b);
			}
		}

		// parse from contiguous snapshot
		let mut linear = [0u8; STREAM_BUF_CAP];
		let len = self.stream_buf.len().min(STREAM_BUF_CAP);

		for (i, b) in self.stream_buf.iter().take(len).enumerate() {
			linear[i] = *b;
		}

		// parse minimal frames
		let (min_frames, consumed) = parse_minimal_frames(&linear[..len]);

		// consume parsed bytes from ring buffer
		for _ in 0..consumed {
			let _ = self.stream_buf.pop_front();
		}

		// map MinimalPacket -> Packet and collect
		let mut out: heapless::Vec<Packet, 16> = heapless::Vec::new();
		for m in min_frames.into_iter() {
			let _ = out.push(Packet::from(m)); // Packet::Minimal(m)
		}

		// update last-known packet (if any)
		if let Some(last) = out.last().cloned() {
			self.last = Some(last);
		}

		Ok(out)
	}

	/// Wait up to the configured poll_timeout for a fresh frame;
	/// if none arrives, return cached snapshot if available.
	pub fn read_latest(&mut self) -> Result<Option<Reading>, U::Error> {
		let start = Instant::now();
		loop {
			let frames = self.poll(256)?;
			if let Some(data) = frames.last().copied() {
				self.last = Some(data);
				return Ok(Some(Reading {
					data,
					fresh: true,
				}));
			}
			if start.elapsed() >= self.poll_timeout {
				return Ok(self.last.map(|data| Reading {
					data,
					fresh: false,
				}));
			}
			std::thread::sleep(Duration::from_millis(2));
		}
	}

	/// Immediate snapshot of last-known data (if any)
	pub fn snapshot(&self) -> Option<Packet> {
		self.last
	}

	/// Send a raw command to the sensor
	pub fn send_command(&mut self, cmd: &[u8]) -> Result<(), U::Error> {
		self.uart.write_all(cmd)
	}

	pub fn set_output_mode(&mut self, mode: OutputMode) -> Result<(), U::Error> {
		let frame = commands::switch_output_frame(mode);
		self.uart.write_all(&frame)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		MinimalPresence,
		types::{MINIMAL_HEAD, MINIMAL_TAIL, MinimalPacket},
	};
	use std::vec::Vec;

	#[derive(Debug)]
	struct MockUart {
		data: Vec<u8>,
		pos: usize,
	}

	impl MockUart {
		fn new(data: Vec<u8>) -> Self {
			Self {
				data,
				pos: 0,
			}
		}
	}

	impl UartInterface for MockUart {
		type Error = ();

		fn write_all(&mut self, _data: &[u8]) -> Result<(), Self::Error> {
			Ok(())
		}

		fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
			if self.pos >= self.data.len() {
				return Ok(0);
			}
			let n = buf.len().min(self.data.len() - self.pos);
			buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
			self.pos += n;
			Ok(n)
		}
	}

	#[test]
	fn stream_buffer_drops_oldest_bytes_when_full() {
		let mut data = vec![0xFF; STREAM_BUF_CAP];
		data.extend_from_slice(&[MINIMAL_HEAD, 1, 2, 0, MINIMAL_TAIL]);
		let mut sensor = LD2410S::new(MockUart::new(data), Duration::from_millis(0));
		for _ in 0..10 {
			let _ = sensor.poll(256).unwrap();
		}
		let snapshot = sensor.snapshot();
		assert!(matches!(
			snapshot,
			Some(MinimalPacket {
				presence: false,
				distance_cm: 2,
			})
		));
	}
}
