use crate::parser::{parse_command_responses, parse_minimal_frames, parse_standard_frames};
use crate::types::{CMD_HEAD, MAX_FRAME_LEN, MINIMAL_HEAD, Packet, STANDARD_HEAD};

pub const STREAM_BUF_CAP: usize = 2 * MAX_FRAME_LEN;

// When `next_packet` returns None, under MAX_FRAME_LEN bytes of a partial frame remain, so
// the next read gets more than MAX_FRAME_LEN bytes of space: a full 128-byte UART FIFO fits.
const _: () = assert!(STREAM_BUF_CAP >= 2 * MAX_FRAME_LEN);

/// Protocol handles the stateful parsing of bytes into packets.
/// It is "Sans-IO", meaning it doesn't know about UART or threads.
/// The UART reads straight into its buffer (`spare_mut` + `commit`), so nothing is copied twice.
pub struct Protocol {
	buf: [u8; STREAM_BUF_CAP],
	/// Unparsed bytes are `buf[start..end]`.
	start: usize,
	end: usize,
}

impl Default for Protocol {
	fn default() -> Self {
		Self::new()
	}
}

impl Protocol {
	pub fn new() -> Self {
		Self {
			buf: [0; STREAM_BUF_CAP],
			start: 0,
			end: 0,
		}
	}

	/// Space for the next UART read, after moving unparsed bytes to the front.
	pub fn spare_mut(&mut self) -> &mut [u8] {
		if self.start > 0 {
			self.buf.copy_within(self.start..self.end, 0);
			self.end -= self.start;
			self.start = 0;
		}
		&mut self.buf[self.end..]
	}

	/// Marks `n` bytes written into `spare_mut` as received.
	pub fn commit(&mut self, n: usize) {
		self.end += n;
	}

	/// Attempt to parse the next packet from the internal buffer.
	/// Returns None if no complete packet is available.
	pub fn next_packet(&mut self) -> Option<Packet> {
		loop {
			let data = &self.buf[self.start..self.end];
			if data.is_empty() {
				return None;
			}

			if data.len() < 4 {
				// We need at least 4 bytes to check headers (except for minimal, which is 1 byte head)
				// but even minimal needs 5 bytes total.
				let head = data[0];
				if head != MINIMAL_HEAD && head != STANDARD_HEAD[0] && head != CMD_HEAD[0] {
					self.start += 1;
					continue;
				}
				return None;
			}

			let consumed;
			let mut packet = None;

			// 1. Check for Command/ACK Header
			if data[..4] == CMD_HEAD {
				let (p, c) = parse_command_responses(window(data, STREAM_BUF_CAP));
				if c > 0 {
					consumed = c;
					packet = p;
				} else {
					return None; // Incomplete
				}
			}
			// 2. Check for Standard Header
			else if data[..4] == STANDARD_HEAD {
				let (p, c) = parse_standard_frames(window(data, STREAM_BUF_CAP));
				if c > 0 {
					consumed = c;
					packet = p;
				} else {
					return None; // Incomplete
				}
			}
			// 3. Check for Minimal Header
			else if data[0] == MINIMAL_HEAD {
				let (p, c) = parse_minimal_frames(window(data, 16));
				if c > 0 {
					consumed = c;
					packet = p;
				} else {
					// Might be incomplete minimal frame (needs 5 bytes)
					if data.len() < 5 {
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

			self.start += consumed;

			if packet.is_some() {
				return packet;
			}
		}
	}
}

/// The first `max_len` bytes: how far a parser may look ahead.
fn window(data: &[u8], max_len: usize) -> &[u8] {
	&data[..data.len().min(max_len)]
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

	/// Copies a whole read into the parse buffer, as the drivers do.
	fn feed(protocol: &mut Protocol, bytes: &[u8]) {
		protocol.spare_mut()[..bytes.len()].copy_from_slice(bytes);
		protocol.commit(bytes.len());
	}

	/// Drives the protocol like the drivers: drain, then read up to `chunk` bytes into the space left.
	fn distances(stream: &[u8], chunk: usize) -> std::vec::Vec<u16> {
		let mut protocol = Protocol::new();
		let mut out = std::vec::Vec::new();
		let mut rest = stream;
		loop {
			while let Some(p) = protocol.next_packet() {
				match p {
					Packet::Minimal(m) => out.push(m.distance_cm),
					Packet::Standard(s) => out.push(s.distance_cm),
					_ => {}
				}
			}
			if rest.is_empty() {
				return out;
			}
			let n = chunk.min(rest.len()).min(protocol.spare_mut().len());
			feed(&mut protocol, &rest[..n]);
			rest = &rest[n..];
		}
	}

	#[test]
	fn a_full_uart_fifo_always_fits_after_draining() {
		// Frames, truncated frames and garbage, biased to header bytes, read in random sizes.
		let mut seed = 0x2545_F491_u32;
		let mut rand = move |n: u32| {
			seed ^= seed << 13;
			seed ^= seed >> 17;
			seed ^= seed << 5;
			seed % n
		};
		let mut stream = std::vec::Vec::new();
		for _ in 0..5000 {
			match rand(6) {
				0 => stream.extend(minimal_frame(rand(800) as u16)),
				1 => stream.extend(standard_frame(rand(800) as u16)),
				2 => stream.extend(&standard_frame(1)[..rand(80) as usize]),
				3 => stream.extend([0xFD, 0xFC, 0xFB, 0xFA, rand(256) as u8, 0x00]),
				4 => stream.push([MINIMAL_HEAD, 0xF4, 0xFD, 0x62][rand(4) as usize]),
				_ => stream.push(rand(256) as u8),
			}
		}
		let mut protocol = Protocol::new();
		let mut rest = &stream[..];
		while !rest.is_empty() {
			while protocol.next_packet().is_some() {}
			assert!(protocol.spare_mut().len() > MAX_FRAME_LEN);
			let n = (rand(STREAM_BUF_CAP as u32) as usize + 1).min(rest.len());
			let n = n.min(protocol.spare_mut().len());
			feed(&mut protocol, &rest[..n]);
			rest = &rest[n..];
		}
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
		feed(&mut protocol, &frame);
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
		feed(&mut protocol, &frame);
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
		feed(&mut protocol, &frame);
		let Some(Packet::Ack(ack)) = protocol.next_packet() else {
			panic!("expected an ACK");
		};
		let decoded: std::vec::Vec<u32> =
			ack.payload.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
		assert_eq!(decoded, values);
	}

	#[test]
	fn truncated_header_does_not_stall_the_stream() {
		// Bytes lost right after a header (e.g. an RX FIFO overflow) leave a garbage length.
		// The ACK behind it must still come out: in config mode the sensor sends nothing else.
		let ack =
			[0xFD, 0xFC, 0xFB, 0xFA, 0x04, 0x00, 0x70, 0x01, 0x00, 0x00, 0x04, 0x03, 0x02, 0x01];
		for head in [STANDARD_HEAD, CMD_HEAD] {
			let mut protocol = Protocol::new();
			feed(&mut protocol, &head);
			feed(&mut protocol, &ack);
			assert!(
				matches!(protocol.next_packet(), Some(Packet::Ack(a)) if a.command == 0x0070),
				"after {head:02X?}"
			);
		}
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
		feed(&mut protocol, &enter);
		feed(&mut protocol, &exit);
		let mut commands = std::vec::Vec::new();
		while let Some(Packet::Ack(a)) = protocol.next_packet() {
			commands.push(a.command);
		}
		assert_eq!(commands, [0x00FF, 0x00FE]);
	}
}
