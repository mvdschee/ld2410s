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
				let (p, c) = self.parse_with::<_, 4>(parse_command_responses, 256);
				if c > 0 {
					consumed = c;
					packet = p.first().cloned();
				} else {
					return None; // Incomplete
				}
			}
			// 2. Check for Standard Header
			else if head == STANDARD_HEAD {
				let (p, c) = self.parse_with::<_, 16>(parse_standard_frames, 256);
				if c > 0 {
					consumed = c;
					packet = p.first().cloned();
				} else {
					return None; // Incomplete
				}
			}
			// 3. Check for Minimal Header
			else if head[0] == MINIMAL_HEAD {
				let (p, c) = self.parse_with::<_, 16>(parse_minimal_frames, 16);
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

#[cfg(test)]
mod tests {
	extern crate std;

	use super::*;

	fn minimal_frame(distance_cm: u16) -> [u8; 5] {
		let [lo, hi] = distance_cm.to_le_bytes();
		[MINIMAL_HEAD, 0x02, lo, hi, 0x62]
	}

	// Synthetic standard frame with the real layout (energy zeroed).
	fn standard_frame(distance_cm: u16) -> [u8; 80] {
		let mut f = [0u8; 80];
		f[..4].copy_from_slice(&STANDARD_HEAD);
		f[4] = 0x46; // data_len = 70
		f[6] = 0x01; // data type
		f[7] = 0x02; // target state
		f[8..10].copy_from_slice(&distance_cm.to_le_bytes());
		f[76..].copy_from_slice(&crate::types::STANDARD_TAIL);
		f
	}

	fn distances(stream: &[u8], chunk: usize) -> std::vec::Vec<u16> {
		let mut protocol = Protocol::new();
		let mut out = std::vec::Vec::new();
		for part in stream.chunks(chunk) {
			protocol.process_bytes(part);
			while let Some(p) = protocol.next_packet() {
				match p {
					Packet::Minimal(m) => out.push(m.distance_cm),
					Packet::Standard(s) => out.push(s.distance_cm),
					_ => {}
				}
			}
		}
		out
	}

	#[test]
	fn no_minimal_frames_dropped_at_any_chunk_size() {
		let expected: std::vec::Vec<u16> = (1..=30).collect();
		let stream: std::vec::Vec<u8> = expected.iter().flat_map(|&d| minimal_frame(d)).collect();
		for chunk in [1, 7, 128, stream.len()] {
			assert_eq!(distances(&stream, chunk), expected, "chunk size {chunk}");
		}
	}

	#[test]
	fn no_standard_frames_dropped_at_any_chunk_size() {
		let expected: std::vec::Vec<u16> = (1..=10).map(|d| d * 50).collect();
		let stream: std::vec::Vec<u8> = expected.iter().flat_map(|&d| standard_frame(d)).collect();
		for chunk in [1, 7, 128, stream.len()] {
			assert_eq!(distances(&stream, chunk), expected, "chunk size {chunk}");
		}
	}

	#[test]
	fn standard_frame_energy_is_sixteen_u32_gates() {
		// Captured from a real LD2410S in Standard mode.
		let frame = [
			0xF4, 0xF3, 0xF2, 0xF1, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
			0x00, 0x00, 0xF9, 0x0E, 0x00, 0x00, 0x87, 0x0B, 0x00, 0x00, 0x84, 0x12, 0x00, 0x00,
			0x4E, 0x0B, 0x00, 0x00, 0x96, 0x04, 0x00, 0x00, 0xF3, 0x04, 0x00, 0x00, 0x9F, 0x04,
			0x00, 0x00, 0x89, 0x03, 0x00, 0x00, 0x0E, 0x05, 0x00, 0x00, 0xFF, 0x02, 0x00, 0x00,
			0x42, 0x00, 0x00, 0x00, 0x36, 0x02, 0x00, 0x00, 0x16, 0x03, 0x00, 0x00, 0x75, 0x03,
			0x00, 0x00, 0xD7, 0x01, 0x00, 0x00, 0xF8, 0xF7, 0xF6, 0xF5,
		];
		let mut protocol = Protocol::new();
		protocol.process_bytes(&frame);
		let Some(Packet::Standard(s)) = protocol.next_packet() else {
			panic!("expected a standard packet");
		};
		assert_eq!(s.target_state, crate::TargetState::Unoccupied);
		assert_eq!(
			s.energy,
			[0, 3833, 2951, 4740, 2894, 1174, 1267, 1183, 905, 1294, 767, 66, 566, 790, 885, 471]
		);
	}

	#[test]
	fn threshold_progress_frame_parses() {
		// PDF 2.2.9: F4F3F2F1 | len | 0x03 | progress(u16) | F8F7F6F5
		let frame = [0xF4, 0xF3, 0xF2, 0xF1, 0x03, 0x00, 0x03, 0x88, 0x13, 0xF8, 0xF7, 0xF6, 0xF5];
		let mut protocol = Protocol::new();
		protocol.process_bytes(&frame);
		assert!(matches!(protocol.next_packet(), Some(Packet::ThresholdProgress(5000))));
	}

	#[test]
	fn threshold_readback_ack_keeps_full_payload() {
		// Trigger threshold readback (0x0073) as sent by a real LD2410S: status + 16 x u32.
		let values: [u32; 16] = [48, 42, 36, 34, 32, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31];
		let mut frame = std::vec![0xFD, 0xFC, 0xFB, 0xFA, 0x44, 0x00, 0x73, 0x01, 0x00, 0x00];
		frame.extend(values.iter().flat_map(|v| v.to_le_bytes()));
		frame.extend([0x04, 0x03, 0x02, 0x01]);
		let mut protocol = Protocol::new();
		protocol.process_bytes(&frame);
		let Some(Packet::Ack(ack)) = protocol.next_packet() else {
			panic!("expected an ACK");
		};
		let decoded: std::vec::Vec<u32> =
			ack.payload.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
		assert_eq!(decoded, values);
	}

	#[test]
	fn no_acks_dropped_when_buffered_together() {
		// Two ACKs from a real sensor (enter config, exit config) arriving in one read.
		let enter = [
			0xFD, 0xFC, 0xFB, 0xFA, 0x08, 0x00, 0xFF, 0x01, 0x00, 0x00, 0x03, 0x00, 0x80, 0x00,
			0x04, 0x03, 0x02, 0x01,
		];
		let exit =
			[0xFD, 0xFC, 0xFB, 0xFA, 0x04, 0x00, 0xFE, 0x01, 0x00, 0x00, 0x04, 0x03, 0x02, 0x01];
		let mut protocol = Protocol::new();
		protocol.process_bytes(&enter);
		protocol.process_bytes(&exit);
		let mut commands = std::vec::Vec::new();
		while let Some(Packet::Ack(a)) = protocol.next_packet() {
			commands.push(a.command);
		}
		assert_eq!(commands, [0x00FF, 0x00FE]);
	}
}
