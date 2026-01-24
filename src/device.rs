use crate::parser::parse_standard_frames;
use crate::types::Packet;
use crate::uart::UartInterface;
use crate::{OutputMode, parser::parse_minimal_frames};
use crate::{Reading, commands};
use heapless::{Deque, Vec};
use std::thread::sleep;
use std::time::{Duration, Instant};

const STREAM_BUF_CAP: usize = 1024; // ring buffer size

/// Driver owning a UART interface
pub struct LD2410S<U: UartInterface> {
	uart: U,
	stream_buf: Deque<u8, STREAM_BUF_CAP>,
	last: Option<Packet>,
	output_mode: OutputMode,
	poll_timeout: Duration,
}

impl<U> LD2410S<U>
where
	U: UartInterface,
{
	/// Create a new LD2410S driver.
	/// `poll_timeout` is the maximum time `read_latest()` will wait for a fresh frame.
	/// `output_mode` is the initial output mode of the device.
	pub fn new(uart: U, poll_timeout: Duration, output_mode: OutputMode) -> Self {
		Self {
			uart,
			stream_buf: Deque::new(),
			last: None,
			output_mode,
			poll_timeout,
		}
	}

	/// Initialize the device with the given output mode.
	pub fn init(&mut self) -> Result<(), U::Error> {
		let [enter, switch, set_freq, exit] = commands::sequence_switch_to_standard_and_8hz();

		self.uart.write_all(&enter)?;
		self.read_for_and_dump(Duration::from_millis(100), "after ENTER")?;

		self.uart.write_all(&switch)?;
		self.read_for_and_dump(Duration::from_millis(200), "after SWITCH")?;

		self.uart.write_all(&set_freq)?;
		self.read_for_and_dump(Duration::from_millis(200), "after SET DIST FREQ 8Hz")?;

		self.uart.write_all(&exit)?;
		self.read_for_and_dump(Duration::from_millis(200), "after EXIT")?;

		self.read_for_and_dump(Duration::from_millis(2000), "read_for_and_dump for 2s")?;

		self.read_for_and_dump(Duration::from_millis(2000), "read_for_and_dump for 4s")?;

		self.read_for_and_dump(Duration::from_millis(2000), "read_for_and_dump for 6s")?;

		Ok(())
	}

	/// Non-blocking poll: reads whatever is available now, parses frames, updates cache.
	pub fn poll(&mut self, read_chunk: usize) -> Result<Vec<Packet, 16>, U::Error> {
		let mut read_buffer = [0u8; 256];
		let bytes_to_read = read_buffer.len().min(read_chunk);
		let bytes_read = self.uart.read(&mut read_buffer[..bytes_to_read])?;

		for &byte in &read_buffer[..bytes_read] {
			if self.stream_buf.push_back(byte).is_err() {
				let _ = self.stream_buf.pop_front();
				let _ = self.stream_buf.push_back(byte);
			}
		}

		// snapshot to contiguous
		let mut snapshot = [0u8; STREAM_BUF_CAP];
		let snapshot_len = self.stream_buf.len().min(STREAM_BUF_CAP);
		for (idx, byte) in self.stream_buf.iter().take(snapshot_len).enumerate() {
			snapshot[idx] = *byte;
		}

		// parse based on mode, map to Packet here so both branches have same type
		let bytes_consumed;
		let mut packets: Vec<Packet, 16> = Vec::new();

		if self.output_mode == OutputMode::Standard {
			let (std_packets, consumed_bytes) = parse_standard_frames(&snapshot[..snapshot_len]);
			bytes_consumed = consumed_bytes;
			for packet in std_packets.into_iter() {
				let _ = packets.push(packet); // Packet::Standard
			}
		} else {
			let (min_packets, consumed_bytes) = parse_minimal_frames(&snapshot[..snapshot_len]);
			bytes_consumed = consumed_bytes;
			for packet in min_packets.into_iter() {
				let _ = packets.push(packet); // Packet::Minimal
			}
		}

		// consume parsed bytes from ring buffer
		for _ in 0..bytes_consumed {
			let _ = self.stream_buf.pop_front();
		}

		// update last-known packet (if any)
		if let Some(last_packet) = packets.last().cloned() {
			self.last = Some(last_packet);
		}

		Ok(packets)
	}

	/// Wait up to the configured poll_timeout for a fresh frame;
	/// if none arrives, return cached snapshot if available.
	pub fn read_latest(&mut self) -> Result<Option<Reading>, U::Error> {
		let start = Instant::now();
		loop {
			let frames = self.poll(256)?;
			if let Some(data) = frames.last() {
				self.last = Some(data.clone());
				return Ok(Some(Reading {
					data: data.clone(),
					fresh: true,
				}));
			}
			if start.elapsed() >= self.poll_timeout {
				return Ok(self.snapshot().map(|data| Reading {
					data,
					fresh: false,
				}));
			}
			std::thread::sleep(Duration::from_millis(2));
		}
	}

	/// Immediate snapshot of last-known data (if any)
	pub fn snapshot(&self) -> Option<Packet> {
		self.last.clone()
	}

	/// Send a raw command to the sensor
	pub fn send_command(&mut self, cmd: &[u8]) -> Result<(), U::Error> {
		self.uart.write_all(cmd)
	}

	pub fn set_output_mode(&mut self, mode: OutputMode) -> Result<(), U::Error> {
		let frame = commands::switch_output_frame(mode);
		self.uart.write_all(&frame)
	}

	/// Read for `duration`, log raw bytes, and also append them to the ring buffer.
	/// (So your normal `poll()`/parsers can still consume them later.)
	fn read_for_and_dump(
		&mut self,
		duration: std::time::Duration,
		label: &str,
	) -> Result<(), U::Error> {
		use std::time::Instant;

		let start = Instant::now();
		let mut collected: Vec<u8, 16> = Vec::new();
		let mut buf = [0u8; 256];

		while start.elapsed() < duration {
			let n = self.uart.read(&mut buf)?;
			if n > 0 {
				collected.extend_from_slice(&buf[..n]);
				// keep bytes in the stream buffer too
				for &byte in &buf[..n] {
					if self.stream_buf.push_back(byte).is_err() {
						let _ = self.stream_buf.pop_front();
						let _ = self.stream_buf.push_back(byte);
					}
				}
			} else {
				std::thread::sleep(std::time::Duration::from_millis(2));
			}
		}

		if collected.is_empty() {
			eprintln!("[ld2410s][{label}] no bytes received in {:?}", duration);
		} else {
			dump_hex(label, &collected);
		}
		Ok(())
	}
}

fn dump_hex(label: &str, bytes: &[u8]) {
	eprintln!("[ld2410s][{label}] {} bytes", bytes.len());
	for chunk in bytes.chunks(16) {
		// hex
		let mut hex = String::new();
		for b in chunk {
			hex.push_str(&format!("{:02X} ", b));
		}
		// ascii
		let mut ascii = String::new();
		for b in chunk {
			let c = if (0x20..=0x7E).contains(b) {
				*b as char
			} else {
				'.'
			};
			ascii.push(c);
		}
		eprintln!("{:48} | {}", hex, ascii);
	}
}
