//! MessagePack framing over the Arduino UNO Q hardware UART.
//!
//! Wire a USB-to-TTL adapter to the UNO Q's `D1 / USART1_TX` and
//! `D0 / USART1_RX` pins with crossed data lines and a shared ground. The UNO
//! Q MCU UART uses 3.3 V logic; do not connect a 5 V TTL adapter directly.
//!
//! Each wire message is one frame: a four-byte big-endian payload length,
//! followed by exactly that many MessagePack bytes. The length prefix lets the
//! receiver handle partial serial reads and distinguish consecutive values.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::env;
use std::io::{self, Read, Write};
use std::time::Duration;

/// Environment variable containing the USB-to-TTL device path, for example
/// `/dev/cu.usbserial-...` on macOS or `/dev/ttyUSB0` on Linux.
pub const UNO_Q_PORT_ENV: &str = "ARDUINO_UNO_Q_PORT";
/// Optional UART baud-rate environment variable. Defaults to 115200.
pub const UNO_Q_BAUD_ENV: &str = "ARDUINO_UNO_Q_BAUD";
pub const DEFAULT_BAUD_RATE: u32 = 115_200;
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_MESSAGEPACK_FRAME_BYTES: usize = 1024 * 1024;

/// Opened serial transport connected to the UNO Q `Serial1` hardware UART.
pub struct UnoQUart {
    port: Box<dyn serialport::SerialPort>,
}

impl UnoQUart {
    /// Connects using `ARDUINO_UNO_Q_PORT` and an optional baud-rate override.
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let device = env::var(UNO_Q_PORT_ENV).map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{UNO_Q_PORT_ENV} is not set"),
            )
        })?;
        let baud_rate = env::var(UNO_Q_BAUD_ENV)
            .ok()
            .map(|value| value.parse::<u32>())
            .transpose()
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{UNO_Q_BAUD_ENV} must be an unsigned integer: {error}"),
                )
            })?
            .unwrap_or(DEFAULT_BAUD_RATE);
        Self::open(&device, baud_rate, DEFAULT_READ_TIMEOUT)
    }

    /// Opens the OS serial device attached to the UNO Q UART pins.
    pub fn open(
        device: &str,
        baud_rate: u32,
        read_timeout: Duration,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let port = serialport::new(device, baud_rate)
            .timeout(read_timeout)
            .open()?;
        Ok(Self { port })
    }

    /// Sends a length-prefixed MessagePack value and flushes the UART.
    pub fn send<T: Serialize>(&mut self, value: &T) -> Result<(), Box<dyn std::error::Error>> {
        write_messagepack_frame(&mut *self.port, value)
    }

    /// Waits for and decodes one length-prefixed MessagePack value.
    pub fn receive<T: DeserializeOwned>(&mut self) -> Result<T, Box<dyn std::error::Error>> {
        read_messagepack_frame(&mut *self.port)
    }
}

/// Writes one framed MessagePack value to any byte stream.
pub fn write_messagepack_frame<W: Write + ?Sized, T: Serialize>(
    writer: &mut W,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let payload = rmp_serde::to_vec_named(value)?;
    if payload.len() > MAX_MESSAGEPACK_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "MessagePack payload is {} bytes; maximum is {MAX_MESSAGEPACK_FRAME_BYTES}",
                payload.len()
            ),
        )
        .into());
    }

    let length = u32::try_from(payload.len())?;
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Reads and decodes one framed MessagePack value from any byte stream.
pub fn read_messagepack_frame<R: Read + ?Sized, T: DeserializeOwned>(
    reader: &mut R,
) -> Result<T, Box<dyn std::error::Error>> {
    let mut length = [0_u8; 4];
    reader.read_exact(&mut length)?;
    let payload_length = u32::from_be_bytes(length) as usize;
    if payload_length > MAX_MESSAGEPACK_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "MessagePack frame is {payload_length} bytes; maximum is {MAX_MESSAGEPACK_FRAME_BYTES}"
            ),
        )
        .into());
    }

    let mut payload = vec![0_u8; payload_length];
    reader.read_exact(&mut payload)?;
    Ok(rmp_serde::from_slice(&payload)?)
}

/// Compatibility helper used by the MCP layer to transmit its JSON envelope.
/// The string itself is MessagePack encoded, not written as UTF-8 line text.
pub fn send_tool_call(data: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut uart = UnoQUart::from_env()?;
    uart.send(&data)
}

/// Opens the configured UNO Q UART. Prefer `UnoQUart::from_env` for new code.
pub fn init_airlock() -> Result<Box<dyn serialport::SerialPort>, Box<dyn std::error::Error>> {
    let device = env::var(UNO_Q_PORT_ENV).map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{UNO_Q_PORT_ENV} is not set"),
        )
    })?;
    let baud_rate = env::var(UNO_Q_BAUD_ENV)
        .ok()
        .map(|value| value.parse::<u32>())
        .transpose()?
        .unwrap_or(DEFAULT_BAUD_RATE);
    Ok(serialport::new(device, baud_rate)
        .timeout(DEFAULT_READ_TIMEOUT)
        .open()?)
}

/// Compatibility helper that writes one framed MessagePack string.
pub fn send_str(
    airlock: &mut dyn serialport::SerialPort,
    data: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    write_messagepack_frame(airlock, &data)
}

/// Compatibility helper that reads one framed MessagePack string.
pub fn read_line(
    airlock: &mut dyn serialport::SerialPort,
) -> Result<String, Box<dyn std::error::Error>> {
    read_messagepack_frame(airlock)
}

#[cfg(test)]
mod tests {
    use super::{MAX_MESSAGEPACK_FRAME_BYTES, read_messagepack_frame, write_messagepack_frame};
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct ToolEnvelope {
        kind: String,
        sequence: u64,
    }

    #[test]
    fn round_trips_multiple_messagepack_frames() {
        let first = ToolEnvelope {
            kind: "tool_call".to_owned(),
            sequence: 1,
        };
        let second = ToolEnvelope {
            kind: "tool_response".to_owned(),
            sequence: 2,
        };
        let mut bytes = Vec::new();
        write_messagepack_frame(&mut bytes, &first).unwrap();
        write_messagepack_frame(&mut bytes, &second).unwrap();

        let mut reader = Cursor::new(bytes);
        assert_eq!(
            read_messagepack_frame::<_, ToolEnvelope>(&mut reader).unwrap(),
            first
        );
        assert_eq!(
            read_messagepack_frame::<_, ToolEnvelope>(&mut reader).unwrap(),
            second
        );
    }

    #[test]
    fn rejects_oversized_frame_before_allocating() {
        let mut reader = Cursor::new(((MAX_MESSAGEPACK_FRAME_BYTES as u32) + 1).to_be_bytes());
        let error = read_messagepack_frame::<_, String>(&mut reader).unwrap_err();
        assert!(error.to_string().contains("maximum"));
    }
}
