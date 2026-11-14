fn main() {
    if let Err(error) = agent_no_mcp_rust::mainloop::main_from_env() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
