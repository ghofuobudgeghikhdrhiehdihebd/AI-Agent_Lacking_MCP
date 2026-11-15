use serde_json::{Map, Value};
use std::time::Duration;

use crate::serial_utils::ArduinoSerial;
use crate::tool_types::{ToolCall, ToolMessage, ToolResponse};

pub struct MCPServer {
    uart: ArduinoSerial,
    tool_timeout: Duration,
}

impl MCPServer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self::with_uart(ArduinoSerial::new()?))
    }

    pub fn with_uart(uart: ArduinoSerial) -> Self {
        Self {
            uart,
            tool_timeout: Duration::from_secs(30),
        }
    }

    /// Receives one unframed UTF-8 prompt from the USB serial peer.
    pub fn receive_prompt(
        &mut self,
        timeout: Duration,
        idle_timeout: Duration,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let bytes = self.uart.receive_until_idle(timeout, idle_timeout)?;
        Ok(String::from_utf8(bytes)?)
    }

    /// Transmits model text as unframed UTF-8 over USB serial.
    pub fn send_text(&mut self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.uart.send_text(text)
    }

    /// Sends a framed MessagePack tool call and waits for one framed MessagePack
    /// tool result returned by the board over USB serial.
    pub fn call_tool(
        &mut self,
        tool: &str,
        params: &Map<String, Value>,
    ) -> Result<ToolResponse, Box<dyn std::error::Error>> {
        self.uart
            .send_msgpack_frame(&ToolMessage::ToolCall(ToolCall {
                tool: tool.to_owned(),
                params: params.clone(),
            }))?;

        match self.uart.receive_msgpack_frame(self.tool_timeout)? {
            ToolMessage::ToolResult(response) => Ok(response),
            ToolMessage::ToolCall(_) => {
                Err("received a tool call while awaiting a tool result".into())
            }
        }
    }
}
