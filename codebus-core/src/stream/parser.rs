//! Stream-JSON parser for the `claude -p` subprocess output.
//!
//! Schema verified by spike against claude CLI 2.1.126 (TS iter-8 lesson —
//! the previously-assumed `{type: "stream_event"}` wrapper does not exist).
//! Real shapes:
//!
//! ```text
//! {type:"system", subtype:...}                                      → skip
//! {type:"assistant", message:{content:[{type:"text"|"tool_use"|"thinking"}]}}
//! {type:"user",      message:{content:[{type:"tool_result"}]}}
//! {type:"rate_limit_event"}                                          → skip
//! {type:"result",    subtype:...}                                    → skip
//! ```
//!
//! `assistant.content[]` can hold multiple items per line (text + tool_use
//! together), so [`parse_claude_stream_line`] returns 0..N events and the
//! caller iterates. Malformed JSON returns an empty vec instead of erroring
//! — forward-compat for unknown future event types and for partial output
//! at the very end of a stream.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StreamEvent {
    Thought { text: String },
    ToolUse { name: String, input: Value },
    ToolResult { output: String, is_error: bool },
    Done,
}

pub fn parse_claude_stream_line(raw: &str) -> Vec<StreamEvent> {
    let parsed: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let outer_type = parsed.get("type").and_then(Value::as_str);

    match outer_type {
        Some("assistant") => parse_assistant_content(&parsed),
        Some("user") => parse_user_content(&parsed),
        // system / result / rate_limit_event / unknown future → skip
        _ => Vec::new(),
    }
}

fn parse_assistant_content(parsed: &Value) -> Vec<StreamEvent> {
    let Some(items) = parsed
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut events = Vec::with_capacity(items.len());
    for item in items {
        let item_type = item.get("type").and_then(Value::as_str);
        match item_type {
            Some("text") => {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        events.push(StreamEvent::Thought {
                            text: text.to_string(),
                        });
                    }
                }
            }
            Some("tool_use") => {
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let input = item.get("input").cloned().unwrap_or(Value::Null);
                events.push(StreamEvent::ToolUse { name, input });
            }
            // 'thinking' items skipped — internal reasoning, not user-facing
            _ => {}
        }
    }
    events
}

fn parse_user_content(parsed: &Value) -> Vec<StreamEvent> {
    let Some(items) = parsed
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut events = Vec::with_capacity(items.len());
    for item in items {
        if item.get("type").and_then(Value::as_str) != Some("tool_result") {
            continue;
        }
        let output = match item.get("content") {
            Some(Value::Array(arr)) => arr
                .iter()
                .map(|c| c.get("text").and_then(Value::as_str).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(""),
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
        let is_error = item
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        events.push(StreamEvent::ToolResult { output, is_error });
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_assistant_text_as_thought() {
        let line = json!({
            "type": "assistant",
            "message": { "content": [{ "type": "text", "text": "hello" }] }
        })
        .to_string();
        assert_eq!(
            parse_claude_stream_line(&line),
            vec![StreamEvent::Thought {
                text: "hello".into()
            }]
        );
    }

    #[test]
    fn parses_assistant_tool_use() {
        let line = json!({
            "type": "assistant",
            "message": { "content": [{ "type": "tool_use", "name": "Read", "input": { "path": "a" } }] }
        })
        .to_string();
        assert_eq!(
            parse_claude_stream_line(&line),
            vec![StreamEvent::ToolUse {
                name: "Read".into(),
                input: json!({ "path": "a" })
            }]
        );
    }

    #[test]
    fn parses_user_tool_result_success_with_array_content() {
        let line = json!({
            "type": "user",
            "message": { "content": [{ "type": "tool_result", "content": [{ "text": "ok" }] }] }
        })
        .to_string();
        assert_eq!(
            parse_claude_stream_line(&line),
            vec![StreamEvent::ToolResult {
                output: "ok".into(),
                is_error: false
            }]
        );
    }

    #[test]
    fn parses_user_tool_result_error_with_string_content() {
        let line = json!({
            "type": "user",
            "message": { "content": [{ "type": "tool_result", "content": "fail", "is_error": true }] }
        })
        .to_string();
        assert_eq!(
            parse_claude_stream_line(&line),
            vec![StreamEvent::ToolResult {
                output: "fail".into(),
                is_error: true
            }]
        );
    }

    #[test]
    fn returns_multiple_events_and_skips_thinking() {
        let line = json!({
            "type": "assistant",
            "message": {
                "content": [
                    { "type": "thinking", "thinking": "internal" },
                    { "type": "text", "text": "visible" },
                    { "type": "tool_use", "name": "Grep", "input": { "pattern": "x" } }
                ]
            }
        })
        .to_string();
        assert_eq!(
            parse_claude_stream_line(&line),
            vec![
                StreamEvent::Thought {
                    text: "visible".into()
                },
                StreamEvent::ToolUse {
                    name: "Grep".into(),
                    input: json!({ "pattern": "x" })
                },
            ]
        );
    }

    #[test]
    fn returns_empty_for_system_result_rate_limit_unknown() {
        for line in [
            json!({ "type": "system", "subtype": "init" }).to_string(),
            json!({ "type": "result", "subtype": "success" }).to_string(),
            json!({ "type": "rate_limit_event" }).to_string(),
            json!({ "type": "totally_unknown_future" }).to_string(),
        ] {
            assert_eq!(parse_claude_stream_line(&line), vec![], "line: {line}");
        }
    }

    #[test]
    fn returns_empty_for_malformed_json_or_empty_string() {
        assert_eq!(parse_claude_stream_line("{{{not valid json"), vec![]);
        assert_eq!(parse_claude_stream_line(""), vec![]);
    }

    #[test]
    fn assistant_without_message_content_array_returns_empty() {
        // Forward-compat: schema variants where message.content is missing
        // or non-array must NOT panic.
        let line = json!({ "type": "assistant", "message": {} }).to_string();
        assert_eq!(parse_claude_stream_line(&line), vec![]);
        let line =
            json!({ "type": "assistant", "message": { "content": "not-an-array" } }).to_string();
        assert_eq!(parse_claude_stream_line(&line), vec![]);
    }

    #[test]
    fn empty_text_item_is_skipped() {
        // Mirrors TS `if (item.text)` truthiness check — empty string is falsy.
        let line = json!({
            "type": "assistant",
            "message": { "content": [{ "type": "text", "text": "" }] }
        })
        .to_string();
        assert_eq!(parse_claude_stream_line(&line), vec![]);
    }

    #[test]
    fn tool_use_with_missing_name_yields_empty_name_string() {
        let line = json!({
            "type": "assistant",
            "message": { "content": [{ "type": "tool_use", "input": {} }] }
        })
        .to_string();
        assert_eq!(
            parse_claude_stream_line(&line),
            vec![StreamEvent::ToolUse {
                name: String::new(),
                input: json!({})
            }]
        );
    }
}
