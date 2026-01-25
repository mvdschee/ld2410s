use ld2410s::{
	BAUD_RATE, LD2410S, OutputMode,
	uart::{SerialPortWrapper, StdTimer},
};
use std::time::Duration;

fn main() {
	let port = serialport::new("/dev/tty.usbserial-130", BAUD_RATE)
		.timeout(Duration::from_millis(50))
		.open()
		.expect("Failed to open serial port");

	// Now requires a Timer implementation (StdTimer for desktop)
	let mut sensor =
		LD2410S::new(SerialPortWrapper(port), StdTimer::default(), OutputMode::Standard);

	// 1. Initialize the sensor.
	println!("Initializing sensor...");
	sensor.init().expect("Failed to initialize sensor");

	// 2. Configure sensor parameters for faster updates.
	println!("Configuring sensor...");
	let _ = sensor.set_distance_frequency(8.0);
	let _ = sensor.set_status_frequency(8.0);
	let _ = sensor.set_response_speed(10);

	// 3. Read diagnostic info (Firmware Version & Serial Number).
	println!("Reading firmware version...");
	match sensor.read_firmware_version() {
		Ok(v) => println!(
			"Firmware: Type={:08X} VerType={:04X} v{}.{}.{}",
			v.equipment_type, v.version_type, v.major, v.minor, v.patch
		),
		Err(e) => eprintln!("Error reading firmware: {:?}", e),
	}

	println!("Reading serial number...");
	match sensor.read_serial_number() {
		Ok(sn) => {
			let sn_str = String::from_utf8_lossy(&sn);
			println!("Serial Number: {}", sn_str);
		}
		Err(e) => eprintln!("Error reading serial number: {:?}", e),
	}

	loop {
		// 4. Main Loop: Read packets.
		// next_packet() is now a non-blocking poll of the UART.
		match sensor.next_packet() {
			Ok(Some(packet)) => match packet {
				ld2410s::Packet::Minimal(m) => println!("Minimal: {:?}", m),
				ld2410s::Packet::Standard(s) => println!("Standard: {:?}", s),
				ld2410s::Packet::Ack(ack) => println!("ACK: {:?}", ack),
				ld2410s::Packet::Firmware(v) => {
					println!(
						"Firmware: Type={:08X} v{}.{}.{}",
						v.equipment_type, v.major, v.minor, v.patch
					)
				}
				ld2410s::Packet::SerialNumber(sn) => println!("Serial Number: {:?}", sn),
			},
			Ok(None) => {
				// No packet ready yet
			}
			Err(e) => eprintln!("UART Error: {:?}", e),
		}

		std::thread::sleep(Duration::from_millis(10));
	}
}
