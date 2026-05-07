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
//! {type:"result",    usage:{input_tokens, output_tokens,
//!                           cache_creation_input_tokens,
//!                           cache_read_input_tokens, ...}, ...}     → emit Usage
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
//! Future providers (Anthropic API direct / OpenAI / Ollama) will emit
//! their own `StreamEvent::Usage` events using this same normalized
//! struct so the consumer (`run_goal` / `run_query`) accumulates uniformly
//! regardless of which provider produced the data.

use crate::log::TokenUsage;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StreamEvent {
    Thought { text: String },
    ToolUse { name: String, input: Value },
    ToolResult { output: String, is_error: bool },
    Usage(TokenUsage),
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
        Some("result") => parse_result_event(&parsed),
        // system / rate_limit_event / unknown future → skip
        _ => Vec::new(),
    }
}

/// Map the Claude CLI `result` event's `usage` field onto the
/// provider-agnostic [`TokenUsage`] shape and emit a `Usage` event.
/// Falls back to no events when the line lacks a usage object — preserves
/// the previous "skip result events" behavior for streams that don't carry
/// usage data (e.g. mock streams in tests).
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
    fn returns_empty_for_system_rate_limit_unknown_or_result_without_usage() {
        for line in [
            json!({ "type": "system", "subtype": "init" }).to_string(),
            // `result` event without `usage` (e.g., subtype-only stub used
            // in older mock streams) still yields no events — preserves
            // backward compat for tests that don't bother emitting usage.
            json!({ "type": "result", "subtype": "success" }).to_string(),
            json!({ "type": "rate_limit_event" }).to_string(),
            json!({ "type": "totally_unknown_future" }).to_string(),
        ] {
            assert_eq!(parse_claude_stream_line(&line), vec![], "line: {line}");
        }
    }

    #[test]
    fn parses_result_event_with_usage_into_usage_event() {
        // Spec scenario: "Claude CLI invocation populates all four anthropic fields"
        let line = json!({
            "type": "result",
            "subtype": "success",
            "usage": {
                "input_tokens": 1234,
                "output_tokens": 567,
                "cache_creation_input_tokens": 100,
                "cache_read_input_tokens": 8900,
            },
        })
        .to_string();
        let events = parse_claude_stream_line(&line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Usage(u) => {
                assert_eq!(u.input_tokens, 1234);
                assert_eq!(u.output_tokens, 567);
                assert_eq!(u.cache_read_tokens, Some(8900));
                assert_eq!(u.cache_write_tokens, Some(100));
                assert_eq!(u.reasoning_tokens, None);
                // extras carries the original usage object verbatim.
                assert_eq!(
                    u.extras.get("input_tokens").and_then(|v| v.as_u64()),
                    Some(1234)
                );
                assert_eq!(
                    u.extras
                        .get("cache_creation_input_tokens")
                        .and_then(|v| v.as_u64()),
                    Some(100)
                );
            }
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn parses_result_event_without_cache_fields_yields_none_for_cache() {
        // Hypothetical CLI output (or future provider) reports only base
        // counts. Cache fields fall to None to preserve "this provider has
        // no cache concept" semantics.
        let line = json!({
            "type": "result",
            "usage": {
                "input_tokens": 50,
                "output_tokens": 30,
            },
        })
        .to_string();
        let events = parse_claude_stream_line(&line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Usage(u) => {
                assert_eq!(u.input_tokens, 50);
                assert_eq!(u.output_tokens, 30);
                assert!(u.cache_read_tokens.is_none());
                assert!(u.cache_write_tokens.is_none());
            }
            other => panic!("expected Usage, got {other:?}"),
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
