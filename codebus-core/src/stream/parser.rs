//! Stream-JSON parser for the `claude -p --output-format stream-json --verbose`
//! subprocess output.
//!
//! Schema verified by spike against claude CLI 2.1.x. Real shapes:
//!
//! ```text
//! {type:"system", subtype:...}                                       → skip
//! {type:"assistant", message:{content:[{type:"text"|"tool_use"|"thinking"}]}}
//! {type:"user",      message:{content:[{type:"tool_result"}]}}
//! {type:"rate_limit_event"}                                          → skip
//! {type:"result",    usage:{input_tokens, output_tokens,
//!                           cache_creation_input_tokens,
//!                           cache_read_input_tokens, ...}, ...}      → emit Usage
//! ```
//!
//! `assistant.content[]` can hold multiple items per line (text + tool_use
//! together), so [`parse_claude_stream_line`] returns 0..N events and the
//! caller iterates. Malformed JSON returns an empty vec instead of erroring
//! — forward-compat for unknown future event types and for partial output
//! at the very end of a stream.
//!
//! ## Token usage extraction
//!
//! When a `type: "result"` event arrives, the parser maps the Anthropic
//! `usage` object onto the provider-agnostic [`TokenUsage`] shape:
//!
//! - `input_tokens` → `input_tokens`
//! - `output_tokens` → `output_tokens`
//! - `cache_read_input_tokens` → `cache_read_tokens` (Some)
//! - `cache_creation_input_tokens` → `cache_write_tokens` (Some)
//! - the original `usage` object verbatim → `extras`
//!
//! Future providers (OpenAI / Ollama / Gemini) will emit their own
//! `StreamEvent::Usage` events using this same normalized struct so the
//! consumer (verb commands) accumulates uniformly.

use crate::log::TokenUsage;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Semantic classification of a tool-use event, used by GUI 2-phase cluster
/// rendering (READING CODEBASE / WRITING WIKI) and any future phase-aware
/// consumer. Emitted by the agent skill, NOT inferred by the parser — see
/// `agent-stream-rendering` spec § "Stream Event Tool Classification".
///
/// Field name SHALL remain `tool_kind` (not `kind`) on the `ToolUse` variant
/// because the outer [`StreamEvent`] already uses `kind` as its serde tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Read,
    Inspect,
    Mutation,
    OtherRead,
    OtherWrite,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StreamEvent {
    Thought {
        text: String,
    },
    ToolUse {
        name: String,
        input: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_kind: Option<ToolKind>,
    },
    ToolResult {
        output: String,
        is_error: bool,
    },
    Usage(TokenUsage),
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
        Some("result") => parse_result_event(&parsed),
        // system / rate_limit_event / unknown future → skip
        _ => Vec::new(),
    }
}

