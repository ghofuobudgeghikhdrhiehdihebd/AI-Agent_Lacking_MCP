use std::time::Duration;

use agent_no_mcp_rust::mainloop::run_with_executor;
use agent_no_mcp_rust::mcp_serv::MCPServer;

const PROMPT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const PROMPT_IDLE_TIMEOUT: Duration = Duration::from_millis(100);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut serial_server = MCPServer::new()?;

    loop {
        let prompt = match serial_server.receive_prompt(PROMPT_TIMEOUT, PROMPT_IDLE_TIMEOUT) {
            Ok(prompt) if prompt.trim().is_empty() => continue,
            Ok(prompt) => prompt,
            Err(error) => {
                eprintln!("failed to receive a UTF-8 prompt over USB serial: {error}");
                continue;
            }
        };

        if let Err(error) = run_with_executor(&prompt, &mut serial_server) {
            eprintln!("agent request failed: {error}");
            serial_server.send_text(&format!("Error: {error}\n"))?;
        }
    }
}
