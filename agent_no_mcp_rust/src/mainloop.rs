use anyhow::{Context, Result};
use serde_json::{Map, Value, json};
use std::env;

use crate::llm_prov::{GeminiChat, StreamTurn};
use crate::load_sys_prompt::get_sys_prompt;

/// Runs the agent using the first command-line argument as its user prompt.
pub fn main_from_env() -> Result<String> {
    let prompt = env::args()
        .nth(1)
        .context("usage: agent-no-mcp-rust <prompt>")?;
    run(&prompt)
}

/// Boundary for a synchronous MCP/UART executor.
///
/// Gemini streaming is stopped before this call is made. A real executor can
/// block until its UART/MCP response arrives, then return the JSON result.
pub trait ToolExecutor {
    fn call(&mut self, tool: &str, params: &Map<String, Value>) -> Value;
}

/// Temporary behavior while the in-progress MCP/UART receiver is not connected.
/// Returning a structured error avoids the busy wait in `MCPServer::call_tool`.
struct UnavailableToolExecutor;

impl ToolExecutor for UnavailableToolExecutor {
    fn call(&mut self, tool: &str, _params: &Map<String, Value>) -> Value {
        json!({
            "error": format!("Tool executor is not connected; {tool:?} was not executed"),
        })
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
                return Ok(response);
            }
            StreamTurn::ToolCall { text, call } => {
                response.push_str(&text);
                let tool_result = executor.call(&call.name, &call.args);
                turn = chat.continue_after_tool(&call, tool_result)?;
            }
        }
    }
}

/// Runs the agent with the current temporary MCP/UART boundary.
pub fn run(init_prompt: &str) -> Result<String> {
    run_with_executor(init_prompt, &mut UnavailableToolExecutor)
}

#[cfg(test)]
mod tests {
    use super::ToolExecutor;
    use serde_json::{Map, Value, json};

    struct RecordingExecutor {
        calls: Vec<(String, Map<String, Value>)>,
    }

    impl ToolExecutor for RecordingExecutor {
        fn call(&mut self, tool: &str, params: &Map<String, Value>) -> Value {
            self.calls.push((tool.to_owned(), params.clone()));
            json!({"result": "ok"})
        }
    }

    #[test]
    fn tool_executor_receives_native_function_arguments() {
        let mut executor = RecordingExecutor { calls: Vec::new() };
        let result = executor.call(
            "read",
            &Map::from_iter([(String::from("path"), json!("/tmp/a"))]),
        );
        assert_eq!(result, json!({"result": "ok"}));
        assert_eq!(executor.calls[0].0, "read");
        assert_eq!(executor.calls[0].1.get("path"), Some(&json!("/tmp/a")));
    }
}
