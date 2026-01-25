use ld2410s::{BAUD_RATE, LD2410S, OutputMode, uart::SerialPortWrapper};
use std::time::Duration;

fn main() {
	let port = serialport::new("/dev/tty.usbserial-123", BAUD_RATE)
		.timeout(Duration::from_millis(50))
		.open()
		.unwrap();

	let mut sensor = LD2410S::new(SerialPortWrapper(port), OutputMode::Standard);

	// 1. Initialize the sensor.
	// This sends the "Switch to Standard Output" command (0x007A) if Standard mode is requested.
	println!("Initializing sensor...");
	let _ = sensor.init();

	// 2. (Optional) Configure sensor parameters for faster updates.
	// The sensor defaults to ~1Hz. We set it to 8Hz here for smoother tracking.
	// Response speed 10 = Fast (default is 5 = Normal).
	println!("Configuring sensor...");
	let _ = sensor.set_distance_frequency(8.0);
	let _ = sensor.set_status_frequency(8.0);
	let _ = sensor.set_response_speed(10);

	// 3. Read diagnostic info (Firmware Version & Serial Number).
	println!("Reading firmware version...");
	match sensor.read_firmware_version() {
		Ok(Some(v)) => println!(
			"Firmware: Type={:08X} VerType={:04X} v{}.{}.{}",
			v.equipment_type, v.version_type, v.major, v.minor, v.patch
		),
		Ok(None) => println!("Firmware version: timeout"),
		Err(e) => eprintln!("Error reading firmware: {:?}", e),
	}

	println!("Reading serial number...");
	match sensor.read_serial_number() {
		Ok(Some(sn)) => {
			let sn_str = String::from_utf8_lossy(&sn);
			println!("Serial Number: {}", sn_str);
		}
		Ok(None) => println!("Serial Number: timeout"),
		Err(e) => eprintln!("Error reading serial number: {:?}", e),
	}

	loop {
		// 4. Main Loop: Read latest data packets.
		if let Some(reading) = sensor.read_latest().unwrap() {
			let prefix = if reading.fresh {
				"[FRESH]"
			} else {
				"[CACHED]"
			};
			match &reading.data {
				ld2410s::Packet::Minimal(m) => println!("{} Minimal: {:?}", prefix, m),
				ld2410s::Packet::Standard(s) => println!("{} Standard: {:?}", prefix, s),
				ld2410s::Packet::Ack(ack) => println!("{} ACK: {:?}", prefix, ack),
				ld2410s::Packet::Firmware(v) => {
					println!(
						"{} Firmware: Type={:08X} v{}.{}.{}",
						prefix, v.equipment_type, v.major, v.minor, v.patch
					)
				}
				ld2410s::Packet::SerialNumber(sn) => println!("{} Serial Number: {:?}", prefix, sn),
			}
		}

		std::thread::sleep(Duration::from_millis(100));
	}
}
