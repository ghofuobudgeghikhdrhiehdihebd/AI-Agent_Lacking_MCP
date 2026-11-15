use anyhow::{Context, Result, bail};
use reqwest::blocking::{Client, Response};
use serde_json::{Map, Value, json};
use std::env;
use std::io::{BufRead, BufReader};
use std::time::Duration;

const DEFAULT_MODEL: &str = "gemini-3.8-flash";
const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const DEFAULT_TOOLS_JSON: &str = include_str!("../assets/tools.json");

/// A structured function call emitted by Gemini.
#[derive(Debug, Clone, PartialEq)]
pub struct GeminiToolCall {
    pub name: String,
    pub args: Map<String, Value>,
}

/// The result of consuming one Gemini streaming turn.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamTurn {
    Complete { text: String },
    ToolCall { text: String, call: GeminiToolCall },
}

/// Stateful Gemini chat client with native function-calling support.
pub struct GeminiChat {
    client: Client,
    api_key: String,
    model: String,
    system_prompt: String,
    tools: Value,
    history: Vec<Value>,
}

impl GeminiChat {
    pub fn from_env(system_prompt: String) -> Result<Self> {
        dotenvy::dotenv().ok();
        let api_key = env::var("GEMINI_API_KEY").context("GEMINI_API_KEY is not set")?;
        let model = env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_owned());
        Self::new(api_key, model, system_prompt)
    }

    pub fn new(api_key: String, model: String, system_prompt: String) -> Result<Self> {
        let definitions: Value = serde_json::from_str(DEFAULT_TOOLS_JSON)
            .context("failed to parse bundled tools.json")?;
        Self::new_with_tools(api_key, model, system_prompt, definitions)
    }

    fn new_with_tools(
        api_key: String,
        model: String,
        system_prompt: String,
        definitions: Value,
    ) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .connect_timeout(Duration::from_secs(10))
                .build()
                .context("failed to build Gemini HTTP client")?,
            api_key,
            model,
            system_prompt,
            tools: gemini_tools_from_definitions(&definitions)?,
            history: Vec::new(),
        })
    }

    /// Adds the initial user message and consumes its streamed model turn.
    pub fn start_turn(&mut self, prompt: &str) -> Result<StreamTurn> {
        self.history
            .push(json!({"role": "user", "parts": [{"text": prompt}]}));
        self.stream_turn()
    }

    /// Submits the result of the previous native function call in a new request.
    pub fn continue_after_tool(
        &mut self,
        call: &GeminiToolCall,
        result: Value,
    ) -> Result<StreamTurn> {
        self.history.push(function_response_content(call, result));
        self.stream_turn()
    }

    fn stream_turn(&mut self) -> Result<StreamTurn> {
        let response = self.send_stream_request()?;
        let (turn, content) = consume_sse_response(response)?;
        if let Some(content) = content {
            self.history.push(content);
        }
        Ok(turn)
    }

    fn send_stream_request(&self) -> Result<Response> {
        let mut endpoint = reqwest::Url::parse(&format!(
            "{GEMINI_API_BASE}/{}:streamGenerateContent",
            self.model
        ))
        .context("failed to build Gemini streaming API URL")?;
        endpoint
            .query_pairs_mut()
            .append_pair("alt", "sse")
            .append_pair("key", &self.api_key);

        self.client
            .post(endpoint)
            .json(&json!({
                "systemInstruction": {"parts": [{"text": self.system_prompt}]},
                "contents": self.history,
                "tools": self.tools,
            }))
            .send()
            .context("Gemini streaming API request failed")?
            .error_for_status()
            .context("Gemini streaming API returned an error response")
    }
}

/// Reads Gemini SSE events until completion or the first native function call.
/// Returning on a call drops the HTTP response; continuation always uses a new
/// `streamGenerateContent` request with a `functionResponse` history turn.
fn consume_sse_response(response: Response) -> Result<(StreamTurn, Option<Value>)> {
    consume_sse_reader(BufReader::new(response))
}

