#![allow(dead_code)]

use crate::types::{CMD_HEAD, CMD_TAIL, OutputMode};
use heapless::Vec;

// Max size for a command frame. Most are < 20 bytes.
// 256 is generous to allow for batch parameter writes if needed.
pub const MAX_CMD_SIZE: usize = 256;

// ---- Command words (from the PDF) ----
const CMD_READ_FIRMWARE: u16 = 0x0000;
const CMD_ENABLE_CONFIG: u16 = 0x00FF; // param: 0x0001
const CMD_END_CONFIG: u16 = 0x00FE;
const CMD_WRITE_SERIAL: u16 = 0x0010;
const CMD_READ_SERIAL: u16 = 0x0011;
const CMD_SWITCH_OUTPUT: u16 = 0x007A;
const CMD_WRITE_GENERIC: u16 = 0x0070; // write common/config params
const CMD_READ_GENERIC: u16 = 0x0071; // read common/config params
const CMD_AUTO_THRESHOLD: u16 = 0x0009;
const CMD_WRITE_TRIGGER_THRESHOLD: u16 = 0x0072;
const CMD_READ_TRIGGER_THRESHOLD: u16 = 0x0073;
const CMD_WRITE_HOLD_THRESHOLD: u16 = 0x0076;
const CMD_READ_HOLD_THRESHOLD: u16 = 0x0077;
const CMD_ENABLE_ENG_MODE: u16 = 0x0062;

/// Build: HEAD | len(u16 LE) | cmd(u16 LE) | params... | TAIL
fn build_cmd(cmd: u16, params: &[u8]) -> Vec<u8, MAX_CMD_SIZE> {
	let data_len = 2 + params.len(); // cmd(2) + params
	let mut out = Vec::<u8, MAX_CMD_SIZE>::new();

	// We use expect here because MAX_CMD_SIZE is sized to fit any valid protocol command.
	// If this panics, it's a developer error in sizing the buffer.

	// HEAD
	out.extend_from_slice(&CMD_HEAD).expect("Cmd buffer overflow");
	// LEN
	out.push((data_len & 0xFF) as u8).expect("Cmd buffer overflow");
	out.push(((data_len >> 8) & 0xFF) as u8).expect("Cmd buffer overflow");
	// CMD (LE)
	out.push((cmd & 0xFF) as u8).expect("Cmd buffer overflow");
	out.push(((cmd >> 8) & 0xFF) as u8).expect("Cmd buffer overflow");
	// PARAMS
	out.extend_from_slice(params).expect("Cmd buffer overflow");
	// TAIL
	out.extend_from_slice(&CMD_TAIL).expect("Cmd buffer overflow");

	out
}

// ---- High-level helpers built on build_cmd ----

pub fn read_firmware_frame() -> Vec<u8, MAX_CMD_SIZE> {
	build_cmd(CMD_READ_FIRMWARE, &[])
}

pub fn enter_config_frame() -> Vec<u8, MAX_CMD_SIZE> {
	// 0x00FF with param 0x0001
	build_cmd(CMD_ENABLE_CONFIG, &[0x01, 0x00])
}

pub fn exit_config_frame() -> Vec<u8, MAX_CMD_SIZE> {
	build_cmd(CMD_END_CONFIG, &[])
}

pub fn read_serial_frame() -> Vec<u8, MAX_CMD_SIZE> {
	build_cmd(CMD_READ_SERIAL, &[])
}

pub fn switch_output_frame(mode: OutputMode) -> Vec<u8, MAX_CMD_SIZE> {
	build_cmd(CMD_SWITCH_OUTPUT, &mode.as_six_bytes())
}

/// Write generic/common parameters (pairs of (word, value_u32 LE))
/// Example: set distance report freq (word 0x000C) to 8.0 Hz -> value = 80 (Hz*10)
pub fn write_generic_params_frame(pairs: &[(u16, u32)]) -> Vec<u8, MAX_CMD_SIZE> {
	let mut params: Vec<u8, MAX_CMD_SIZE> = Vec::new();
	for &(word, value) in pairs {
		// word (u16 LE)
		params.push((word & 0xFF) as u8).unwrap();
		params.push((word >> 8) as u8).unwrap();
		// value (u32 LE)
		params.push((value & 0xFF) as u8).unwrap();
		params.push(((value >> 8) & 0xFF) as u8).unwrap();
		params.push(((value >> 16) & 0xFF) as u8).unwrap();
		params.push(((value >> 24) & 0xFF) as u8).unwrap();
	}
	build_cmd(CMD_WRITE_GENERIC, &params)
}

