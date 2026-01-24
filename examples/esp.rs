use esp_idf_hal::prelude::*;
use esp_idf_hal::uart::{UartDriver, config::Config};
use ld2410s::OutputMode;
use ld2410s::{BAUD_RATE, LD2410S, uart::EspUartWrapper};
use std::time::Duration;

fn main() {
	// esp_idf_svc::log::EspLogger::initialize_default(); // optional logging

	let peripherals = Peripherals::take().unwrap();
	let pins = peripherals.pins;

	// Pick your actual TX/RX pins:
	let tx = pins.gpio4; // ESP TX -> LD2410S RX
	let rx = pins.gpio5; // ESP RX -> LD2410S TX

	let cfg = Config::default().baudrate(BAUD_RATE);

	// RTS/CTS unused:
	let uart = UartDriver::new(
		peripherals.uart1,
		tx,
		rx,
		Option::<esp_idf_hal::gpio::AnyIOPin>::None,
		Option::<esp_idf_hal::gpio::AnyIOPin>::None,
		&cfg,
	)?;

	// Optional: set a short RX timeout so read() returns quickly
	// (API exists on recent esp-idf-hal; if not in your version, you can skip)
	// uart.set_rx_timeout(Duration::from_millis(20))?;

	let mut sensor = LD2410S::new(EspUartWrapper(uart), OutputMode::Standard);

	// 1. Initialize the sensor (switches to the requested OutputMode).
	// This might take a second as it sends commands and waits for ACKs.
	if let Err(e) = sensor.init() {
		println!("Failed to init sensor: {:?}", e);
	}

	// 2. (Optional) Configure sensor parameters.
	// By default, the sensor might report at 1Hz. Here we set it to 8Hz for faster updates.
	// We also set the response speed to 'Fast' (10) to make it more reactive.
	println!("Configuring sensor parameters...");
	let _ = sensor.set_distance_frequency(8.0);
	let _ = sensor.set_status_frequency(8.0);
	let _ = sensor.set_response_speed(10);

	loop {
		// 3. Read the latest data.
		// read_latest() tries to fetch a fresh packet. If no fresh packet arrives within poll_timeout,
		// it returns the last cached packet (fresh=false).
		if let Some(r) = sensor.read_latest().unwrap() {
			if let Some(m) = r.data.as_minimal() {
				println!("Minimal (Fresh: {}): {:?}", r.fresh, m);
			}

			if let Some(s) = r.data.as_standard() {
				println!("Standard (Fresh: {}): {:?}", r.fresh, s);
			}
		}
		// small delay to keep loop polite; adjust to your needs
		std::thread::sleep(Duration::from_millis(100));
	}
}
