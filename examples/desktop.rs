use std::time::Duration;
use ld2410s::{LD2410S, uart::SerialPortWrapper, BAUD_RATE};

fn main() {
    let port = serialport::new("/dev/tty.usbserial-2110", BAUD_RATE)
        .timeout(Duration::from_millis(50))
        .open().unwrap();

    let mut sensor = LD2410S::new(SerialPortWrapper(port), Duration::from_millis(100));

    loop {
        if let Some(r) = sensor.read_latest().unwrap() {
            println!(
                "{}: signal={} dist={}cm",
                if r.fresh { "fresh" } else { "cached" },
                r.pkt.signal,
                r.pkt.distance_cm
            );
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
