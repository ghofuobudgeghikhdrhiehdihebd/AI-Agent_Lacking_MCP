use serde_json::Value;
use std::collections::HashMap;

use crate::UART_serial::send_tool_call;
use crate::tool_types::ToolResponse;

pub struct MCPServer {
    awaiting_response: bool,
    tool_awaiting_response: Option<String>,
    params_awaiting_response: Option<HashMap<String, Value>>,
    response: Option<ToolResponse>,
}

impl MCPServer {
    pub fn new() -> Self {
        MCPServer {
            awaiting_response: false,
            tool_awaiting_response: None,
            params_awaiting_response: None,
            response: None,
        }
    }

    /// Sends a tool call to the MCP server via UART serial, sets state to awaiting
    fn send_tool_call(
        &mut self,
        tool: &str,
        params: &HashMap<String, Value>,
    ) -> Result<(), String> {
        if self.awaiting_response {
            return Err("Already awaiting a response from the MCP server".to_string());
        }
        self.awaiting_response = true;
        self.tool_awaiting_response = Some(tool.to_owned());
        self.params_awaiting_response = Some(params.to_owned());
        self.response = None;
        send_tool_call(
            &serde_json::to_string(&serde_json::json!({
                "type": "tool_call",
                "data": {
                    "tool": tool,
                    "params": params
                }
            }))
            .unwrap()
            .as_str(),
        )
        .map_err(|e| format!("Failed to send tool call: {}", e))?;
        // format: "{“type”: “tool_call”, “data”: {“tool”: (tool name), “params”: (tool params as JSON object)}}"
        // todo: send tool call to the actual MCP server via GPIO
        Ok(())
    }

    /// Receives a response from the MCP server and returns it to the agent.
    /// Note: This will consume the response object. That should be fine.
    pub fn receive_response(&mut self, response: ToolResponse) -> Result<(), String> {
        if !self.awaiting_response {
            return Err("Not awaiting a response from the MCP server".to_string());
        }
        self.awaiting_response = false;
        self.tool_awaiting_response = None;
        self.params_awaiting_response = None;
        self.response = Some(response);
        Ok(())
    }

    /// Calls a tool via the MCP server, awaits the response, and returns it
    pub fn call_tool(
        &mut self,
        tool: &str,
        params: &HashMap<String, Value>,
    ) -> Result<ToolResponse, String> {
        self.send_tool_call(tool, params)?;
        while self.awaiting_response {
            // Wait
            // Someone else will call receive_response()
        }
        let response = self.response.take().unwrap();
        return Ok(response);
    }
}
