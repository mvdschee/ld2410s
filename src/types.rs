use heapless::Vec;

pub const BAUD_RATE: u32 = 115_200;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TargetState {
	Unoccupied, // 0x00 or 0x01
	Occupied,   // 0x02 or 0x03
	Unknown(u8),
}

impl From<u8> for TargetState {
	fn from(byte: u8) -> Self {
		match byte {
			0x00 | 0x01 => TargetState::Unoccupied,
			0x02 | 0x03 => TargetState::Occupied,
			b => TargetState::Unknown(b),
		}
	}
}

// Minimal 5-byte frame markers
pub const MINIMAL_HEAD: u8 = 0x6E;
pub const MINIMAL_TAIL: u8 = 0x62;

// Standard frame markers
pub const STANDARD_HEAD: [u8; 4] = [0xF4, 0xF3, 0xF2, 0xF1];
pub const STANDARD_TAIL: [u8; 4] = [0xF8, 0xF7, 0xF6, 0xF5];

// Command markers
pub const CMD_HEAD: [u8; 4] = [0xFD, 0xFC, 0xFB, 0xFA];
pub const CMD_TAIL: [u8; 4] = [0x04, 0x03, 0x02, 0x01];

// Largest frame on the wire: the buffer size the sensor reports in its enter-config ACK.
pub const MAX_FRAME_LEN: usize = 128;

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
			OutputMode::Standard => [0, 0, 1, 0, 0, 0],
		}
	}
}

#[derive(Clone, Debug)]
pub struct MinimalPacket {
	pub target_state: TargetState,
	pub distance_cm: u16,
}

/// Standard data frame (Table 2-1: data_type=0x01)
#[derive(Clone, Debug)]
pub struct StandardPacket {
	pub data_type: u8, // usually 0x01
	pub target_state: TargetState,
	pub distance_cm: u16,  // little-endian
	pub reserved: u16,     // 2 bytes after distance
	pub energy: [u32; 16], // energy per distance gate (u32 LE each)
}

#[derive(Clone, Debug)]
pub struct FirmwareVersion {
	pub equipment_type: u32,
	pub version_type: u16,
	pub major: u16,
	pub minor: u16,
	pub patch: u16,
}

#[derive(Clone, Debug)]
pub struct AckPacket {
	pub command: u16,
	pub success: bool,
	pub payload: Vec<u8, 64>, // largest reply: threshold readback, 16 x u32
}

#[derive(Clone, Debug)]
pub enum Packet {
	Minimal(MinimalPacket),
	Standard(StandardPacket),
	Firmware(FirmwareVersion),
	SerialNumber(Vec<u8, 32>),
	Ack(AckPacket),
	/// Auto-threshold progress (data type 0x03), raw value as reported (progress * 100).
	ThresholdProgress(u16),
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
