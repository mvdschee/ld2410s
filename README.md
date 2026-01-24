# LD2410S Rust Driver

A Rust library for reading and controlling the **HLK-LD2410S** 24GHz radar presence sensor over UART.
Supports both **desktop** (via [serialport](https://crates.io/crates/serialport)) and **embedded** (via [esp-idf-hal](https://crates.io/crates/esp-idf-hal)) targets.

## ✨ Features

- Works with **desktop** and **ESP-IDF** environments using the same API.
- **Unified UART interface** so your application doesn’t need to handle serial quirks.
- Built-in **frame parsing** for:
  - **Minimal Packets** (Presence state, Distance)
  - **Standard/Engineering Packets** (Distance, Signal Energy arrays)
  - **Firmware Version** & **Serial Number**
- **Configuration Support**:
  - Switch between Minimal and Standard/Engineering modes.
  - Configure reporting frequency (e.g., 8Hz).
  - Configure response speed.
- Automatic caching of last reading if no fresh data is available.

## 📚 Documentation

This library implements the protocol defined in the official HLK-LD2410S manuals:

- [HLK-LD2410S Serial Communication Protocol V1.00](./docs/HLK-LD2410S_serial_communication_protocol-V1.00.pdf) (2024-08-23)
- [HLK-LD2410S User Manual V1.2](./docs/HLK-LD2410S_User_manual-V1.2.pdf) (2023-10-31)

## 📦 Installation

Add to your `Cargo.toml`:

```toml
    [dependencies]
    ld2410s = { version = "0.1.0", features = ["serial"] }    # desktop serialport
    # ld2410s = { version = "0.1.0", features = ["embedded"] } # embedded ESP-IDF
```

## 🚀 Examples

### Desktop (via USB-to-UART adapter)

    cargo run --example desktop --features serial

```rs
	let port = serialport::new("/dev/tty.usbserial-140", BAUD_RATE)
		.timeout(Duration::from_millis(50))
		.open()?;

	// 1. Initialize sensor (Standard Mode = Engineering Data)
	let mut sensor = LD2410S::new(SerialPortWrapper(port), OutputMode::Standard);
	sensor.init()?;

	// 2. Configure for faster updates (8Hz)
	sensor.set_distance_frequency(8.0)?;
	sensor.set_status_frequency(8.0)?;
	sensor.set_response_speed(10)?;

	loop {
		if let Some(reading) = sensor.read_latest()? {
			match reading.data {
				ld2410s::Packet::Standard(s) => println!("Dist: {} Energy: {:?}", s.distance_cm, s.energy),
				_ => {}
			}
		}
	}
```

### ESP32 (via `esp-idf-hal`)

    cargo run --example esp --features embedded

See `examples/esp.rs` for full implementation.

## ⚙️ Feature Flags

- `serial` → Use [serialport](https://crates.io/crates/serialport) (desktop/hosted)
- `embedded` → Use [esp-idf-hal](https://crates.io/crates/esp-idf-hal) (ESP32)

## 📝 License

MIT License.
