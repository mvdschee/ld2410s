use crate::{
	Packet, StandardPacket,
	types::{MINIMAL_HEAD, MINIMAL_TAIL, MinimalPacket, STANDARD_HEAD, STANDARD_TAIL},
};
use heapless::Vec;

pub fn presence_from_state(value: u8) -> bool {
	match value {
		0 | 1 => false,
		2 | 3 => true,
		_ => false,
	}
}

/// Parse one or more minimal packets from a byte slice
pub fn parse_minimal_frames(input: &[u8]) -> (Vec<Packet, 16>, usize) {
	let mut out: Vec<Packet, 16> = Vec::new();
	let mut i = 0usize;

	while i + 5 <= input.len() {
		if input[i] == MINIMAL_HEAD && input[i + 4] == MINIMAL_TAIL {
			let state = input[i + 1];
			let lo = input[i + 2] as u16;
			let hi = input[i + 3] as u16;
			let dist = lo | (hi << 8);
			let pkt = MinimalPacket {
				presence: presence_from_state(state),
				distance_cm: dist,
			};
			let _ = out.push(Packet::from(pkt));
			i += 5;
		} else {
			i += 1;
		}
	}
	(out, i)
}

/// Parse one or more *standard* frames (Table 2-1).
/// Returns (frames, bytes_consumed).
pub fn parse_standard_frames(input: &[u8]) -> (Vec<Packet, 8>, usize) {
	let mut out: Vec<Packet, 8> = Vec::new();
	let mut i = 0usize;

	while i + 10 <= input.len() {
		// Seek header
		if i + 4 > input.len() || &input[i..i + 4] != STANDARD_HEAD {
			i += 1;
			continue;
		}

		// Need length
		if i + 6 > input.len() {
			break; // incomplete length
		}
		let data_len = (input[i + 4] as usize) | ((input[i + 5] as usize) << 8);

		// Entire frame size = head(4) + len(2) + data_len + tail(4)
		let total = 4 + 2 + data_len + 4;
		if i + total > input.len() {
			break; // incomplete frame
		}

		// Tail check
		if &input[i + total - 4..i + total] != STANDARD_TAIL {
			// Desync: skip this byte and resync
			i += 1;
			continue;
		}

		// Payload starts at i+6
		let data_start = i + 6;
		let data_end = data_start + data_len;

		// Need at least: data_type(1) + state(1) + distance(2) + reserved(2) = 6 bytes
		if data_len >= 6 {
			let data_type = input[data_start];
			if data_type == 0x01 {
				let state = input[data_start + 1];
				let dist_lo = input[data_start + 2] as u16;
				let dist_hi = input[data_start + 3] as u16;
				let distance_cm = dist_lo | (dist_hi << 8);

				let res_lo = input[data_start + 4] as u16;
				let res_hi = input[data_start + 5] as u16;
				let reserved = res_lo | (res_hi << 8);

				// Energy bytes begin at offset 6 of payload, up to 64 bytes (if present)
				let energy_start = data_start + 6;
				let mut energy: Vec<u8, 64> = Vec::new();
				if energy_start < data_end {
					let available = data_end - energy_start;
					let to_copy = core::cmp::min(available, 64);
					for k in 0..to_copy {
						let _ = energy.push(input[energy_start + k]);
					}
				}

				let pkt = StandardPacket {
					data_type,
					presence: presence_from_state(state),
					distance_cm,
					reserved,
					energy,
				};
				let _ = out.push(Packet::from(pkt));
			} else {
				// Not the 0x01 standard data subtype; ignore here (e.g., 0x03 progress)
				// You can add a separate parser for 0x03 if you want.
			}
		}

		// Consume this frame
		i += total;
	}

	(out, i)
}
