use crate::parser::parse_minimal_frames;
use crate::types::MinimalPacket;
use crate::uart::UartInterface;
use heapless::{Deque, Vec};
use std::time::{Duration, Instant};

const STREAM_BUF_CAP: usize = 1024; // ring buffer size

/// Returned by read_latest(): either fresh data or cached
#[derive(Clone, Copy, Debug)]
pub struct Reading {
    pub pkt: MinimalPacket,
    pub fresh: bool,
}

/// Driver owning a UART interface
pub struct LD2410S<U: UartInterface> {
    uart: U,
    stream_buf: Deque<u8, STREAM_BUF_CAP>,
    last: Option<MinimalPacket>,
    poll_timeout: Duration,
}

impl<U> LD2410S<U>
where
    U: UartInterface,
{
    /// Create a new LD2410S driver.
    /// `poll_timeout` is the maximum time `read_latest()` will wait for a fresh frame.
    pub fn new(uart: U, poll_timeout: Duration) -> Self {
        Self {
            uart,
            stream_buf: Deque::new(),
            last: None,
            poll_timeout,
        }
    }

    /// Non-blocking poll: reads whatever is available now, parses frames, updates cache.
    pub fn poll(&mut self, read_chunk: usize) -> Result<Vec<MinimalPacket, 16>, U::Error> {
        let mut tmp = [0u8; 256];
        let to_read = tmp.len().min(read_chunk);
        let n = self.uart.read(&mut tmp[..to_read])?;
        for &b in &tmp[..n] {
            let _ = self.stream_buf.push_back(b); // drop oldest if full
        }

        // parse from contiguous snapshot
        let mut linear = [0u8; STREAM_BUF_CAP];
        let len = self.stream_buf.len().min(STREAM_BUF_CAP);
        for (i, b) in self.stream_buf.iter().take(len).enumerate() {
            linear[i] = *b;
        }

        let (frames, consumed) = parse_minimal_frames(&linear[..len]);

        // consume parsed bytes from ring buffer
        for _ in 0..consumed {
            let _ = self.stream_buf.pop_front();
        }

        // update last-known packet
        if let Some(last) = frames.last().copied() {
            self.last = Some(last);
        }

        Ok(frames)
    }

    /// Wait up to the configured poll_timeout for a fresh frame;
    /// if none arrives, return cached snapshot if available.
    pub fn read_latest(&mut self) -> Result<Option<Reading>, U::Error> {
        let start = Instant::now();
        loop {
            let frames = self.poll(256)?;
            if let Some(pkt) = frames.last().copied() {
                self.last = Some(pkt);
                return Ok(Some(Reading { pkt, fresh: true }));
            }
            if start.elapsed() >= self.poll_timeout {
                return Ok(self.last.map(|pkt| Reading { pkt, fresh: false }));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Immediate snapshot of last-known data (if any)
    pub fn snapshot(&self) -> Option<MinimalPacket> {
        self.last
    }

    /// Send a raw command to the sensor
    pub fn send_command(&mut self, cmd: &[u8]) -> Result<(), U::Error> {
        self.uart.write_all(cmd)
    }
}
