use crate::types::{CMD_HEAD, CMD_TAIL, OutputMode};
use std::vec::Vec;

// ---- Command words (from the PDF) ----
const CMD_ENABLE_CONFIG: u16 = 0x00FF; // param: 0x0001
const CMD_END_CONFIG: u16 = 0x00FE;
const CMD_SWITCH_OUTPUT: u16 = 0x007A;
const CMD_WRITE_GENERIC: u16 = 0x0070; // write common/config params
const CMD_READ_GENERIC: u16 = 0x0071; // read common/config params

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

// ---- High-level helpers built on build_cmd ----

pub fn enter_config_frame() -> Vec<u8> {
	// 0x00FF with param 0x0001
	build_cmd(CMD_ENABLE_CONFIG, &[0x01, 0x00])
}

pub fn exit_config_frame() -> Vec<u8> {
	build_cmd(CMD_END_CONFIG, &[])
}

pub fn switch_output_frame(mode: OutputMode) -> Vec<u8> {
	build_cmd(CMD_SWITCH_OUTPUT, &mode.as_six_bytes())
}

/// Write generic/common parameters (pairs of (word, value_u32 LE))
/// Example: set distance report freq (word 0x000C) to 8.0 Hz -> value = 80 (Hz*10)
pub fn write_generic_params_frame(pairs: &[(u16, u32)]) -> Vec<u8> {
	let mut params: Vec<u8> = Vec::new();
	for &(word, value) in pairs {
		// word (u16 LE)
		params.push((word & 0xFF) as u8);
		params.push((word >> 8) as u8);
		// value (u32 LE)
		params.push((value & 0xFF) as u8);
		params.push(((value >> 8) & 0xFF) as u8);
		params.push(((value >> 16) & 0xFF) as u8);
		params.push(((value >> 24) & 0xFF) as u8);
	}
	build_cmd(CMD_WRITE_GENERIC, &params)
}

/// Read generic/common parameters for a list of words
pub fn read_generic_params_frame(words: &[u16]) -> Vec<u8> {
	let mut params: Vec<u8> = Vec::new();
	for &w in words {
		params.push((w & 0xFF) as u8);
		params.push((w >> 8) as u8);
	}
	build_cmd(CMD_READ_GENERIC, &params)
}

// ---- Nice convenience wrappers ----

/// Build a frame to set the Standard "distance reporting" frequency (word 0x000C).
/// Pass Hz*10, e.g. 80 = 8.0 Hz. Use inside ENTER/EXIT CONFIG.
pub fn set_distance_freq_x10_frame(x10_hz: u16) -> Vec<u8> {
	write_generic_params_frame(&[(0x000C, x10_hz as u32)])
}

/// Quick sequences if you like miniscripts:
pub fn sequence_switch_to_standard_and_8hz() -> [Vec<u8>; 4] {
	[
		enter_config_frame(),
		switch_output_frame(OutputMode::Standard),
		set_distance_freq_x10_frame(80), // 8.0 Hz
		exit_config_frame(),
	]
}

pub fn sequence_switch_to_minimal() -> [Vec<u8>; 3] {
	[enter_config_frame(), switch_output_frame(OutputMode::Minimal), exit_config_frame()]
}
