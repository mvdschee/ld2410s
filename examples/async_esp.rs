use esp_idf_hal::prelude::*;
use esp_idf_hal::task::block_on;
use esp_idf_hal::uart::{AsyncUartDriver, config::Config};
use ld2410s::asynchronous::LD2410SAsync;
use ld2410s::{BAUD_RATE, OutputMode, Packet};

fn main() -> anyhow::Result<()> {
	esp_idf_hal::sys::link_patches();

	let peripherals = Peripherals::take().unwrap();
	let pins = peripherals.pins;
	let tx = pins.gpio4;
	let rx = pins.gpio5;
	let cfg = Config::default().baudrate(BAUD_RATE.Hz());

	// Initialize Async UART
	let uart = AsyncUartDriver::new(
		peripherals.uart1,
		tx,
		rx,
		Option::<esp_idf_hal::gpio::AnyIOPin>::None,
		Option::<esp_idf_hal::gpio::AnyIOPin>::None,
		&cfg,
	)?;

	// Create Async Driver
	// AsyncUartDriver implements embedded_io_async::Read and Write
	let mut sensor = LD2410SAsync::new(uart, OutputMode::Standard);

	// Run the async task
	block_on(async {
		println!("Initializing sensor...");
		sensor.init().await.expect("Failed to init");

		println!("Configuring sensor...");
		sensor.set_distance_frequency(8.0).await.expect("Failed to config");
		sensor.set_status_frequency(8.0).await.expect("Failed to config");
		sensor.set_response_speed(10).await.expect("Failed to config");

		loop {
			match sensor.next_packet().await {
				Ok(Some(packet)) => match packet {
					Packet::Minimal(m) => println!("Minimal: {:?}", m),
					Packet::Standard(s) => println!("Standard: {:?}", s),
					_ => {}
				},
				Ok(None) => {}
				Err(e) => println!("Error: {:?}", e),
			}
		}
	})
}
