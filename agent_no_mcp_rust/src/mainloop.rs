use anyhow::{Result, anyhow};
use serde_json::{Map, Value};

use crate::llm_prov::{GeminiChat, StreamTurn};
use crate::load_sys_prompt::get_sys_prompt;
use crate::mcp_serv::MCPServer;

/// Boundary for a synchronous MCP/UART executor.
///
/// Gemini streaming is stopped before this call is made. A real executor can
/// block until its UART/MCP response arrives, then return the JSON result.
pub trait ToolExecutor {
    /// Sends assistant text as unframed UTF-8 over USB serial.
    fn send_text(&mut self, text: &str) -> Result<()>;
    /// Sends a MessagePack tool call and returns its MessagePack-decoded result.
    fn call(&mut self, tool: &str, params: &Map<String, Value>) -> Result<Value>;
}

impl ToolExecutor for MCPServer {
    fn send_text(&mut self, text: &str) -> Result<()> {
        self.send_text(&format!("{text}\n"))
            .map_err(|error| anyhow!("failed to send UTF-8 text over USB serial: {error}"))
    }

    fn call(&mut self, tool: &str, params: &Map<String, Value>) -> Result<Value> {
        self.call_tool(tool, params)
            .map_err(|error| anyhow!("USB serial tool transport failed: {error}"))
            .map(|response| response.into_gemini_result())
    }
}

/// Runs the streaming model/tool state machine using the supplied executor.
pub fn run_with_executor(init_prompt: &str, executor: &mut dyn ToolExecutor) -> Result<String> {
    let mut chat = GeminiChat::from_env(get_sys_prompt()?)?;
    let mut response = String::new();
    let mut turn = chat.start_turn(init_prompt)?;

    loop {
        match turn {
            StreamTurn::Complete { text } => {
                response.push_str(&text);
                send_text_over_uart(executor, &text)?;
                return Ok(response);
            }
            StreamTurn::ToolCall { text, call } => {
                response.push_str(&text);
                send_text_over_uart(executor, &text)?;
                let tool_result = executor.call(&call.name, &call.args)?;
                turn = chat.continue_after_tool(&call, tool_result)?;
            }
        }
    }
}

fn send_text_over_uart(executor: &mut dyn ToolExecutor, text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    println!("Sending text over USB serial: {text}");
    executor.send_text(text)
}

/// Runs the agent through the Arduino USB serial transport.
pub fn run(init_prompt: &str) -> Result<String> {
    let mut mcp_server = MCPServer::new().map_err(|error| {
        anyhow!("failed to open Arduino USB serial device /dev/ttyGS0: {error}")
    })?;
    run_with_executor(init_prompt, &mut mcp_server)
}

#[cfg(test)]
mod tests {
    use super::ToolExecutor;
    use anyhow::Result;
    use serde_json::{Map, Value, json};

    struct RecordingExecutor {
        calls: Vec<(String, Map<String, Value>)>,
    }

    impl ToolExecutor for RecordingExecutor {
        fn send_text(&mut self, _text: &str) -> Result<()> {
            Ok(())
        }

        fn call(&mut self, tool: &str, params: &Map<String, Value>) -> Result<Value> {
            self.calls.push((tool.to_owned(), params.clone()));
            Ok(json!({"result": "ok"}))
        }
    }

    #[test]
    fn tool_executor_receives_native_function_arguments() {
        let mut executor = RecordingExecutor { calls: Vec::new() };
        let result = executor.call(
            "read",
            &Map::from_iter([(String::from("path"), json!("/tmp/a"))]),
        );
        assert_eq!(result.unwrap(), json!({"result": "ok"}));
        assert_eq!(executor.calls[0].0, "read");
        assert_eq!(executor.calls[0].1.get("path"), Some(&json!("/tmp/a")));
    }
}
