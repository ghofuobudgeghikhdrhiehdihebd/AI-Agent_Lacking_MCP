use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use std::time::Duration;
use serde_json::{Value, json};
use std::env;

const DEFAULT_MODEL: &str = "gemini-3.8-flash";
const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// A text response returned by the configured Gemini model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmResponse {
    pub text: Option<String>,
}

/// Stateful Gemini client matching the Python SDK chat session.
pub struct GeminiChat {
    client: Client,
    api_key: String,
    model: String,
    system_prompt: String,
    history: Vec<Value>,
}

impl GeminiChat {
    /// Creates a chat using `GEMINI_API_KEY` and optional `GEMINI_MODEL`.
    pub fn from_env(system_prompt: String) -> Result<Self> {
        dotenvy::dotenv().ok();
        let api_key = env::var("GEMINI_API_KEY").context("GEMINI_API_KEY is not set")?;
        let model = env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_owned());
        Ok(Self::new(api_key, model, system_prompt)?)
    }

    pub fn new(api_key: String, model: String, system_prompt: String) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .connect_timeout(Duration::from_secs(10))
                .build()
                .context("failed to build Gemini HTTP client")?,
            api_key,
            model,
            system_prompt,
            history: Vec::new(),
        })
    }

    /// Sends the prompt and accumulated tool-result context to Gemini.
    pub fn invoke(&mut self, prompt: &str, context: &str) -> Result<LlmResponse> {
        let message = format!(
            "\n        User prompt:\n\n        {prompt}\n\n        Relevant context (previous tool call results):\n\n        {context}\n        "
        );
        self.history.push(json!({
            "role": "user",
            "parts": [{"text": message}],
        }));

        let mut endpoint =
            reqwest::Url::parse(&format!("{GEMINI_API_BASE}/{}:generateContent", self.model))
                .context("failed to build Gemini API URL")?;
        endpoint.query_pairs_mut().append_pair("key", &self.api_key);
        let response = self
            .client
            .post(endpoint)
            .json(&json!({
                "systemInstruction": {"parts": [{"text": self.system_prompt}]},
                "contents": self.history,
            }))
            .send()
            .context("Gemini API request failed")?
            .error_for_status()
            .context("Gemini API returned an error response")?;

        let payload: Value = response
            .json()
            .context("failed to parse Gemini API response")?;
        let candidate = payload
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|candidates| candidates.first());
        let text = candidate
            .and_then(|value| value.pointer("/content/parts"))
            .and_then(Value::as_array)
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|part| part.get("text").and_then(Value::as_str))
                    .collect::<String>()
            })
            .filter(|text| !text.is_empty());

        if let Some(candidate) = candidate {
            self.history.push(
                candidate
                    .get("content")
                    .cloned()
                    .context("Gemini candidate did not contain content")?,
            );
        } else if payload.get("promptFeedback").is_none() {
            bail!("Gemini response did not contain a candidate or prompt feedback");
        }

        Ok(LlmResponse { text })
    }
}

/// Extracts valid JSON tool calls from fenced `json` markdown blocks.
pub fn split_tools_from_text(text: &str) -> (String, Vec<Value>) {
    const START: &str = "```json\n";
    const END: &str = "```";

    let mut remaining = text;
    let mut output = String::new();
    let mut tool_calls = Vec::new();

    while let Some(start) = remaining.find(START) {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start + START.len()..];
        let Some(end) = after_start.find(END) else {
            output.push_str(&remaining[start..]);
            return (output, tool_calls);
        };

        let json_block = &after_start[..end];
        if let Ok(tool_call) = serde_json::from_str(json_block) {
            tool_calls.push(tool_call);
        }
        remaining = &after_start[end + END.len()..];
    }

    output.push_str(remaining);
    (output, tool_calls)
}

#[cfg(test)]
mod tests {
    use super::split_tools_from_text;
    use serde_json::json;

    #[test]
    fn extracts_valid_fenced_tool_calls_and_removes_their_blocks() {
        let (text, calls) = split_tools_from_text(
            "Before\n```json\n{\"tool\": \"read\", \"params\": {}}\n```\nAfter",
        );
        assert_eq!(text, "Before\n\nAfter");
        assert_eq!(calls, vec![json!({"tool": "read", "params": {}})]);
    }

    #[test]
    fn ignores_invalid_json_but_still_removes_a_complete_fence() {
        let (text, calls) = split_tools_from_text("```json\nnot-json\n```visible");
        assert_eq!(text, "visible");
        assert!(calls.is_empty());
    }

    #[test]
    fn leaves_an_unclosed_fence_in_the_visible_text() {
        let input = "visible ```json\n{\"tool\": \"read\"}";
        assert_eq!(split_tools_from_text(input), (input.to_owned(), Vec::new()));
    }
}
