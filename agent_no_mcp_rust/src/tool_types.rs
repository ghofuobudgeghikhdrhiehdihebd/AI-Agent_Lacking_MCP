use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// A native Gemini tool call transported to the MCU as MessagePack.
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct ToolCall {
    pub tool: String,
    pub params: Map<String, Value>,
}

/// A tool result transported from the MCU as MessagePack.
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ToolResponse {
    Success { result: Value },
    Failure { error: String },
}

impl ToolResponse {
    /// Converts the MCU response to the object Gemini receives in
    /// `functionResponse.response`.
    pub fn into_gemini_result(self) -> Value {
        match self {
            Self::Success { result } => json!({"result": result}),
            Self::Failure { error } => json!({"error": error}),
        }
    }
}

/// MessagePack envelope used on the tool UART stream.
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ToolMessage {
    ToolCall(ToolCall),
    ToolResult(ToolResponse),
}

#[cfg(test)]
mod tests {
    use super::{ToolCall, ToolMessage, ToolResponse};
    use serde_json::{Map, json};

    #[test]
    fn tool_messages_round_trip_as_messagepack() {
        let message = ToolMessage::ToolCall(ToolCall {
            tool: "read".to_owned(),
            params: Map::from_iter([(String::from("path"), json!("/tmp/example"))]),
        });
        let encoded = rmp_serde::to_vec_named(&message).unwrap();
        assert_eq!(
            rmp_serde::from_slice::<ToolMessage>(&encoded).unwrap(),
            message
        );

        let result = ToolMessage::ToolResult(ToolResponse::Success {
            result: json!("contents"),
        });
        let encoded = rmp_serde::to_vec_named(&result).unwrap();
        assert_eq!(
            rmp_serde::from_slice::<ToolMessage>(&encoded).unwrap(),
            result
        );
    }
}
