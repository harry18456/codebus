//! Codex CLI `--json` JSONL → neutral [`StreamEvent`] parser.
//!
//! Format-only mapping (see `codex-backend` spec `Codex Stream Parsing`).
//! Real mapping implemented in task 3.4; these are stubs so the backend
//! compiles for the argv tests in task 3.1/3.2.

use super::parser::{StreamEvent, ToolKind};
use crate::log::TokenUsage;
use serde_json::{Value, json};

/// Map one line of codex `--json` output to zero or more [`StreamEvent`]s.
/// Format-only; malformed JSON and unhandled event types yield an empty vec
/// (forward-compat). Does NOT interpret `[CODEBUS_*]` markers.
pub fn parse_codex_stream_line(raw: &str) -> Vec<StreamEvent> {
    let v: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    match v.get("type").and_then(Value::as_str).unwrap_or("") {
        "item.completed" => {
            let Some(item) = v.get("item") else {
                return Vec::new();
            };
            match item.get("type").and_then(Value::as_str).unwrap_or("") {
                "command_execution" => {
                    let command = item.get("command").and_then(Value::as_str).unwrap_or("");
                    let output = item
                        .get("aggregated_output")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    // exit_code may be null while in_progress; absent/null → not an error.
                    let is_error = item
                        .get("exit_code")
                        .and_then(Value::as_i64)
                        .map(|c| c != 0)
                        .unwrap_or(false);
                    // Codex wire today does not carry `tool_kind`; the field is
                    // read defensively so that if/when codex adds it, this
                    // parser forwards it without further code change. Unknown
                    // enum values cause the entire item to be skipped.
                    let tool_kind = match item.get("tool_kind") {
                        None | Some(Value::Null) => None,
                        Some(v) => match serde_json::from_value::<ToolKind>(v.clone()) {
                            Ok(k) => Some(k),
                            Err(_) => return Vec::new(),
                        },
                    };
                    vec![
                        StreamEvent::ToolUse {
                            name: "Shell".to_string(),
                            input: json!({ "command": command }),
                            tool_kind,
                        },
                        StreamEvent::ToolResult { output, is_error },
                    ]
                }
                "agent_message" => {
                    let text = item
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    vec![StreamEvent::Thought { text }]
                }
                _ => Vec::new(),
            }
        }
        "turn.completed" => {
            let Some(usage) = v.get("usage") else {
                return Vec::new();
            };
            let g = |k: &str| usage.get(k).and_then(Value::as_u64);
            vec![StreamEvent::Usage(TokenUsage {
                input_tokens: g("input_tokens").unwrap_or(0),
                output_tokens: g("output_tokens").unwrap_or(0),
                cache_read_tokens: g("cached_input_tokens"),
                cache_write_tokens: None,
                reasoning_tokens: g("reasoning_output_tokens"),
                extras: usage.clone(),
            })]
        }
        _ => Vec::new(),
    }
}

