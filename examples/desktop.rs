use ld2410s::{BAUD_RATE, LD2410S, OutputMode, uart::SerialPortWrapper};
use std::time::Duration;

fn main() {
	let port = serialport::new("/dev/tty.usbserial-2110", BAUD_RATE)
		.timeout(Duration::from_millis(50))
		.open()
		.unwrap();

	// poll_timeout is 110ms as 100 is just hitting the cache every other read
	let mut sensor =
		LD2410S::new(SerialPortWrapper(port), Duration::from_millis(110), OutputMode::Standard);

	let _ = sensor.init();

	loop {
		if let Some(reading) = sensor.read_latest().unwrap() {
			if let Some(minimal_packet) = reading.data.as_minimal() {
				println!("minimal: {:?}", minimal_packet);
			}

			if let Some(standard_packet) = reading.data.as_standard() {
				println!("standaard: {:?}", standard_packet);
			}
		}

		std::thread::sleep(Duration::from_millis(100));
	}
}
