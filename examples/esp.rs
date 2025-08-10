use esp_idf_hal::prelude::*;
use esp_idf_hal::uart::{UartDriver, config::Config};
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

	// poll_timeout is 110ms as 100 is just hitting the cache every other read
	let mut sensor = LD2410S::new(EspUartWrapper(uart), Duration::from_millis(110));

	loop {
		if let Some(r) = sensor.read_latest().unwrap() {
			if let Some(m) = r.data.as_minimal() {
				println!("minimal: {:?}", m);
			}

			if let Some(s) = r.data.as_standard() {
				println!("standaard: {:?}", s);
			}
		}
		// small delay to keep loop polite; adjust to your needs
		std::thread::sleep(Duration::from_millis(100));
	}
}