/// Read generic/common parameters for a list of words
pub fn read_generic_params_frame(words: &[u16]) -> Vec<u8, MAX_CMD_SIZE> {
	let mut params: Vec<u8, MAX_CMD_SIZE> = Vec::new();
	for &w in words {
		params.push((w & 0xFF) as u8).unwrap();
		params.push((w >> 8) as u8).unwrap();
	}
	build_cmd(CMD_READ_GENERIC, &params)
}

pub fn auto_threshold_frame(
	trigger_factor: u16,
	retention_factor: u16,
	scanning_time: u16,
) -> Vec<u8, MAX_CMD_SIZE> {
	let mut params: Vec<u8, 64> = Vec::new();
	params.extend_from_slice(&[0x02, 0x00]).unwrap(); // Trigger factor word
	params.push((trigger_factor & 0xFF) as u8).unwrap();
	params.push((trigger_factor >> 8) as u8).unwrap();

	params.extend_from_slice(&[0x01, 0x00]).unwrap(); // Retention factor word
	params.push((retention_factor & 0xFF) as u8).unwrap();
	params.push((retention_factor >> 8) as u8).unwrap();

	params.extend_from_slice(&[0x78, 0x00]).unwrap(); // Scanning time word
	params.push((scanning_time & 0xFF) as u8).unwrap();
	params.push((scanning_time >> 8) as u8).unwrap();

	build_cmd(CMD_AUTO_THRESHOLD, &params)
}

fn build_threshold_params(thresholds: &[u16; 16]) -> Vec<u8, MAX_CMD_SIZE> {
	let mut params: Vec<u8, MAX_CMD_SIZE> = Vec::new();
	for (i, &val) in thresholds.iter().enumerate() {
		let word = i as u16;
		params.push((word & 0xFF) as u8).unwrap();
		params.push((word >> 8) as u8).unwrap();
		params.push((val & 0xFF) as u8).unwrap();
		params.push(((val >> 8) & 0xFF) as u8).unwrap();
		params.push(0).unwrap();
		params.push(0).unwrap();
	}

	params
}

pub fn write_trigger_threshold_frame(thresholds: &[u16; 16]) -> Vec<u8, MAX_CMD_SIZE> {
	build_cmd(CMD_WRITE_TRIGGER_THRESHOLD, &build_threshold_params(thresholds))
}

pub fn read_trigger_threshold_frame() -> Vec<u8, MAX_CMD_SIZE> {
	let mut params: Vec<u8, 64> = Vec::new();
	for i in 0..16u16 {
		params.push((i & 0xFF) as u8).unwrap();
		params.push((i >> 8) as u8).unwrap();
	}

	build_cmd(CMD_READ_TRIGGER_THRESHOLD, &params)
}

pub fn write_hold_threshold_frame(thresholds: &[u16; 16]) -> Vec<u8, MAX_CMD_SIZE> {
	build_cmd(CMD_WRITE_HOLD_THRESHOLD, &build_threshold_params(thresholds))
}

pub fn read_hold_threshold_frame() -> Vec<u8, MAX_CMD_SIZE> {
	let mut params: Vec<u8, 64> = Vec::new();
	for i in 0..16u16 {
		params.push((i & 0xFF) as u8).unwrap();
		params.push((i >> 8) as u8).unwrap();
	}
	build_cmd(CMD_READ_HOLD_THRESHOLD, &params)
}

// ---- Nice convenience wrappers ----

/// Build a frame to set the Standard "distance reporting" frequency (word 0x000C).
/// Pass Hz*10, e.g. 80 = 8.0 Hz. Use inside ENTER/EXIT CONFIG.
pub fn set_distance_freq_x10_frame(x10_hz: u16) -> Vec<u8, MAX_CMD_SIZE> {
	write_generic_params_frame(&[(0x000C, x10_hz as u32)])
}

/// Set status reporting frequency (word 0x0002).
pub fn set_status_freq_x10_frame(x10_hz: u16) -> Vec<u8, MAX_CMD_SIZE> {
	write_generic_params_frame(&[(0x0002, x10_hz as u32)])
}

/// Set response speed (word 0x000B).
/// 5 = Normal, 10 = Fast.
pub fn set_response_speed_frame(speed: u16) -> Vec<u8, MAX_CMD_SIZE> {
	write_generic_params_frame(&[(0x000B, speed as u32)])
}
