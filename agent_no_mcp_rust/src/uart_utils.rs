use std::error::Error;
use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::{Duration, Instant};

use rmpv::Value;
use rmpv::decode::read_value;
use rmpv::encode::write_value;
use serde::{Serialize, de::DeserializeOwned};

pub const SOCKET_PATH: &str = "/var/run/arduino-router.sock";
const CHUNK_SIZE: usize = 64;
pub const MAX_FRAME_LENGTH: usize = 1024 * 1024;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub struct ArduinoUart {
    stream: UnixStream,
    next_id: u32,
    pending_rx: Vec<u8>,
}

impl ArduinoUart {
    /// Connect to the Arduino Router on Linux.
    pub fn new() -> Result<Self> {
        let stream = UnixStream::connect(SOCKET_PATH)?;

        stream.set_read_timeout(Some(Duration::from_secs(5)))?;

        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        Ok(Self {
            stream,
            next_id: 1,
            pending_rx: Vec::new(),
        })
    }

    /// Invoke an STM32 function using MessagePack-RPC.
    fn call(&mut self, method: &str, args: Vec<Value>) -> Result<Value> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        // MessagePack-RPC:
        // [0, message_id, method, arguments]
        let request = Value::Array(vec![
            Value::from(0),
            Value::from(id),
            Value::from(method),
            Value::Array(args),
        ]);

        // Serialize into a complete request buffer.
        let mut encoded = Vec::new();
        write_value(&mut encoded, &request)?;

        // Write the entire request to Arduino Router.
        self.stream.write_all(&encoded)?;

        // Read one complete MessagePack response.
        let response = read_value(&mut self.stream)?;

        let fields = response.as_array().ok_or("Invalid RPC response")?;

        if fields.len() != 4
            || fields[0].as_u64() != Some(1)
            || fields[1].as_u64() != Some(id as u64)
        {
            return Err("Unexpected RPC response".into());
        }

        if !fields[2].is_nil() {
            return Err(format!("STM32 RPC error: {:?}", fields[2]).into());
        }

        Ok(fields[3].clone())
    }

    /// Send arbitrary bytes through RouterBridge `uart_write`.
    ///
    /// Large buffers are split into smaller RPC calls.
    pub fn send(&mut self, data: &[u8]) -> Result<()> {
        for chunk in data.chunks(CHUNK_SIZE) {
            let mut offset = 0;

            while offset < chunk.len() {
                let remaining = &chunk[offset..];

                let result = self.call("uart_write", vec![Value::Binary(remaining.to_vec())])?;

                // RouterBridge exposes:
                // int uart_write(std::vector<uint8_t> data)
                // so every successful RPC result must be a byte count.
                let written = accepted_write_length(&result, remaining.len())?;

                offset += written;
            }
        }

        Ok(())
    }

    /// Read any currently available UART bytes.
    ///
    /// Returns an empty Vec if no bytes are available.
    /// Also returns any bytes previously buffered by
    /// receive_exact().
    pub fn receive(&mut self) -> Result<Vec<u8>> {
        let mut data = std::mem::take(&mut self.pending_rx);

        data.extend(self.read_uart_chunk()?);

        Ok(data)
    }

    /// Read one chunk directly from the STM32 UART.
    fn read_uart_chunk(&mut self) -> Result<Vec<u8>> {
        let result = self.call("uart_read", vec![])?;

        match result {
            Value::Binary(bytes) => Ok(bytes),
            _ => Err("Expected binary data from UART".into()),
        }
    }

    /// Wait until exactly `count` bytes have arrived.
    ///
    /// Returns a timeout error if insufficient data
    /// arrives before `timeout`.
    pub fn receive_exact(&mut self, count: usize, timeout: Duration) -> Result<Vec<u8>> {
        let start = Instant::now();

        while self.pending_rx.len() < count {
            if start.elapsed() >= timeout {
                return Err(
                    io::Error::new(io::ErrorKind::TimedOut, "UART receive timed out").into(),
                );
            }

            let chunk = self.read_uart_chunk()?;

            if chunk.is_empty() {
                thread::sleep(Duration::from_millis(5));
            } else {
                self.pending_rx.extend(chunk);
            }
        }

        // Keep any bytes beyond the requested count
        // for the next receive operation.
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

    /// Serialize a value as MessagePack and transmit it in one framed UART
    /// message. Use this for tool-call and tool-result envelopes.
    pub fn send_msgpack_frame<T: Serialize>(&mut self, value: &T) -> Result<()> {
        let encoded = rmp_serde::to_vec_named(value)?;
        self.send_frame(&encoded)
    }

    /// Send a length-prefixed frame.
    ///
    /// Format: [4-byte length (big endian)][payload]
    pub fn send_frame(&mut self, data: &[u8]) -> Result<()> {
        let length = u32::try_from(data.len())?;

        let mut frame = Vec::with_capacity(4 + data.len());

        frame.extend_from_slice(&length.to_be_bytes());
        frame.extend_from_slice(data);

        self.send(&frame)
    }

    /// Receive one length-prefixed frame.
    pub fn receive_frame(&mut self, timeout: Duration, max_length: usize) -> Result<Vec<u8>> {
        let deadline = Instant::now() + timeout;

        let length_bytes =
            self.receive_exact(4, deadline.saturating_duration_since(Instant::now()))?;

        let length = u32::from_be_bytes(length_bytes.try_into().unwrap_or([0; 4])) as usize;

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
}

fn accepted_write_length(result: &Value, requested: usize) -> Result<usize> {
    let written = usize::try_from(result.as_i64().ok_or("uart_write must return an integer")?)
        .map_err(|_| "uart_write must return a non-negative integer")?;

    if written == 0 || written > requested {
        return Err(format!("Invalid UART write count: {written}").into());
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::accepted_write_length;
    use rmpv::Value;

    #[test]
    fn rejects_non_integer_router_bridge_uart_write_result() {
        assert!(accepted_write_length(&Value::Nil, 8).is_err());
        assert!(accepted_write_length(&Value::from("8"), 8).is_err());
    }

    #[test]
    fn accepts_valid_router_bridge_write_count() {
        assert_eq!(accepted_write_length(&Value::from(3), 8).unwrap(), 3);
    }

    #[test]
    fn rejects_invalid_router_bridge_write_count() {
        assert!(accepted_write_length(&Value::from(-1), 8).is_err());
        assert!(accepted_write_length(&Value::from(9), 8).is_err());
    }
}
