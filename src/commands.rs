use crate::types::{CMD_HEAD, CMD_TAIL, OutputMode};
use std::vec::Vec;

// PDF command word
const CMD_SWITCH_OUTPUT: u16 = 0x007A;

/// Build: HEAD | len(u16 LE) | cmd(u16 LE) | params... | TAIL
fn build_cmd(cmd: u16, params: &[u8]) -> Vec<u8> {
	let data_len = 2 + params.len(); // cmd(2) + params
	let mut out = Vec::with_capacity(4 + 2 + data_len + 4);

	// HEAD
	out.extend_from_slice(&CMD_HEAD);
	// LEN
	out.push((data_len & 0xFF) as u8);
	out.push(((data_len >> 8) & 0xFF) as u8);
	// CMD (LE)
	out.push((cmd & 0xFF) as u8);
	out.push(((cmd >> 8) & 0xFF) as u8);
	// PARAMS
	out.extend_from_slice(params);
	// TAIL
	out.extend_from_slice(&CMD_TAIL);

	out
}

/// Switch output mode (Minimal or Standard)
pub fn switch_output_frame(mode: OutputMode) -> Vec<u8> {
	build_cmd(CMD_SWITCH_OUTPUT, &mode.as_six_bytes())
}
