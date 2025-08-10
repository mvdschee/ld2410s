use ld2410s::{BAUD_RATE, LD2410S, OutputMode, Packet, uart::SerialPortWrapper};
use std::time::Duration;

fn main() {
	let port = serialport::new("/dev/tty.usbserial-2110", BAUD_RATE)
		.timeout(Duration::from_millis(50))
		.open()
		.unwrap();

	// poll_timeout is 110ms as 100 is just hitting the cache every other read
	let mut sensor = LD2410S::new(SerialPortWrapper(port), Duration::from_millis(110));

	let _ = sensor.init(OutputMode::Standard);

	loop {
		if let Some(r) = sensor.read_latest().unwrap() {
			if let Some(m) = r.data.as_minimal() {
				println!("minimal: {:?}", m);
			}

			if let Some(s) = r.data.as_standard() {
				println!("standaard: {:?}", s);
			}
		}

		std::thread::sleep(Duration::from_millis(100));
	}
}
