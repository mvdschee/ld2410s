# Changelog

## 0.3.0 - 2026-11-08

### Breaking

- `embedded` feature: esp-idf-hal 0.45 to 0.47. `EspUartWrapper` wraps the 0.47 `UartDriver`, so firmware on esp-idf-hal 0.45 has to upgrade too. esp-idf-hal 0.46 removed its `prelude`, so the ESP examples now import `peripherals::Peripherals` and `units::*` instead.

### Changed

- Less RAM. The UART now reads straight into a 256 B parse buffer, It replaces the 1024 B ring buffer, the 256 B stack copy made for every parse, and the 128 B read buffer. Measured on an ESP32-C3:
  - `LD2410S`: 1088 B to 320 B
  - `LD2410SAsync`: 1096 B to 320 B
  - stack per sync `next_packet`: about 1950 B to 330 B
  - async `next_packet` future: 272 B to 144 B
- Parsing a frame takes 2 to 4 times fewer CPU cycles on the same C3.
- Minimum dependency versions: heapless 0.9.3, embassy-time 0.5.1, serialport 4.10.1.
- The published crate no longer includes `.github/` and the `makefile`.