fn consume_sse_reader<R: BufRead>(mut reader: R) -> Result<(StreamTurn, Option<Value>)> {
    let mut event_data = String::new();
    let mut text = String::new();
    let mut model_parts = Vec::new();
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .context("failed to read Gemini SSE stream")?;
        if bytes == 0 {
            if !event_data.is_empty()
                && let Some(call) = consume_sse_event(&event_data, &mut text, &mut model_parts)?
            {
                return Ok((
                    StreamTurn::ToolCall { text, call },
                    model_content(model_parts),
                ));
            }
            return Ok((StreamTurn::Complete { text }, model_content(model_parts)));
        }

        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if event_data.is_empty() {
                continue;
            }
            if let Some(call) = consume_sse_event(&event_data, &mut text, &mut model_parts)? {
                return Ok((
                    StreamTurn::ToolCall { text, call },
                    model_content(model_parts),
                ));
            }
            event_data.clear();
        } else if let Some(data) = line.strip_prefix("data:") {
            if !event_data.is_empty() {
                event_data.push('\n');
            }
            event_data.push_str(data.trim_start());
        }
    }
}

fn consume_sse_event(
    data: &str,
    text: &mut String,
    model_parts: &mut Vec<Value>,
) -> Result<Option<GeminiToolCall>> {
    if data == "[DONE]" {
        return Ok(None);
    }
    let payload: Value = serde_json::from_str(data).context("failed to parse Gemini SSE event")?;
    let Some(candidate) = payload
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
    else {
        if payload.get("promptFeedback").is_none() {
            bail!("Gemini stream event did not contain a candidate or prompt feedback");
        }
        return Ok(None);
    };
    let Some(parts) = candidate
        .pointer("/content/parts")
        .and_then(Value::as_array)
    else {
        return Ok(None);
    };

    for part in parts {
        if let Some(chunk) = part.get("text").and_then(Value::as_str) {
            text.push_str(chunk);
            model_parts.push(json!({"text": chunk}));
        } else if let Some(function_call) = part.get("functionCall") {
            let call = parse_function_call(function_call)?;
            model_parts.push(json!({"functionCall": function_call}));
            return Ok(Some(call));
        } else {
            model_parts.push(part.clone());
        }
    }
    Ok(None)
}

fn parse_function_call(value: &Value) -> Result<GeminiToolCall> {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .context("Gemini function call did not contain a name")?
        .to_owned();
    let args = value
        .get("args")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()))
        .as_object()
        .cloned()
        .context("Gemini function call arguments were not an object")?;
    Ok(GeminiToolCall { name, args })
}

fn model_content(parts: Vec<Value>) -> Option<Value> {
    (!parts.is_empty()).then(|| json!({"role": "model", "parts": parts}))
}

fn function_response_content(call: &GeminiToolCall, result: Value) -> Value {
    json!({
        "role": "user",
        "parts": [{"functionResponse": {"name": call.name, "response": result}}],
    })
}

/// Translates this project's `tools.json` format into Gemini declarations.
fn gemini_tools_from_definitions(definitions: &Value) -> Result<Value> {
    let definitions = definitions
        .as_object()
        .context("tools.json must be a JSON object keyed by tool name")?;
    let mut declarations = Vec::with_capacity(definitions.len());
    for (name, definition) in definitions {
        let description = definition
            .get("description")
            .and_then(Value::as_str)
            .with_context(|| format!("tool {name:?} is missing a string description"))?;
        let parameters = definition
            .get("params")
            .and_then(Value::as_object)
            .with_context(|| format!("tool {name:?} is missing an object params field"))?;
        let mut properties = Map::new();
        let mut required = Vec::new();
        for (parameter_name, parameter) in parameters {
            let description = parameter
                .get("description")
                .and_then(Value::as_str)
                .with_context(|| {
                    format!(
                        "parameter {parameter_name:?} for tool {name:?} is missing a description"
                    )
                })?;
            properties.insert(
                parameter_name.clone(),
                json!({"type": "STRING", "description": description}),
            );
            if parameter
                .get("mandatory")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                required.push(Value::String(parameter_name.clone()));
            }
        }
        declarations.push(json!({
            "name": name,
            "description": description,
            "parameters": {"type": "OBJECT", "properties": properties, "required": required},
        }));
    }
    Ok(json!([{"functionDeclarations": declarations}]))
}

