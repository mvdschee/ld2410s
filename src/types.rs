/// Common constants for LD2410S
pub const BAUD_RATE: u32 = 115_200u32;

/// Minimal frame markers (engineering/simple mode)
pub const MINIMAL_HEAD: u8 = 0x6E;
pub const MINIMAL_TAIL: u8 = 0x62;

/// A minimal parsed packet (from 6E .. 62)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MinimalPacket {
    /// raw signal / presence strength
    pub signal: u8,
    /// distance in cm (u16 little endian)
    pub distance_cm: u16,
}