/// Map the Claude CLI `result` event's `usage` field onto the
/// provider-agnostic [`TokenUsage`] shape and emit a `Usage` event.
/// Falls back to no events when the line lacks a usage object.
fn parse_result_event(parsed: &Value) -> Vec<StreamEvent> {
    // Claude CLI puts `usage` at the top of the result event. Defensive
    // alternates cover hypothetical future shapes (`result.usage` /
    // `message.usage`) so a CLI version bump doesn't silently drop usage.
    let usage = parsed
        .get("usage")
        .or_else(|| parsed.get("result").and_then(|r| r.get("usage")))
        .or_else(|| parsed.get("message").and_then(|m| m.get("usage")));

    let Some(usage) = usage else {
        return Vec::new();
    };

    let input_tokens = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read_tokens = usage.get("cache_read_input_tokens").and_then(Value::as_u64);
    let cache_write_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64);

    vec![StreamEvent::Usage(TokenUsage {
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
        reasoning_tokens: None,
        extras: usage.clone(),
    })]
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
                // Extract optional `tool_kind` field; unknown enum values
                // cause this entire content item to be skipped (per spec
                // Failure modes — reject the event rather than silently
                // dropping the classification).
                let tool_kind = match item.get("tool_kind") {
                    None | Some(Value::Null) => None,
                    Some(v) => match serde_json::from_value::<ToolKind>(v.clone()) {
                        Ok(k) => Some(k),
                        Err(_) => continue,
                    },
                };
                events.push(StreamEvent::ToolUse {
                    name,
                    input,
                    tool_kind,
                });
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
    fn parses_assistant_tool_use_with_input_preserved() {
        let line = json!({
            "type": "assistant",
            "message": {
                "content": [{
                    "type": "tool_use",
                    "name": "Read",
                    "input": { "file_path": "/x.rs" }
                }]
            }
        })
        .to_string();
        assert_eq!(
            parse_claude_stream_line(&line),
            vec![StreamEvent::ToolUse {
                name: "Read".into(),
                input: json!({ "file_path": "/x.rs" }),
                tool_kind: None,
            }]
        );
    }

    #[test]
    fn empty_text_item_is_skipped() {
        let line = json!({
            "type": "assistant",
            "message": { "content": [{ "type": "text", "text": "" }] }
        })
        .to_string();
        assert!(parse_claude_stream_line(&line).is_empty());
    }

    #[test]
    fn thinking_items_are_skipped() {
        let line = json!({
            "type": "assistant",
            "message": { "content": [{ "type": "thinking", "thinking": "internal" }] }
        })
        .to_string();
        assert!(parse_claude_stream_line(&line).is_empty());
    }

    #[test]
    fn parses_user_tool_result_array_form_joined() {
        let line = json!({
            "type": "user",
            "message": {
                "content": [{
                    "type": "tool_result",
                    "content": [
                        { "type": "text", "text": "line1\n" },
                        { "type": "text", "text": "line2" }
                    ],
                    "is_error": false
                }]
            }
        })
        .to_string();
        assert_eq!(
            parse_claude_stream_line(&line),
            vec![StreamEvent::ToolResult {
                output: "line1\nline2".into(),
                is_error: false
            }]
        );
    }

    #[test]
    fn parses_user_tool_result_string_form() {
        let line = json!({
            "type": "user",
            "message": {
                "content": [{
                    "type": "tool_result",
                    "content": "single string body",
                    "is_error": true
                }]
            }
        })
        .to_string();
        assert_eq!(
            parse_claude_stream_line(&line),
            vec![StreamEvent::ToolResult {
                output: "single string body".into(),
                is_error: true
            }]
        );
    }

    #[test]
    fn parses_result_event_with_usage_into_usage_event() {
        let line = json!({
            "type": "result",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50,
                "cache_read_input_tokens": 10,
                "cache_creation_input_tokens": 5
            }
        })
        .to_string();
        let events = parse_claude_stream_line(&line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Usage(u) => {
                assert_eq!(u.input_tokens, 100);
                assert_eq!(u.output_tokens, 50);
                assert_eq!(u.cache_read_tokens, Some(10));
                assert_eq!(u.cache_write_tokens, Some(5));
                assert!(u.reasoning_tokens.is_none());
            }
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn result_event_without_usage_emits_nothing() {
        let line = json!({"type": "result", "subtype": "end_turn"}).to_string();
        assert!(parse_claude_stream_line(&line).is_empty());
    }

    #[test]
    fn system_event_returns_empty() {
        let line = json!({"type": "system", "subtype": "init"}).to_string();
        assert!(parse_claude_stream_line(&line).is_empty());
    }

    #[test]
    fn unknown_future_type_returns_empty() {
        let line = json!({"type": "future_event", "anything": true}).to_string();
        assert!(parse_claude_stream_line(&line).is_empty());
    }

    #[test]
    fn malformed_json_returns_empty_vec_no_panic() {
        let result =
            parse_claude_stream_line(r#"{"type":"assistant","message":{"content":[{"type":"#);
        assert!(result.is_empty());
    }

    #[test]
    fn multi_item_assistant_content_emits_in_declaration_order() {
        let line = json!({
            "type": "assistant",
            "message": {
                "content": [
                    { "type": "text", "text": "calling" },
                    { "type": "tool_use", "name": "Grep", "input": {} }
                ]
            }
        })
        .to_string();
        let events = parse_claude_stream_line(&line);
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], StreamEvent::Thought { text } if text == "calling"));
        assert!(matches!(&events[1], StreamEvent::ToolUse { name, .. } if name == "Grep"));
    }

    /// Spec: agent-stream-rendering § Stream Event Tool Classification —
    /// every variant round-trips through serde with snake_case wire form.
    #[test]
    fn tool_kind_enum_serde_round_trip() {
        for (variant, wire) in [
            (ToolKind::Read, "\"read\""),
            (ToolKind::Inspect, "\"inspect\""),
            (ToolKind::Mutation, "\"mutation\""),
            (ToolKind::OtherRead, "\"other_read\""),
            (ToolKind::OtherWrite, "\"other_write\""),
        ] {
            let serialized = serde_json::to_string(&variant).unwrap();
            assert_eq!(serialized, wire, "serialize {variant:?}");
            let deserialized: ToolKind = serde_json::from_str(wire).unwrap();
            assert_eq!(deserialized, variant, "deserialize {wire}");
        }
    }

    /// Spec: ToolUse line that omits `tool_kind` deserializes with
    /// `tool_kind: None` and SHALL NOT log warnings or errors.
    #[test]
    fn tooluse_without_tool_kind_deserializes_as_none() {
        let line = json!({
            "type": "assistant",
            "message": {
                "content": [{
                    "type": "tool_use",
                    "name": "Read",
                    "input": { "file_path": "/x.rs" }
                }]
            }
        })
        .to_string();
        let events = parse_claude_stream_line(&line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ToolUse { tool_kind, .. } => assert_eq!(*tool_kind, None),
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    /// Spec: a `tool_kind` value present in the input SHALL be carried
    /// through as `Some(ToolKind::...)` on the resulting ToolUse event.
    #[test]
    fn parse_assistant_with_tool_kind() {
        let line = json!({
            "type": "assistant",
            "message": {
                "content": [{
                    "type": "tool_use",
                    "name": "Bash",
                    "input": { "command": "git status" },
                    "tool_kind": "inspect"
                }]
            }
        })
        .to_string();
        let events = parse_claude_stream_line(&line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ToolUse {
                name,
                tool_kind,
                input,
            } => {
                assert_eq!(name, "Bash");
                assert_eq!(*tool_kind, Some(ToolKind::Inspect));
                assert_eq!(input.get("command").and_then(Value::as_str), Some("git status"));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    /// Spec § Failure modes 2: an unknown `tool_kind` enum value SHALL
    /// cause the entire content item to be skipped (rather than panicking
    /// or silently dropping the classification).
    #[test]
    fn parse_assistant_unknown_tool_kind_drops_line() {
        let line = json!({
            "type": "assistant",
            "message": {
                "content": [{
                    "type": "tool_use",
                    "name": "Bash",
                    "input": { "command": "rm -rf /" },
                    "tool_kind": "garbage"
                }]
            }
        })
        .to_string();
        let events = parse_claude_stream_line(&line);
        assert!(events.is_empty(), "expected zero events for unknown tool_kind, got {events:?}");
    }
}
