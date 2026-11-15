use std::sync::{LazyLock, Mutex};
use serde_json::Value;
use crate::serial_utils;

static AIRLOCK: LazyLock<Mutex<Box<dyn serialport::SerialPort>>> = LazyLock::new(|| {
    Mutex::new(
        serial_utils::init_airlock()
            .expect("Failed to initialize airlock"),
    )
});

/// The result returned to the agent after attempting an MCP tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpResponse {
    pub status: String,
    pub result: Option<String>,
    pub error: Option<String>
}

impl McpResponse {
    fn success(result: impl Into<String>) -> Self {
        Self {
            status: "success".to_owned(),
            result: Some(result.into()),
            error: None
        }
    }

    fn failed(error: impl Into<String>) -> Self {
        Self {
            status: "failed".to_owned(),
            result: None,
            error: Some(error.into())
        }
    }
}

/// Validates the tool-call envelope and reports that no MCP server exists yet.
///
/// No tool is executed locally. A future implementation can replace this
/// transport boundary with an MCP client without changing the agent loop.
pub fn mcp_server(tool: &str, params: &Value) -> McpResponse {
    if tool.trim().is_empty() || !params.is_object() {
        return McpResponse::failed("Invalid input types");
    }
    let mut airlock = AIRLOCK.lock().expect("Airlock mutex is poisoned");
    // Send a serial request
    serial_utils::send_str(&mut **airlock, &format!("Tool: {}\nParams: {}\n", tool, params))
        .expect("Failed to send request to airlock");
    // Get a serial response
    loop {
        match serial_utils::read_line(&mut **airlock) {
            Ok(line) => {
                if !line.trim().is_empty() {
                    return McpResponse::success(line);
                }
            }
            Err(e) => {
                eprintln!("Error reading from airlock: {}", e);
                return McpResponse::failed("Error reading from airlock");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mcp_server;
    use serde_json::json;

    #[test]
    fn does_not_execute_tools_before_mcp_is_implemented() {
        let response = mcp_server("read", &json!({"path": "/tmp/example"}));
        assert_eq!(response.status, "failed");
        assert_eq!(
            response.error.as_deref(),
            Some("MCP server is not implemented")
        );
    }

    #[test]
    fn rejects_non_object_parameters() {
        let response = mcp_server("read", &json!("not an object"));
        assert_eq!(response.error.as_deref(), Some("Invalid input types"));
    }
}
