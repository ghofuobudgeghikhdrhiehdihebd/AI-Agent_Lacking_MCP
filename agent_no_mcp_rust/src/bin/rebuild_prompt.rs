fn main() {
    if let Err(error) = agent_no_mcp_rust::build_sys_prompt::build_sys_prompt() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
