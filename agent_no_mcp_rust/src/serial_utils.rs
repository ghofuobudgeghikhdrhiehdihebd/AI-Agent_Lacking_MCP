use std::error::Error;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

use serde::{Serialize, de::DeserializeOwned};

pub const USB_SERIAL_PATH: &str = "/dev/ttyGS0";
pub const USB_SERIAL_BAUD_RATE: u32 = 9600;
pub const MAX_FRAME_LENGTH: usize = 1024 * 1024;

const READ_POLL_INTERVAL: Duration = Duration::from_millis(50);
const READ_BUFFER_SIZE: usize = 1024;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Direct USB serial connection to the Arduino board.
///
/// The board receives and sends length-prefixed frames using the format
/// `[4-byte big-endian payload length][payload]`.
pub struct ArduinoSerial {
    port: Box<dyn serialport::SerialPort>,
    pending_rx: Vec<u8>,
}

impl ArduinoSerial {
    /// Open the Arduino USB CDC serial device at the default baud rate.
    pub fn new() -> Result<Self> {
        Self::open(USB_SERIAL_PATH, USB_SERIAL_BAUD_RATE)
    }

    /// Open a USB serial device with an explicit path and baud rate.
    ///
    /// USB CDC devices commonly ignore the configured baud rate, but it is
    /// supplied for compatibility with serial device drivers and the board's
    /// USB serial implementation.
    pub fn open(path: &str, baud_rate: u32) -> Result<Self> {
        let port = serialport::new(path, baud_rate)
            .timeout(READ_POLL_INTERVAL)
            .open()?;

        Ok(Self {
            port,
            pending_rx: Vec::new(),
        })
    }

    /// Send arbitrary bytes directly to the board over USB serial.
    pub fn send(&mut self, data: &[u8]) -> Result<()> {
        self.port.write_all(data)?;
        self.port.flush()?;
        Ok(())
    }

    /// Read bytes currently available from USB serial.
    ///
    /// Returns an empty vector if no new bytes arrive during one read-poll
    /// interval, along with any bytes buffered by `receive_exact`.
    pub fn receive(&mut self) -> Result<Vec<u8>> {
        let mut data = std::mem::take(&mut self.pending_rx);
        data.extend(self.read_serial_chunk()?);
        Ok(data)
    }

    /// Receive one unframed serial message.
    ///
    /// This is intended for plain `pyserial.Serial.write()` traffic, where the
    /// sender does not prepend the 4-byte length header required by
    /// `receive_frame`. The first byte must arrive before `timeout`; after data
    /// begins arriving, the message ends when no new bytes arrive for
    /// `idle_timeout`.
    ///
    /// Raw serial has no inherent message boundaries. Use `receive_frame` for
    /// protocol traffic that needs unambiguous message framing.
    pub fn receive_until_idle(
        &mut self,
        timeout: Duration,
        idle_timeout: Duration,
    ) -> Result<Vec<u8>> {
        if idle_timeout.is_zero() {
            return Err("USB serial idle timeout must be greater than zero".into());
        }

        let deadline = Instant::now() + timeout;
        let mut data = std::mem::take(&mut self.pending_rx);
        let mut last_received_at = (!data.is_empty()).then(Instant::now);

        loop {
            let now = Instant::now();
            if let Some(last_received_at) = last_received_at {
                if now.duration_since(last_received_at) >= idle_timeout {
                    return Ok(data);
                }
            } else if now >= deadline {
                return Err(receive_timeout().into());
            }

            let chunk = self.read_serial_chunk()?;
            if !chunk.is_empty() {
                data.extend(chunk);
                last_received_at = Some(Instant::now());
            }
        }
    }

    /// Wait until exactly `count` bytes have arrived.
    ///
    /// Returns a timeout error if insufficient data arrives before `timeout`.
    pub fn receive_exact(&mut self, count: usize, timeout: Duration) -> Result<Vec<u8>> {
        let deadline = Instant::now() + timeout;

        while self.pending_rx.len() < count {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(receive_timeout().into());
            }

            let chunk = self.read_serial_chunk()?;
            if !chunk.is_empty() {
                self.pending_rx.extend(chunk);
            }
        }

        let remaining = self.pending_rx.split_off(count);
        let result = std::mem::replace(&mut self.pending_rx, remaining);
        Ok(result)
    }

    /// Send UTF-8 text.
    pub fn send_text(&mut self, text: &str) -> Result<()> {
        self.send(text.as_bytes())
    }

    /// Serialize a value as MessagePack and transmit it.
    pub fn send_msgpack<T: Serialize>(&mut self, value: &T) -> Result<()> {
        let encoded = rmp_serde::to_vec_named(value)?;
        self.send(&encoded)
    }

    /// Serialize a value as MessagePack and transmit it in one framed message.
    pub fn send_msgpack_frame<T: Serialize>(&mut self, value: &T) -> Result<()> {
        let encoded = rmp_serde::to_vec_named(value)?;
        self.send_frame(&encoded)
    }

    /// Send a length-prefixed frame.
    ///
    /// Format: `[4-byte length (big endian)][payload]`.
    pub fn send_frame(&mut self, data: &[u8]) -> Result<()> {
        self.send(&encode_frame(data)?)
    }

    /// Receive one length-prefixed frame.
    pub fn receive_frame(&mut self, timeout: Duration, max_length: usize) -> Result<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        let length_bytes =
            self.receive_exact(4, deadline.saturating_duration_since(Instant::now()))?;
        let length = decode_frame_length(&length_bytes)?;

        if length > max_length {
            return Err(format!("Incoming frame too large: {length} bytes").into());
        }

        self.receive_exact(length, deadline.saturating_duration_since(Instant::now()))
    }

    /// Receive and decode one length-prefixed MessagePack value.
    pub fn receive_msgpack_frame<T: DeserializeOwned>(&mut self, timeout: Duration) -> Result<T> {
        let frame = self.receive_frame(timeout, MAX_FRAME_LENGTH)?;
        Ok(rmp_serde::from_slice(&frame)?)
    }

    fn read_serial_chunk(&mut self) -> Result<Vec<u8>> {
        let mut buffer = [0_u8; READ_BUFFER_SIZE];

        match self.port.read(&mut buffer) {
            Ok(read) => Ok(buffer[..read].to_vec()),
            Err(error) if error.kind() == io::ErrorKind::TimedOut => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }
}

fn encode_frame(data: &[u8]) -> Result<Vec<u8>> {
    let length = u32::try_from(data.len())?;
    let mut frame = Vec::with_capacity(4 + data.len());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(data);
    Ok(frame)
}

fn decode_frame_length(length_bytes: &[u8]) -> Result<usize> {
    let length_bytes: [u8; 4] = length_bytes
        .try_into()
        .map_err(|_| "frame length must contain exactly four bytes")?;
    Ok(u32::from_be_bytes(length_bytes) as usize)
}

fn receive_timeout() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "USB serial receive timed out")
}

#[cfg(test)]
mod tests {
    use super::{decode_frame_length, encode_frame};

    #[test]
    fn frame_prefixes_payload_with_a_big_endian_length() {
        assert_eq!(
            encode_frame(b"abc").unwrap(),
            vec![0, 0, 0, 3, b'a', b'b', b'c']
        );
    }

    #[test]
    fn decodes_a_four_byte_frame_length() {
        assert_eq!(decode_frame_length(&[0, 0, 4, 0]).unwrap(), 1024);
    }

    #[test]
    fn rejects_an_incomplete_frame_length() {
        assert!(decode_frame_length(&[0, 0, 0]).is_err());
    }
}
