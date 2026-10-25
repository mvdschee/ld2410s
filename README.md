# LD2410S Rust Driver

A **no_std**, **zero-heap** Rust library for reading and controlling the **HLK-LD2410S** 24GHz radar presence sensor over UART.

## ✨ Features

- **Portability**: Core driver is `no_std` and uses zero heap allocations (via `heapless`).
- **Flexible IO**: Supports **Blocking** (Sync) and **Async** (via `embedded-io-async`).
- **Backend Agnostic**: Works on desktop (via `serialport`) and embedded (via `esp-idf-hal`) through a simple trait abstraction.
- **Unified Parsing**: Built-in frame parsing for:
  - **Minimal Packets** (Presence state, Distance)
  - **Standard/Engineering Packets** (Distance, Signal Energy arrays)
  - **Firmware Version** & **Serial Number**
- **Full Configuration**:
  - Switch between Minimal and Standard modes.
  - Set reporting frequencies, response speeds, and gate thresholds.
  - Support for Automatic Calibration.

## 📚 Documentation

This library implements the protocol defined in the official HLK-LD2410S manuals:

- [HLK-LD2410S Serial Communication Protocol V1.00](./docs/HLK-LD2410S_serial_communication_protocol-V1.00.pdf)
- [HLK-LD2410S User Manual V1.2](./docs/HLK-LD2410S_User_manual-V1.2.pdf)

## 📦 Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
# Desktop Serial (Sync)
ld2410s = { version = "0.2.0", features = ["serial"] }
# Async Support
# ld2410s = { version = "0.2.0", features = ["async"] }
# Embedded ESP-IDF
# ld2410s = { version = "0.2.0", features = ["embedded"] }
```

## 🚀 Examples

### Desktop (Sync)

    cargo run --example serial --features serial -- /dev/tty.usbserial-123

```rs
	let port = serialport::new("/dev/tty.usbserial-123", BAUD_RATE)
		.timeout(Duration::from_millis(50))
		.open()?;

	// 1. Initialize sensor (Standard Mode = Engineering Data)
    // LD2410S::new(UART, TIMER, MODE)
	let mut sensor = LD2410S::new(SerialPortWrapper(port), StdTimer::default(), OutputMode::Standard);
	sensor.init()?;

	loop {
		if let Ok(Some(packet)) = sensor.next_packet() {
			match packet {
				ld2410s::Packet::Standard(s) => println!("Dist: {} Energy: {:?}", s.distance_cm, s.energy),
				_ => {}
			}
		}
	}
```

### Calibration

Runs the sensor's automatic threshold calibration. Keep the detection area empty while it runs (default 120 s).

    cargo run --example calibrate --features serial -- /dev/tty.usbserial-123 [seconds]

### Desktop (Async via Tokio)

    cargo run --example async_serial --features async

```rs
    let port = tokio_serial::new("/dev/tty.usbserial-123", BAUD_RATE).open_native_async()?;
    let mut sensor = LD2410SAsync::new(FromTokio::new(port), OutputMode::Standard);
    sensor.init().await?;

    loop {
        if let Ok(Some(packet)) = sensor.next_packet().await {
            // handle packet...
        }
    }
```

### ESP32 (Sync via `esp-idf-hal`)

    cargo run --example esp --features embedded

```rs
	let uart = UartDriver::new(peripherals.uart1, pins.gpio4, pins.gpio5, None, None, &cfg)?;
	let mut sensor = LD2410S::new(EspUartWrapper(uart), EspTimer, OutputMode::Standard);
	sensor.init()?;

	loop {
		if let Ok(Some(packet)) = sensor.next_packet() {
            // handle packet...
		}
	}
```

### ESP32 (Async via `esp-idf-hal`)

    cargo run --example async_esp --features embedded,async

```rs
	let uart = AsyncUartDriver::new(peripherals.uart1, pins.gpio4, pins.gpio5, None, None, &cfg)?;
	let mut sensor = LD2410SAsync::new(uart, OutputMode::Standard);

    block_on(async {
        sensor.init().await.unwrap();
        loop {
            if let Ok(Some(packet)) = sensor.next_packet().await {
                // handle packet...
            }
        }
    })
```

## ⚙️ Feature Flags

- `serial` → Use [serialport](https://crates.io/crates/serialport) (desktop/hosted).
- `embedded` → Use [esp-idf-hal](https://crates.io/crates/esp-idf-hal) (ESP32).
- `async` → Use [embedded-io-async](https://crates.io/crates/embedded-io-async) and [embassy-time](https://crates.io/crates/embassy-time).

## 📝 License

MIT License.
