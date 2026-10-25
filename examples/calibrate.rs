//! Run the sensor's automatic threshold calibration and print its progress.
//! Keep the detection area empty (no people, nothing moving) while it runs.
//!
//!     cargo run --example calibrate --features serial -- /dev/tty.usbserial-XXX [seconds]

use ld2410s::{
	BAUD_RATE, LD2410S, OutputMode, Packet,
	uart::{SerialPortWrapper, StdTimer},
};
use std::time::{Duration, Instant};

fn main() {
	let mut args = std::env::args().skip(1);
	let port_name = args.next().expect("usage: calibrate <port> [seconds]");
	let seconds: u16 = args.next().map_or(120, |s| s.parse().expect("seconds must be a number"));

	let port = serialport::new(port_name, BAUD_RATE)
		.timeout(Duration::from_millis(50))
		.open()
		.expect("Failed to open serial port");
	let mut sensor =
		LD2410S::new(SerialPortWrapper(port), StdTimer::default(), OutputMode::Minimal);

	// Give the operator time to walk out of the detection area first.
	for left in (1..=15).rev() {
		println!("Leave the area: calibration starts in {left}s");
		std::thread::sleep(Duration::from_secs(1));
	}

	// Factors from the protocol PDF example (trigger 2, retention 1).
	println!("Starting calibration for {seconds}s...");
	sensor.set_auto_threshold(2, 1, seconds).expect("Failed to start calibration");

	let deadline = Instant::now() + Duration::from_secs(seconds as u64 + 30);
	while Instant::now() < deadline {
		match sensor.next_packet() {
			// The scale of the reported value is undocumented, so just log it until the deadline.
			Ok(Some(Packet::ThresholdProgress(p))) => println!("progress: {p}"),
			Ok(Some(Packet::Minimal(_) | Packet::Standard(_))) => {}
			Ok(Some(other)) => println!("{other:?}"),
			Ok(None) => std::thread::sleep(Duration::from_millis(10)),
			Err(e) => eprintln!("UART Error: {e:?}"),
		}
	}
	println!("Scan window over.");
}
