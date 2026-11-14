use serde_json::Value;

/// The result returned to the agent after attempting an MCP tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpResponse {
    pub status: String,
    pub result: Option<String>,
    pub error: Option<String>,
}

impl McpResponse {
    fn failed(error: impl Into<String>) -> Self {
        Self {
            status: "failed".to_owned(),
            result: None,
            error: Some(error.into()),
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

    McpResponse::failed("MCP server is not implemented")
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