/// Extract the codex session id from a `thread.started` line; `None` for any
/// other line or malformed JSON.
pub fn sniff_codex_thread_id(raw: &str) -> Option<String> {
    let v: Value = serde_json::from_str(raw).ok()?;
    if v.get("type").and_then(Value::as_str) == Some("thread.started") {
        return v
            .get("thread_id")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: command_execution maps to a ToolUse + ToolResult pair.
    #[test]
    fn command_execution_maps_to_tooluse_and_toolresult() {
        let line = r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"echo hi","aggregated_output":"hi\n","exit_code":0,"status":"completed"}}"#;
        let events = parse_codex_stream_line(line);
        assert_eq!(events.len(), 2, "got {events:?}");
        match &events[0] {
            StreamEvent::ToolUse { name, input, tool_kind } => {
                assert_eq!(name, "Shell");
                assert_eq!(input.get("command").and_then(|v| v.as_str()), Some("echo hi"));
                assert_eq!(*tool_kind, None);
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
        match &events[1] {
            StreamEvent::ToolResult { output, is_error } => {
                assert_eq!(output, "hi\n");
                assert!(!is_error);
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    /// Spec: non-zero exit_code marks the tool result as an error.
    #[test]
    fn nonzero_exit_marks_tool_result_error() {
        let line = r#"{"type":"item.completed","item":{"type":"command_execution","command":"false","aggregated_output":"","exit_code":1,"status":"completed"}}"#;
        let events = parse_codex_stream_line(line);
        match events.last().expect("a result event") {
            StreamEvent::ToolResult { is_error, .. } => assert!(is_error),
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    /// Spec: agent_message maps to a Thought.
    #[test]
    fn agent_message_maps_to_thought() {
        let line = r#"{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"DONE"}}"#;
        let events = parse_codex_stream_line(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Thought { text } => assert_eq!(text, "DONE"),
            other => panic!("expected Thought, got {other:?}"),
        }
    }

    /// Spec: turn.completed maps usage tokens (with field mapping).
    #[test]
    fn turn_completed_maps_usage() {
        let line = r#"{"type":"turn.completed","usage":{"input_tokens":30515,"cached_input_tokens":22272,"output_tokens":43,"reasoning_output_tokens":17}}"#;
        let events = parse_codex_stream_line(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Usage(u) => {
                assert_eq!(u.input_tokens, 30515);
                assert_eq!(u.output_tokens, 43);
                assert_eq!(u.cache_read_tokens, Some(22272));
                assert_eq!(u.reasoning_tokens, Some(17));
            }
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    /// Spec: thread.started yields the session id and no StreamEvent.
    #[test]
    fn thread_started_yields_session_id_and_no_event() {
        let line = r#"{"type":"thread.started","thread_id":"019e4d0e-abc"}"#;
        assert_eq!(sniff_codex_thread_id(line), Some("019e4d0e-abc".to_string()));
        assert!(parse_codex_stream_line(line).is_empty());
    }

    /// turn.started / item.started produce no events.
    #[test]
    fn lifecycle_lines_produce_no_events() {
        assert!(parse_codex_stream_line(r#"{"type":"turn.started"}"#).is_empty());
        assert!(parse_codex_stream_line(r#"{"type":"item.started","item":{"type":"command_execution","status":"in_progress"}}"#).is_empty());
    }

    /// Malformed JSON returns an empty vec (forward-compat).
    #[test]
    fn malformed_json_returns_empty() {
        assert!(parse_codex_stream_line("not json").is_empty());
        assert_eq!(sniff_codex_thread_id("not json"), None);
    }

    /// Spec: when codex CLI emits a `tool_kind` on a command_execution
    /// item, the codex parser SHALL forward it onto the resulting
    /// ToolUse event with the same value the Claude parser would produce.
    #[test]
    fn codex_parser_forwards_tool_kind() {
        let line = r#"{"type":"item.completed","item":{"type":"command_execution","command":"git commit -m x","aggregated_output":"","exit_code":0,"status":"completed","tool_kind":"mutation"}}"#;
        let events = parse_codex_stream_line(line);
        assert_eq!(events.len(), 2);
        match &events[0] {
            StreamEvent::ToolUse { tool_kind, .. } => {
                assert_eq!(*tool_kind, Some(ToolKind::Mutation));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    /// Spec: codex command_execution items that omit `tool_kind` (the
    /// current production wire format) yield `tool_kind: None` and remain
    /// valid two-event ToolUse + ToolResult pairs.
    #[test]
    fn codex_parser_without_tool_kind_is_none() {
        let line = r#"{"type":"item.completed","item":{"type":"command_execution","command":"ls","aggregated_output":"","exit_code":0,"status":"completed"}}"#;
        let events = parse_codex_stream_line(line);
        assert_eq!(events.len(), 2);
        match &events[0] {
            StreamEvent::ToolUse { tool_kind, name, .. } => {
                assert_eq!(name, "Shell");
                assert_eq!(*tool_kind, None);
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }
}
