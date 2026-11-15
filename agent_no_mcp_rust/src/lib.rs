//! A Rust translation of the `agent_no_mcp` Python package.
//!
//! This crate intentionally does not execute tools. The MCP-facing module is
//! a boundary stub until a real MCP transport and server are implemented.

pub mod build_sys_prompt;
pub mod llm_prov;
pub mod load_sys_prompt;
pub mod mainloop;
pub mod mcp_serv;
pub mod serial_utils;
pub mod tool_types;
