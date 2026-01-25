use core::time::Duration;
use esp_idf_hal::prelude::*;
use esp_idf_hal::uart::{UartDriver, config::Config};
use ld2410s::OutputMode;
use ld2410s::{
	BAUD_RATE, LD2410S,
	uart::{EspTimer, EspUartWrapper},
};

fn main() -> anyhow::Result<()> {
	// esp_idf_svc::log::EspLogger::initialize_default(); // optional logging

	let peripherals = Peripherals::take().unwrap();
	let pins = peripherals.pins;

	// Pick your actual TX/RX pins:
	let tx = pins.gpio4; // ESP TX -> LD2410S RX
	let rx = pins.gpio5; // ESP RX -> LD2410S TX

	let cfg = Config::default().baudrate(BAUD_RATE.Hz());

	// RTS/CTS unused:
	let uart = UartDriver::new(
		peripherals.uart1,
		tx,
		rx,
		Option::<esp_idf_hal::gpio::AnyIOPin>::None,
		Option::<esp_idf_hal::gpio::AnyIOPin>::None,
		&cfg,
	)?;

	// LD2410S now requires a Timer implementation (EspTimer for ESP32)
	let mut sensor = LD2410S::new(EspUartWrapper(uart), EspTimer, OutputMode::Standard);

	// 1. Initialize the sensor (switches to the requested OutputMode).
	println!("Initializing sensor...");
	if let Err(e) = sensor.init() {
		println!("Failed to init sensor: {:?}", e);
	}

	// 2. (Optional) Configure sensor parameters.
	println!("Configuring sensor parameters...");
	let _ = sensor.set_distance_frequency(8.0);
	let _ = sensor.set_status_frequency(8.0);
	let _ = sensor.set_response_speed(10);

	loop {
		// 3. Read packets.
		match sensor.next_packet() {
			Ok(Some(packet)) => {
				if let Some(m) = packet.as_minimal() {
					println!("Minimal: {:?}", m);
				}

				if let Some(s) = packet.as_standard() {
					println!("Standard: {:?}", s);
				}
			}
			Ok(None) => {}
			Err(e) => println!("UART Error: {:?}", e),
		}

		// small delay to keep loop polite
		esp_idf_hal::delay::FreeRtos::delay_ms(10);
	}
}