#[cfg(test)]
mod tests {
    use super::{
        GeminiToolCall, StreamTurn, consume_sse_event, consume_sse_reader,
        function_response_content, gemini_tools_from_definitions, model_content,
    };
    use serde_json::{Map, Value, json};
    use std::io::Cursor;

    #[test]
    fn converts_project_tools_to_gemini_function_declarations() {
        let tools = gemini_tools_from_definitions(&json!({
            "read": {"description": "Reads a file.", "params": {
                "path": {"mandatory": true, "description": "Path."},
                "max_length": {"mandatory": false, "description": "Limit."}
            }}
        }))
        .unwrap();
        assert_eq!(
            tools.pointer("/0/functionDeclarations/0/name"),
            Some(&json!("read"))
        );
        assert_eq!(
            tools.pointer("/0/functionDeclarations/0/parameters/required"),
            Some(&json!(["path"]))
        );
    }

    #[test]
    fn collects_text_and_stops_at_a_native_function_call() {
        let mut text = String::new();
        let mut parts = Vec::new();
        let call = consume_sse_event(
            r#"{"candidates":[{"content":{"parts":[{"text":"Checking "},{"functionCall":{"name":"read","args":{"path":"/tmp/a"}}},{"text":"ignored"}]}}]}"#,
            &mut text,
            &mut parts,
        )
        .unwrap();
        assert_eq!(text, "Checking ");
        assert_eq!(
            call,
            Some(GeminiToolCall {
                name: "read".to_owned(),
                args: Map::from_iter([(String::from("path"), json!("/tmp/a"))]),
            })
        );
        assert_eq!(
            model_content(parts),
            Some(json!({"role":"model","parts":[
                {"text":"Checking "},
                {"functionCall":{"name":"read","args":{"path":"/tmp/a"}}}
            ]}))
        );
    }

    #[test]
    fn sse_reader_drops_the_stream_after_the_first_function_call() {
        let stream = concat!(
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Before \"}]}}]}\n\n",
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"read\",\"args\":{\"path\":\"/tmp/a\"}}}]}}]}\n\n",
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"must not be read\"}]}}]}\n\n"
        );
        let (turn, content) = consume_sse_reader(Cursor::new(stream)).unwrap();

        assert_eq!(
            turn,
            StreamTurn::ToolCall {
                text: "Before ".to_owned(),
                call: GeminiToolCall {
                    name: "read".to_owned(),
                    args: Map::from_iter([(String::from("path"), json!("/tmp/a"))]),
                },
            }
        );
        assert_eq!(
            content,
            Some(json!({"role":"model","parts":[
                {"text":"Before "},
                {"functionCall":{"name":"read","args":{"path":"/tmp/a"}}}
            ]}))
        );
    }

    #[test]
    fn builds_a_structured_function_response_history_turn() {
        let call = GeminiToolCall {
            name: "read".to_owned(),
            args: Map::new(),
        };
        assert_eq!(
            function_response_content(&call, json!({"result": "contents"})),
            json!({
                "role": "user",
                "parts": [{"functionResponse": {
                    "name": "read",
                    "response": {"result": "contents"}
                }}]
            })
        );
    }

    #[test]
    fn rejects_non_object_function_arguments() {
        let mut text = String::new();
        let mut parts: Vec<Value> = Vec::new();
        let error = consume_sse_event(
            r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"read","args":"bad"}}]}}]}"#,
            &mut text,
            &mut parts,
        )
        .unwrap_err();
        assert!(error.to_string().contains("arguments were not an object"));
    }
}
