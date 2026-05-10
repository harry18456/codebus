//! Terminal rendering of [`StreamEvent`] values from the agent's stream-json
//! output. v3-run-log carry from v2's `render::renderers::terminal::format_event`,
//! adapted to v3's plain `RenderOptions` struct (no `EventRenderer` trait).
//!
//! Mapping per `agent-stream-rendering` spec `Stream Event Terminal Rendering`:
//!
//! - `Thought { text }` — `🤔 [Agent 思考]` / `◆` ASCII; multi-line text
//!   indented on a new line, single-line text appended same-line
//! - `ToolUse { name: "Write" | "Edit", file_path }` — specialized
//!   `✍️ [正在生成]` / `+`, body shows the forward-slash-normalized path
//! - `ToolUse { other }` — `🛠️ [呼叫工具]` / `→`, body shows
//!   `<name>(<input summary>)`
//! - `ToolResult { output, is_error }` — `👀 [觀察結果]` / `←`, body
//!   truncated to 200 chars, Write-success echo suppressed entirely
//! - `Usage` — empty string (consumed for `RunLog` accumulation)

use crate::render::options::RenderOptions;
use crate::stream::StreamEvent;
use serde_json::Value;

const INDENT: &str = "    ";

fn lead(emoji: &'static str, symbol: &'static str, opts: &RenderOptions) -> &'static str {
    if opts.use_emoji { emoji } else { symbol }
}

fn normalize_path(p: &str) -> String {
    p.replace('\\', "/")
}

