use heapless::Vec;

pub const BAUD_RATE: u32 = 115_200;

// Minimal 5-byte frame markers
pub const MINIMAL_HEAD: u8 = 0x6E;
pub const MINIMAL_TAIL: u8 = 0x62;

// Standard frame markers
pub const STANDARD_HEAD: [u8; 4] = [0xF4, 0xF3, 0xF2, 0xF1];
pub const STANDARD_TAIL: [u8; 4] = [0xF8, 0xF7, 0xF6, 0xF5];

// Command markers
pub const CMD_HEAD: [u8; 4] = [0xFD, 0xFC, 0xFB, 0xFA];
pub const CMD_TAIL: [u8; 4] = [0x04, 0x03, 0x02, 0x01];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputMode {
	Minimal,
	Standard,
}

impl OutputMode {
	/// CMD_SWITCH_OUTPUT (0x007A) payload per PDF
	pub fn as_six_bytes(self) -> [u8; 6] {
		match self {
			OutputMode::Minimal => [0, 0, 0, 0, 0, 0],
			OutputMode::Standard => [0, 0, 0, 1, 0, 0],
		}
	}
}

#[derive(Clone, Debug)]
pub struct MinimalPacket {
	pub presence: bool,
	pub distance_cm: u16,
}

/// Standard data frame (Table 2-1: data_type=0x01)
#[derive(Clone, Debug)]
pub struct StandardPacket {
	pub data_type: u8,       // usually 0x01
	pub presence: bool,      // mapped from “target state” (0/1 none, 2/3 some)
	pub distance_cm: u16,    // little-endian
	pub reserved: u16,       // 2 bytes after distance
	pub energy: Vec<u8, 64>, // up to 64 energy bytes (distance gates)
}

#[derive(Clone, Debug)]
pub enum Packet {
	Minimal(MinimalPacket),
	Standard(StandardPacket),
}

impl Packet {
	pub fn as_minimal(&self) -> Option<&MinimalPacket> {
		match self {
			Packet::Minimal(m) => Some(m),
			_ => None,
		}
	}

	pub fn as_standard(&self) -> Option<&StandardPacket> {
		match self {
			Packet::Standard(s) => Some(s),
			_ => None,
		}
	}
}

/// Returned by read_latest(): either fresh data or cached
#[derive(Clone, Debug)]
pub struct Reading {
	pub data: Packet,
	pub fresh: bool,
}

impl From<MinimalPacket> for Packet {
	fn from(m: MinimalPacket) -> Self {
		Packet::Minimal(m)
	}
}

impl From<StandardPacket> for Packet {
	fn from(s: StandardPacket) -> Self {
		Packet::Standard(s)
	}
}
