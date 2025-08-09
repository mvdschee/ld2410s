# LD2410S Rust Driver

A Rust library for reading and controlling the **HLK-LD2410S** 24GHz radar presence sensor over UART.
Supports both **desktop** (via [serialport](https://crates.io/crates/serialport)) and **embedded** (via [esp-idf-hal](https://crates.io/crates/esp-idf-hal)) targets.

---

## ✨ Features

- Works with **desktop** and **ESP-IDF** environments using the same API.
- **Unified UART interface** so your application doesn’t need to handle serial quirks.
- Built-in **frame parsing** for minimal packets (`signal`, `distance_cm`).
- Automatic caching of last reading if no fresh data is available.
- Configurable **poll timeout**.

---

## 📦 Installation

Add to your `Cargo.toml`:

```toml
    [dependencies]
    ld2410s = { version = "0.1.0", features = ["serial"] }    # desktop serialport
    # ld2410s = { version = "0.1.0", features = ["embedded"] } # embedded ESP-IDF
```

---

## 🚀 Examples

### Desktop (via USB-to-UART adapter)

    cargo run --example desktop --features serial

Example code:

```rs
    use ld2410s::{LD2410S, BAUD_RATE, uart::SerialPortWrapper};
    use std::time::Duration;

    fn main() -> anyhow::Result<()> {
        let port_name = "/dev/tty.usbserial-2120";
        let port = serialport::new(port_name, BAUD_RATE)
            .timeout(Duration::from_millis(50))
            .open()?;

        let mut dev = LD2410S::new(SerialPortWrapper(port), Duration::from_millis(100));

        loop {
            if let Some(r) = dev.read_latest()? {
                println!(
                    "{}: signal={} dist={}cm",
                    if r.fresh { "fresh" } else { "cached" },
                    r.pkt.signal,
                    r.pkt.distance_cm
                );
            }
        }
    }
```

---

### ESP32 (via `esp-idf-hal`)

    cargo run --example esp --features embedded

Example code:

```rs
    use ld2410s::{LD2410S, BAUD_RATE, uart::EspUartWrapper};
    use esp_idf_hal::prelude::*;
    use esp_idf_hal::uart::{config::Config, UartDriver};
    use std::time::Duration;

    fn main() -> anyhow::Result<()> {
        let peripherals = Peripherals::take().unwrap();
        let pins = peripherals.pins;

        let cfg = Config::default().baudrate(BAUD_RATE.Hz());
        let uart = UartDriver::new(
            peripherals.uart1,
            pins.gpio4, // TX
            pins.gpio5, // RX
            None, None,
            &cfg,
        )?;

        let mut dev = LD2410S::new(EspUartWrapper(uart), Duration::from_millis(100));

        loop {
            if let Some(r) = dev.read_latest()? {
                println!(
                    "{}: signal={} dist={}cm",
                    if r.fresh { "fresh" } else { "cached" },
                    r.pkt.signal,
                    r.pkt.distance_cm
                );
            }
        }
    }
```

---

## ⚙️ Feature Flags

- `serial` → Use [serialport](https://crates.io/crates/serialport) (desktop/hosted)
- `embedded` → Use [esp-idf-hal](https://crates.io/crates/esp-idf-hal) (ESP32)

---

## 📝 License

MIT License.
