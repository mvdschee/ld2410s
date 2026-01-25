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

- [HLK-LD2410S Serial Communication Protocol V1.00](./docs/HLK-LD2410S_serial_communication_protocol-V1.00.pdf) (26 Nov 2024)
- [HLK-LD2410S User Manual V1.2](./docs/HLK-LD2410S_User_manual-V1.2.pdf) (20 Apr 2024)

## 📦 Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ld2410s = { git = "https://github.com/mvdschee/ld2410s", tag = "v0.1.1", features = ["serial"] }    # desktop serialport
# ld2410s = { git = "https://github.com/mvdschee/ld2410s", tag = "v0.1.1", features = ["embedded"] } # embedded ESP-IDF
```

## 🚀 Examples

### Desktop (via USB-to-UART adapter)

    cargo run --example desktop --features serial

```rs
	let port = serialport::new("/dev/tty.usbserial-123", BAUD_RATE)
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

```rs
	let peripherals = Peripherals::take().unwrap();
	let pins = peripherals.pins;
	let cfg = Config::default().baudrate(BAUD_RATE.Hz());
	let uart = UartDriver::new(
		peripherals.uart1,
		pins.gpio4,
		pins.gpio5,
		None, None,
		&cfg,
	)?;

	let mut sensor = LD2410S::new(EspUartWrapper(uart), OutputMode::Standard);
	sensor.init()?;
	sensor.set_distance_frequency(8.0)?;

	loop {
		if let Some(reading) = sensor.read_latest()? {
			match reading.data {
				ld2410s::Packet::Standard(s) => println!("Dist: {} Energy: {:?}", s.distance_cm, s.energy),
				_ => {}
			}
		}
	}
```

## ⚙️ Advanced Configuration

### Manual Thresholds

You can manually set the sensitivity (energy threshold) for each of the 16 distance gates.

- **Trigger Threshold**: Energy required to switch from Unoccupied -> Occupied.
- **Hold Threshold**: Energy required to maintain Occupied state.

```rs
// Set gates 0-15. Lower value = Higher sensitivity.
// Example: High sensitivity for close range (gates 0-2), lower for far (3-15).
let triggers: [u16; 16] = [
    15, 15, 20, 30, 40, 50, 60, 60,
    60, 60, 60, 60, 60, 60, 60, 60
];
sensor.set_trigger_thresholds(&triggers)?;

// Hold thresholds are usually slightly lower than trigger to prevent flickering
let holds: [u16; 16] = [
    10, 10, 15, 25, 35, 45, 55, 55,
    55, 55, 55, 55, 55, 55, 55, 55
];
sensor.set_hold_thresholds(&holds)?;
```

### Automatic Calibration

Use this **once** during installation with an **empty room**. The sensor will measure background noise and set thresholds automatically.

```rs
// 1. Trigger Factor (added to noise floor for Trigger)
// 2. Retention Factor (added to noise floor for Hold)
// 3. Scanning Time (seconds)
// Example: factor=2, retention=1, scan=120s
sensor.set_auto_threshold(2, 1, 120)?;
```

## ⚙️ Feature Flags

- `serial` → Use [serialport](https://crates.io/crates/serialport) (desktop/hosted)
- `embedded` → Use [esp-idf-hal](https://crates.io/crates/esp-idf-hal) (ESP32)

## 📝 License

MIT License.
