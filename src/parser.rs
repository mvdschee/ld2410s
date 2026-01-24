use crate::{
	Packet, StandardPacket,
	types::{
		AckPacket, CMD_HEAD, CMD_TAIL, FirmwareVersion, MINIMAL_HEAD, MINIMAL_TAIL, MinimalPacket,
		STANDARD_HEAD, STANDARD_TAIL, TargetState,
	},
};
use heapless::Vec;

/// Parse command responses (ACKs), firmware versions, etc.
pub fn parse_command_responses(input: &[u8]) -> (Vec<Packet, 4>, usize) {
	let mut out: Vec<Packet, 4> = Vec::new();
	let mut i = 0usize;

	while i + 10 <= input.len() {
		if &input[i..i + 4] != CMD_HEAD {
			i += 1;
			continue;
		}

		let data_len = (input[i + 4] as usize) | ((input[i + 5] as usize) << 8);
		let total = 4 + 2 + data_len + 4;
		if i + total > input.len() {
			break;
		}

		if &input[i + total - 4..i + total] != CMD_TAIL {
			i += 1;
			continue;
		}

		let cmd_word = (input[i + 6] as u16) | ((input[i + 7] as u16) << 8);
		let payload = &input[i + 8..i + 6 + data_len];

		match cmd_word {
			0x0100 => {
				// Firmware version ACK
				// PDF V1.2 Page 17 Table shows:
				// Len: 0E 00 (14 bytes)
				// Payload: Cmd(2) + EquipmentType(4) + VersionType(2) + Major(2) + Minor(2) + Patch(2)
				// My payload slice starts AFTER Cmd word.
				// So payload in `parse_command_responses` is: EquipmentType(4) + VersionType(2) + Major(2) ...
				// Total remaining payload length should be 12 bytes.

				if payload.len() >= 12 {
					// Offsets relative to payload (after cmd word):
					// 0..4: Equipment Type
					// 4..6: Version Type
					// 6..8: Major
					// 8..10: Minor
					// 10..12: Patch

					let equipment_type = (payload[0] as u32)
						| ((payload[1] as u32) << 8)
						| ((payload[2] as u32) << 16)
						| ((payload[3] as u32) << 24);
					let version_type = (payload[4] as u16) | ((payload[5] as u16) << 8);
					let major = (payload[6] as u16) | ((payload[7] as u16) << 8);
					let minor = (payload[8] as u16) | ((payload[9] as u16) << 8);
					let patch = (payload[10] as u16) | ((payload[11] as u16) << 8);
					let _ = out.push(Packet::Firmware(FirmwareVersion {
						equipment_type,
						version_type,
						major,
						minor,
						patch,
					}));
				}
			}
			0x0111 => {
				// Read Serial Number ACK
				if payload.len() >= 4 {
					// PDF says: ACK(2) + SN_LEN(2) + SN(N)
					// payload[0..2] is ACK status (00 00 success)
					let _success = payload[0] == 0 && payload[1] == 0;
					let sn_len = (payload[2] as usize) | ((payload[3] as usize) << 8);
					if payload.len() >= 4 + sn_len {
						let mut sn = Vec::new();
						let _ = sn.extend_from_slice(&payload[4..4 + sn_len]);
						let _ = out.push(Packet::SerialNumber(sn));
					}
				}
			}
			_ => {
				// General ACK
				// Payload structure usually: Status(2) + ...
				if payload.len() >= 2 {
					let success = payload[0] == 0 && payload[1] == 0;
					let mut p = Vec::new();
					if payload.len() > 2 {
						let _ = p.extend_from_slice(&payload[2..]);
					}
					let _ = out.push(Packet::Ack(AckPacket {
						command: cmd_word & 0x00FF, // strip the 0x01 ACK bit roughly
						success,
						payload: p,
					}));
				}
			}
		}

		i += total;
	}
	(out, i)
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
				target_state: TargetState::from(state),
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
pub fn parse_standard_frames(input: &[u8]) -> (Vec<Packet, 16>, usize) {
	let mut out: Vec<Packet, 16> = Vec::new();
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
				let _reserved = res_lo | (res_hi << 8);

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
					target_state: TargetState::from(state),
					distance_cm,
					reserved: _reserved,
					energy,
				};
				let _ = out.push(Packet::from(pkt));
			} else {
				// Not the 0x01 standard data subtype; ignore here
			}
		}

		// Consume this frame
		i += total;
	}

	(out, i)
}
