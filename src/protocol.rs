use crate::parser::{parse_command_responses, parse_minimal_frames, parse_standard_frames};
use crate::types::{CMD_HEAD, MINIMAL_HEAD, Packet, STANDARD_HEAD};
use heapless::Deque;

pub const STREAM_BUF_CAP: usize = 1024;

/// Protocol handles the stateful parsing of bytes into packets.
/// It is "Sans-IO", meaning it doesn't know about UART or threads.
pub struct Protocol {
	stream_buf: Deque<u8, STREAM_BUF_CAP>,
}

impl Default for Protocol {
	fn default() -> Self {
		Self::new()
	}
}

impl Protocol {
	pub fn new() -> Self {
		Self {
			stream_buf: Deque::new(),
		}
	}

	/// Feed a single byte into the parser.
	pub fn process_byte(&mut self, byte: u8) {
		if self.stream_buf.push_back(byte).is_err() {
			let _ = self.stream_buf.pop_front();
			let _ = self.stream_buf.push_back(byte);
		}
	}

	/// Feed a slice of bytes into the parser.
	pub fn process_bytes(&mut self, bytes: &[u8]) {
		for &b in bytes {
			self.process_byte(b);
		}
	}

	/// Attempt to parse the next packet from the internal buffer.
	/// Returns None if no complete packet is available.
	pub fn next_packet(&mut self) -> Option<Packet> {
		loop {
			if self.stream_buf.is_empty() {
				return None;
			}

			if self.stream_buf.len() < 4 {
				// We need at least 4 bytes to check headers (except for minimal, which is 1 byte head)
				// but even minimal needs 5 bytes total.
				let head = *self.stream_buf.front().unwrap();
				if head != MINIMAL_HEAD && head != STANDARD_HEAD[0] && head != CMD_HEAD[0] {
					let _ = self.stream_buf.pop_front();
					continue;
				}
				return None;
			}

			// Peek at the beginning of the buffer to check headers
			let mut head = [0u8; 4];
			for (i, byte) in self.stream_buf.iter().take(4).enumerate() {
				head[i] = *byte;
			}

			let consumed;
			let mut packet = None;

			// 1. Check for Command/ACK Header
			if head == CMD_HEAD {
				let (p, c) = self.parse_with::<_, 4>(|b| parse_command_responses(b), 256);
				if c > 0 {
					consumed = c;
					packet = p.first().cloned();
				} else {
					return None; // Incomplete
				}
			}
			// 2. Check for Standard Header
			else if head == STANDARD_HEAD {
				let (p, c) = self.parse_with::<_, 16>(|b| parse_standard_frames(b), 256);
				if c > 0 {
					consumed = c;
					packet = p.first().cloned();
				} else {
					return None; // Incomplete
				}
			}
			// 3. Check for Minimal Header
			else if head[0] == MINIMAL_HEAD {
				let (p, c) = self.parse_with::<_, 16>(|b| parse_minimal_frames(b), 16);
				if c > 0 {
					consumed = c;
					packet = p.first().cloned();
				} else {
					// Might be incomplete minimal frame (needs 5 bytes)
					if self.stream_buf.len() < 5 {
						return None;
					} else {
						// Malformed? skip the 0x6E
						consumed = 1;
					}
				}
			}
			// 4. Unknown byte
			else {
				consumed = 1;
			}

			// Apply consumption
			for _ in 0..consumed {
				let _ = self.stream_buf.pop_front();
			}

			if packet.is_some() {
				return packet;
			}
		}
	}

	/// Helper to copy buffer to a contiguous slice and run a parser function
	fn parse_with<F, const P: usize>(
		&self,
		parser: F,
		max_len: usize,
	) -> (heapless::Vec<Packet, P>, usize)
	where
		F: FnOnce(&[u8]) -> (heapless::Vec<Packet, P>, usize),
	{
		let snapshot_len = self.stream_buf.len().min(max_len);
		let mut snapshot = [0u8; 256]; // Should be large enough for any frame
		let final_len = snapshot_len.min(256);

		for (i, byte) in self.stream_buf.iter().take(final_len).enumerate() {
			snapshot[i] = *byte;
		}

		parser(&snapshot[..final_len])
	}
}
