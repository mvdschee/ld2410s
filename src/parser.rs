use crate::types::{MINIMAL_HEAD, MINIMAL_TAIL, MinimalPacket};
use heapless::Vec;

/// Parse one or more minimal packets from a byte slice
pub fn parse_minimal_frames(input: &[u8]) -> (Vec<MinimalPacket, 16>, usize) {
	let mut out: Vec<MinimalPacket, 16> = Vec::new();
	let mut i = 0usize;

	while i + 5 <= input.len() {
		if input[i] == MINIMAL_HEAD && input[i + 4] == MINIMAL_TAIL {
			let state = input[i + 1];
			let lo = input[i + 2] as u16;
			let hi = input[i + 3] as u16;
			let dist = lo | (hi << 8);
			let pkt = MinimalPacket {
				presence: MinimalPacket::presence_hint(state),
				distance_cm: dist,
			};
			let _ = out.push(pkt);
			i += 5;
		} else {
			i += 1;
		}
	}
	(out, i)
}
