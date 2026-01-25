use embedded_io_adapters::tokio_1::FromTokio;
use ld2410s::asynchronous::LD2410SAsync;
use ld2410s::{BAUD_RATE, OutputMode, Packet};
use std::time::Duration;
use tokio_serial::SerialPortBuilderExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let port_name = "/dev/tty.usbserial-130"; // Adjust to your port

	let mut port = tokio_serial::new(port_name, BAUD_RATE).open_native_async()?;

	#[cfg(unix)]
	port.set_exclusive(false)?;

	// Wrap the tokio serial port in an adapter that implements embedded-io-async traits
	let embedded_port = FromTokio::new(port);

	// Create the async driver
	let mut sensor = LD2410SAsync::new(embedded_port, OutputMode::Standard);

	println!("Initializing sensor...");
	sensor.init().await.expect("Failed to initialize sensor");

	println!("Configuring sensor...");
	sensor.set_distance_frequency(8.0).await.map_err(|e| anyhow::anyhow!("{:?}", e))?;
	sensor.set_status_frequency(8.0).await.map_err(|e| anyhow::anyhow!("{:?}", e))?;
	sensor.set_response_speed(10).await.map_err(|e| anyhow::anyhow!("{:?}", e))?;

	println!("Reading firmware version...");
	match sensor.read_firmware_version().await {
		Ok(v) => println!(
			"Firmware: Type={:08X} VerType={:04X} v{}.{}.{}",
			v.equipment_type, v.version_type, v.major, v.minor, v.patch
		),
		Err(e) => eprintln!("Error reading firmware: {:?}", e),
	}

	println!("Starting poll loop...");
	loop {
		// Read packets asynchronously
		match sensor.next_packet().await {
			Ok(Some(packet)) => match packet {
				Packet::Minimal(m) => println!("Minimal: {:?}", m),
				Packet::Standard(s) => println!("Standard: {:?}", s),
				Packet::Ack(ack) => println!("ACK: {:?}", ack),
				Packet::Firmware(v) => println!("Firmware: {:?}", v),
				Packet::SerialNumber(sn) => println!("Serial Number: {:?}", sn),
			},
			Ok(None) => {}
			Err(e) => eprintln!("UART Error: {:?}", e),
		}

		// Yield to let other tasks run (not strictly needed with await, but good practice in loops)
		tokio::time::sleep(Duration::from_millis(10)).await;
	}
}
