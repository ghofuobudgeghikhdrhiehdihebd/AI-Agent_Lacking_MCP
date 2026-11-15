use serde_json::Value;
use std::collections::HashMap;

pub enum ToolStatus {
    Failure,
    Success,
}

pub struct ToolCall {
    pub tool: String,
    pub params: HashMap<String, Value>,
}

pub struct Success {
    pub status: ToolStatus,
    pub result: String,
}

pub struct Failure {
    pub status: ToolStatus,
    pub error: String,
}

pub enum ToolResponse {
    Success(Success),
    Failure(Failure),
}
