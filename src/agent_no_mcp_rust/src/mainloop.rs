use crate::llm_prov::{GeminiChat, split_tools_from_text};
use crate::load_sys_prompt::get_sys_prompt;
use crate::mcp_serv::mcp_server;
use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::env;

/// Runs the agent using the first command-line argument as its user prompt.
pub fn main_from_env() -> Result<()> {
    let prompt = env::args()
        .nth(1)
        .context("usage: agent-no-mcp-rust <prompt>")?;
    run(&prompt)
}

/// Runs the tool-calling loop. MCP calls intentionally return failures until
/// an actual MCP server is connected.
pub fn run(init_prompt: &str) -> Result<()> {
    let mut chat = GeminiChat::from_env(get_sys_prompt()?)?;
    let mut context = Vec::new();

    let initial_context: String = context.concat();
    let initial_response = chat.invoke(init_prompt, &initial_context)?;
    let Some(initial_text) = initial_response.text else {
        return Ok(());
    };
    let (text, mut tools) = split_tools_from_text(&initial_text);
    println!("Response (text): {text}\nResponse (tools): {tools:?}\n");

    while !tools.is_empty() {
        let next_tool = tools.remove(0);
        let tool_name = next_tool
            .get("tool")
            .and_then(Value::as_str)
            .unwrap_or("INVALID_TOOL");
        let tool_params = next_tool
            .get("params")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        let mcp_result = mcp_server(tool_name, &tool_params);

        if mcp_result.status == "success" {
            context.push(format!(
                "Tool {tool_name} executed successfully with result: {}",
                mcp_result.result.unwrap_or_default()
            ));
        } else {
            context.push(format!(
                "Tool {tool_name} failed with error: {}",
                mcp_result
                    .error
                    .unwrap_or_else(|| "Unknown error".to_owned())
            ));
        }

        let current_context: String = context.concat();
        let response = chat.invoke(init_prompt, &current_context)?;
        let Some(response_text) = response.text else {
            continue;
        };
        let (text, additional_tools) = split_tools_from_text(&response_text);
        println!("Response (text): {text}\nResponse (tools): {additional_tools:?}\n");
        tools.extend(additional_tools);
        println!("Current context: {context:?}\n");
    }

    Ok(())
}