fn indent(body: &str) -> String {
    body.lines()
        .map(|line| format!("{INDENT}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format a single `StreamEvent` to its terminal-friendly multi-line string.
/// Empty string when the event SHOULD NOT be displayed (e.g. `Usage`,
/// Write-success echo).
pub fn format_event(event: &StreamEvent, opts: &RenderOptions) -> String {
    match event {
        StreamEvent::Thought { text } => {
            let label = format!("{} [Agent 思考]", lead("🤔", "◆", opts));
            if text.contains('\n') {
                format!("{label}\n{}", indent(text))
            } else {
                format!("{label} {text}")
            }
        }
        StreamEvent::ToolUse { name, input } => {
            if name == "Write" || name == "Edit" {
                let fp = input
                    .get("file_path")
                    .and_then(Value::as_str)
                    .map(normalize_path)
                    .unwrap_or_else(|| "(unknown)".into());
                format!(
                    "{} [正在生成]\n{INDENT}{fp}",
                    lead("✍️", "+", opts)
                )
            } else {
                let args = format_tool_args(input);
                format!(
                    "{} [呼叫工具]\n{INDENT}{name}({args})",
                    lead("🛠️", "→", opts)
                )
            }
        }
        StreamEvent::ToolResult { output, is_error } => {
            if is_write_success_echo(output) {
                return String::new();
            }
            let body = if let Some(n) = read_line_count(output) {
                format!("({n} lines)")
            } else if output.chars().count() > 200 {
                let mut t: String = output.chars().take(200).collect();
                t.push('…');
                t
            } else {
                output.clone()
            };
            // is_error already conveyed by the tool's own output; we don't
            // duplicate the signal in the renderer.
            let _ = is_error;
            format!(
                "{} [觀察結果]\n{}",
                lead("👀", "←", opts),
                indent(&body)
            )
        }
        StreamEvent::Usage(_) => String::new(),
    }
}

/// Print formatted event to stdout (newline-terminated). Empty strings are
/// suppressed entirely so a `Usage` or Write-success echo doesn't print a
/// blank line.
pub fn print_event(event: &StreamEvent, opts: &RenderOptions) {
    let s = format_event(event, opts);
    if !s.is_empty() {
        println!("{s}");
    }
}

/// Compact key=value summary of tool input. Best-effort: known string fields
/// get shown verbatim, complex objects get JSON-stringified short.
fn format_tool_args(input: &Value) -> String {
    match input {
        Value::Object(obj) if !obj.is_empty() => obj
            .iter()
            .map(|(k, v)| format!("{k}={}", short_value(v)))
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(_) => String::new(),
        other => other.to_string(),
    }
}

fn short_value(v: &Value) -> String {
    match v {
        Value::String(s) => format!("\"{s}\""),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
        Value::Array(a) => format!("[{} items]", a.len()),
        Value::Object(o) => format!("{{{} keys}}", o.len()),
    }
}

/// Detect Claude's standard Write/Edit tool success echoes. These would
/// duplicate the Write/Edit ToolUse banner that already showed the file
/// path, so suppress them entirely.
fn is_write_success_echo(output: &str) -> bool {
    output.starts_with("File created successfully")
        || output.starts_with("The file ")
        || output.starts_with("File updated successfully")
}

/// Recognize the Read tool's `<path>(<N> lines)` return form so we can show
/// the compact line count instead of dumping the whole file body.
fn read_line_count(output: &str) -> Option<usize> {
    let open = output.rfind('(')?;
    let close = output.rfind(" lines)")?;
    if close <= open {
        return None;
    }
    let n: usize = output[open + 1..close].parse().ok()?;
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::TokenUsage;
    use serde_json::json;

    fn emoji_on() -> RenderOptions {
        RenderOptions::explicit(true, false, false, None)
    }

    fn emoji_off() -> RenderOptions {
        RenderOptions::no_styling()
    }

    /// Spec scenario: "Thought single-line text appends body to label line"
    #[test]
    fn thought_single_line_appends_to_label() {
        let s = format_event(
            &StreamEvent::Thought {
                text: "hello".into(),
            },
            &emoji_on(),
        );
        assert_eq!(s, "🤔 [Agent 思考] hello");
    }

    /// Spec scenario: "Thought multi-line text indents body on new line"
    #[test]
    fn thought_multi_line_indents_body() {
        let s = format_event(
            &StreamEvent::Thought {
                text: "line1\nline2".into(),
            },
            &emoji_on(),
        );
        assert_eq!(s, "🤔 [Agent 思考]\n    line1\n    line2");
    }

    /// Spec scenario: "Thought ASCII fallback uses diamond glyph"
    #[test]
    fn thought_ascii_fallback_uses_diamond() {
        let s = format_event(
            &StreamEvent::Thought { text: "x".into() },
            &emoji_off(),
        );
        assert!(s.starts_with("◆ [Agent 思考]"), "got: {s:?}");
    }

    /// Spec scenario: "ToolUse Write specialization shows file_path"
    #[test]
    fn tooluse_write_special_shows_file_path() {
        let s = format_event(
            &StreamEvent::ToolUse {
                name: "Write".into(),
                input: json!({"file_path": "/repo/wiki/foo.md"}),
            },
            &emoji_on(),
        );
        assert_eq!(s, "✍️ [正在生成]\n    /repo/wiki/foo.md");
    }

    #[test]
    fn tooluse_write_missing_file_path_uses_unknown() {
        let s = format_event(
            &StreamEvent::ToolUse {
                name: "Write".into(),
                input: json!({}),
            },
            &emoji_on(),
        );
        assert!(s.contains("(unknown)"), "got: {s:?}");
    }

    /// Spec scenario: "ToolUse Read formats name with input summary"
    #[test]
    fn tooluse_read_formats_name_and_args() {
        let s = format_event(
            &StreamEvent::ToolUse {
                name: "Read".into(),
                input: json!({"file_path": "/x"}),
            },
            &emoji_on(),
        );
        assert!(s.starts_with("🛠️ [呼叫工具]"), "got: {s:?}");
        assert!(s.contains("Read("), "got: {s:?}");
    }

    /// Spec scenario: "ToolResult truncates long output at 200 chars"
    #[test]
    fn toolresult_truncates_long_output_at_200_chars() {
        let long: String = "x".repeat(500);
        let s = format_event(
            &StreamEvent::ToolResult {
                output: long.clone(),
                is_error: false,
            },
            &emoji_on(),
        );
        // Body indented; truncated body is 200 chars + …
        let body_marker = "    ";
        let body_start = s.find(body_marker).unwrap() + body_marker.len();
        let body = &s[body_start..];
        // 200 chars then ellipsis
        let body_chars: Vec<char> = body.chars().collect();
        assert_eq!(body_chars.len(), 201);
        assert_eq!(body_chars[200], '…');
    }

    /// Spec scenario: "ToolResult Write-success echo is suppressed"
    #[test]
    fn toolresult_write_success_echo_suppressed() {
        let s = format_event(
            &StreamEvent::ToolResult {
                output: "File created successfully at /x.md".into(),
                is_error: false,
            },
            &emoji_on(),
        );
        assert!(s.is_empty(), "expected suppressed: {s:?}");
    }

    #[test]
    fn toolresult_the_file_echo_suppressed() {
        let s = format_event(
            &StreamEvent::ToolResult {
                output: "The file /x.md has been updated. Here is the result of running `cat -n` on a snippet of the edited file:".into(),
                is_error: false,
            },
            &emoji_on(),
        );
        assert!(s.is_empty(), "expected suppressed: {s:?}");
    }

    #[test]
    fn toolresult_short_output_passes_through() {
        let s = format_event(
            &StreamEvent::ToolResult {
                output: "fn main() {}".into(),
                is_error: false,
            },
            &emoji_on(),
        );
        assert!(s.starts_with("👀 [觀察結果]"));
        assert!(s.contains("fn main() {}"));
    }

    /// Spec scenario: "Usage event renders nothing"
    #[test]
    fn usage_renders_empty_string() {
        let s = format_event(&StreamEvent::Usage(TokenUsage::default()), &emoji_on());
        assert!(s.is_empty());
    }

    /// Spec scenario: "Path normalization uses forward slashes"
    #[test]
    fn path_normalization_uses_forward_slashes() {
        let s = format_event(
            &StreamEvent::ToolUse {
                name: "Write".into(),
                input: json!({"file_path": "C:\\repo\\wiki\\foo.md"}),
            },
            &emoji_on(),
        );
        assert!(s.contains("C:/repo/wiki/foo.md"), "got: {s:?}");
        assert!(!s.contains("\\repo"), "backslash leaked: {s:?}");
    }
}
