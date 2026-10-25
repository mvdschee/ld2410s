//! Set the detection range and the no-one delay.
//!
//!     cargo run --example configure --features serial -- <port> <nearest_gate> <farthest_gate> <no_one_delay_s>
//!
//! Gates are ~0.7 m each, e.g. `0 6 10` detects 0..~4.2 m and reports "no one" 10 s after the last detection.

use ld2410s::{
	BAUD_RATE, LD2410S, OutputMode,
	uart::{SerialPortWrapper, StdTimer},
};
use std::time::Duration;

fn main() {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let [port_name, nearest, farthest, delay] = args.as_slice() else {
		panic!("usage: configure <port> <nearest_gate> <farthest_gate> <no_one_delay_s>");
	};

	let port = serialport::new(port_name, BAUD_RATE)
		.timeout(Duration::from_millis(50))
		.open()
		.expect("Failed to open serial port");
	let mut sensor =
		LD2410S::new(SerialPortWrapper(port), StdTimer::default(), OutputMode::Minimal);

	let (nearest, farthest) = (nearest.parse().unwrap(), farthest.parse().unwrap());
	sensor.set_gate_range(nearest, farthest).expect("Failed to set gate range");
	sensor.set_no_one_delay(delay.parse().unwrap()).expect("Failed to set no-one delay");
	println!("Range: gates {nearest}..={farthest}, no-one delay: {delay}s");
}
