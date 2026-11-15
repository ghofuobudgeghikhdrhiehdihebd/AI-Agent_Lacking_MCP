use serde_json::{Map, Value};
use std::time::Duration;

use crate::tool_types::{ToolCall, ToolMessage, ToolResponse};
use crate::uart_utils::ArduinoUart;

pub struct MCPServer {
    uart: ArduinoUart,
    tool_timeout: Duration,
}

impl MCPServer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self::with_uart(ArduinoUart::new()?))
    }

    pub fn with_uart(uart: ArduinoUart) -> Self {
        Self {
            uart,
            tool_timeout: Duration::from_secs(30),
        }
    }

    /// Transmits UTF-8 prompt/model text without MessagePack serialization.
    pub fn send_text(&mut self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.uart.send_frame(text.as_bytes())
    }

    /// Sends a framed MessagePack tool call and waits for one framed MessagePack
    /// tool result returned by RouterBridge `uart_read`.
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
