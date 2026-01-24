use crate::parser::{parse_command_responses, parse_standard_frames};
use crate::types::{CMD_HEAD, FirmwareVersion, MINIMAL_HEAD, Packet, STANDARD_HEAD};
use crate::uart::UartInterface;
use crate::{OutputMode, parser::parse_minimal_frames};
use crate::{Reading, commands};
use heapless::{Deque, Vec};
use std::thread::sleep;
use std::time::{Duration, Instant};

const STREAM_BUF_CAP: usize = 1024; // ring buffer size

#[derive(Debug, Clone, Copy)]
pub struct Config {
	pub poll_timeout: Duration,
	pub command_timeout: Duration,
	pub config_mode_delay: Duration,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			poll_timeout: Duration::from_millis(100),
			command_timeout: Duration::from_millis(1000),
			config_mode_delay: Duration::from_millis(250),
		}
	}
}

/// Driver owning a UART interface
pub struct LD2410S<U: UartInterface> {
	uart: U,
	stream_buf: Deque<u8, STREAM_BUF_CAP>,
	last: Option<Packet>,
	output_mode: OutputMode,
	config: Config,
}

impl<U> LD2410S<U>
where
	U: UartInterface,
{
	/// Create a new LD2410S driver.
	pub fn new(uart: U, output_mode: OutputMode) -> Self {
		Self {
			uart,
			stream_buf: Deque::new(),
			last: None,
			output_mode,
			config: Config::default(),
		}
	}

	pub fn set_config(&mut self, config: Config) {
		self.config = config;
	}

	/// Initialize the device by switching to the desired output mode.
	/// This does NOT set frequencies or speeds; use the setters for that.
	pub fn init(&mut self) -> Result<(), U::Error> {
		let cmd = commands::switch_output_frame(self.output_mode);
		self.run_config_transaction(&cmd)
	}

	pub fn set_distance_frequency(&mut self, hz: f32) -> Result<(), U::Error> {
		let val = (hz * 10.0) as u16;
		let cmd = commands::set_distance_freq_x10_frame(val);
		self.run_config_transaction(&cmd)
	}

	pub fn set_status_frequency(&mut self, hz: f32) -> Result<(), U::Error> {
		let val = (hz * 10.0) as u16;
		let cmd = commands::set_status_freq_x10_frame(val);
		self.run_config_transaction(&cmd)
	}

	/// Set response speed: 5 (Normal) or 10 (Fast)
	pub fn set_response_speed(&mut self, speed: u16) -> Result<(), U::Error> {
		let cmd = commands::set_response_speed_frame(speed);
		self.run_config_transaction(&cmd)
	}

	/// Helper to wrap a command in Enter/Exit config mode frames
	fn run_config_transaction(&mut self, cmd: &[u8]) -> Result<(), U::Error> {
		self.send_and_wait_ack(&commands::enter_config_frame())?;
		sleep(self.config.config_mode_delay);

		self.send_and_wait_ack(cmd)?;
		sleep(self.config.config_mode_delay);

		self.send_and_wait_ack(&commands::exit_config_frame())?;
		sleep(self.config.config_mode_delay);
		Ok(())
	}

	fn send_and_wait_ack(&mut self, cmd: &[u8]) -> Result<bool, U::Error> {
		self.uart.write_all(cmd)?;
		let start = Instant::now();
		while start.elapsed() < self.config.command_timeout {
			let packets = self.poll(256)?;
			for p in packets {
				if let Packet::Ack(ack) = p {
					// We could check if ack.command matches cmd, but cmd parsing is tricky here
					// without decoding the input cmd. For init sequence, any ACK is likely good enough
					// or we assume it's for the last command.
					return Ok(ack.success);
				}
			}
			sleep(Duration::from_millis(10));
		}
		Ok(false) // Timeout
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

		let mut packets: Vec<Packet, 16> = Vec::new();

		// Parse from the stream buffer continuously until we can't anymore
		loop {
			if self.stream_buf.len() < 4 {
				break;
			}

			// Snapshot the beginning of the buffer to check headers
			let mut head = [0u8; 4];
			let len = self.stream_buf.len();

			for (i, byte) in self.stream_buf.iter().take(4.min(len)).enumerate() {
				head[i] = *byte;
			}

			let mut parsed_packets = Vec::<Packet, 4>::new(); // small temporary batch

			// 1. Check for Command/ACK Header
			let consumed = if head == CMD_HEAD {
				// Copy relevant part to contiguous slice for parser
				let snapshot_len = len.min(256); // Limit to reasonable frame size check
				let mut snapshot = [0u8; 256];

				for (i, byte) in self.stream_buf.iter().take(snapshot_len).enumerate() {
					snapshot[i] = *byte;
				}

				let (p, c) = parse_command_responses(&snapshot[..snapshot_len]);

				if c > 0 {
					for pkt in p {
						let _ = parsed_packets.push(pkt);
					}

					c
				} else {
					// Header matches but incomplete frame, wait for more data
					break;
				}
			}
			// 2. Check for Standard Header
			else if head == STANDARD_HEAD {
				let snapshot_len = len.min(256);
				let mut snapshot = [0u8; 256];

				for (i, byte) in self.stream_buf.iter().take(snapshot_len).enumerate() {
					snapshot[i] = *byte;
				}

				let (p, c) = parse_standard_frames(&snapshot[..snapshot_len]);

				if c > 0 {
					for pkt in p {
						let _ = parsed_packets.push(pkt);
					}

					c
				} else {
					break;
				}
			}
			// 3. Check for Minimal Header (1 byte 0x6E)
			else if head[0] == MINIMAL_HEAD {
				let snapshot_len = len.min(16); // Minimal is only 5 bytes
				let mut snapshot = [0u8; 16];

				for (i, byte) in self.stream_buf.iter().take(snapshot_len).enumerate() {
					snapshot[i] = *byte;
				}

				let (p, c) = parse_minimal_frames(&snapshot[..snapshot_len]);

				if c > 0 {
					for pkt in p {
						let _ = parsed_packets.push(pkt);
					}

					c
				} else {
					// Might be incomplete minimal frame
					if len < 5 {
						break;
					} else {
						// Malformed? parse_minimal_frames usually consumes if it finds valid frame.
						// If it returned 0 consumed but we have >5 bytes, it's not a valid frame starting here.
						1 // Skip the 0x6E and try again
					}
				}
			}
			// 4. Unknown byte
			else {
				1
			};

			// Apply consumption
			for _ in 0..consumed {
				let _ = self.stream_buf.pop_front();
			}

			// Add parsed packets to result
			for pkt in parsed_packets {
				let _ = packets.push(pkt);
			}

			// If we didn't consume anything, break to avoid infinite loop on partial data
			if consumed == 0 {
				break;
			}
		}

		// Update last known state if data packet
		if let Some(last_packet) = packets.last().cloned() {
			match last_packet {
				Packet::Minimal(_) | Packet::Standard(_) => {
					self.last = Some(last_packet);
				}
				_ => {}
			}
		}

		Ok(packets)
	}

	/// Wait up to the configured poll_timeout for a fresh frame;
	/// if none arrives, return cached snapshot if available.
	pub fn read_latest(&mut self) -> Result<Option<Reading>, U::Error> {
		let start = Instant::now();
		loop {
			let frames = self.poll(256)?;
			// Search for the latest DATA packet
			let latest_data =
				frames.iter().rev().find(|p| matches!(p, Packet::Minimal(_) | Packet::Standard(_)));

			if let Some(data) = latest_data {
				self.last = Some(data.clone());
				return Ok(Some(Reading {
					data: data.clone(),
					fresh: true,
				}));
			}
			if start.elapsed() >= self.config.poll_timeout {
				return Ok(self.snapshot().map(|data| Reading {
					data,
					fresh: false,
				}));
			}
			std::thread::sleep(Duration::from_millis(2));
		}
	}

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

	pub fn read_firmware_version(&mut self) -> Result<Option<FirmwareVersion>, U::Error> {
		self.uart.write_all(&commands::enter_config_frame())?;
		sleep(self.config.config_mode_delay);

		let frame = commands::read_firmware_frame();
		self.uart.write_all(&frame)?;

		let mut result = None;
		let start = Instant::now();
		while start.elapsed() < self.config.command_timeout {
			let packets = self.poll(256)?;
			for p in packets {
				if let Packet::Firmware(v) = p {
					result = Some(v);
				}
			}
			if result.is_some() {
				break;
			}
			sleep(Duration::from_millis(10));
		}

		self.uart.write_all(&commands::exit_config_frame())?;
		sleep(self.config.config_mode_delay);

		Ok(result)
	}

	pub fn read_serial_number(&mut self) -> Result<Option<Vec<u8, 32>>, U::Error> {
		self.uart.write_all(&commands::enter_config_frame())?;
		sleep(self.config.config_mode_delay);

		let frame = commands::read_serial_frame();
		self.uart.write_all(&frame)?;

		let mut result = None;
		let start = Instant::now();
		while start.elapsed() < self.config.command_timeout {
			let packets = self.poll(256)?;
			for p in packets {
				if let Packet::SerialNumber(sn) = p {
					result = Some(sn);
				}
			}
			if result.is_some() {
				break;
			}
			sleep(Duration::from_millis(10));
		}

		self.uart.write_all(&commands::exit_config_frame())?;
		sleep(self.config.config_mode_delay);

		Ok(result)
	}
}
