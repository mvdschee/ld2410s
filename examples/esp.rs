use ld2410s::{LD2410S, BAUD_RATE, uart::EspUartWrapper};
use esp_idf_hal::prelude::*;
use esp_idf_hal::uart::{config::Config, UartDriver};
use std::time::Duration;

fn main() {
    esp_idf_svc::log::EspLogger::initialize_default(); // optional logging

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

    let mut sensor = LD2410S::new(EspUartWrapper(uart), Duration::from_millis(100));

    loop {
        if let Some(r) = sensor.read_latest().unwrap() {
            println!(
                "{}: signal={} dist={}cm",
                if r.fresh { "fresh" } else { "cached" },
                r.pkt.signal,
                r.pkt.distance_cm
            );
        }
        // small delay to keep loop polite; adjust to your needs
        std::thread::sleep(Duration::from_millis(100));
    }
}
