//! Lightweight markdown styling and OSC 8 hyperlink helpers for the
//! terminal renderer. Pure string transforms — no I/O, no terminal
//! detection. Callers (e.g. `TerminalRenderer`) decide when to invoke
//! these based on `use_color`, hyperlink support, and event kind so
//! `tool_use` / `tool_result` payloads stay byte-equal with fixtures.

use regex::Regex;
use std::sync::OnceLock;

fn bold_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\*\*([^*]+)\*\*").expect("bold regex compiles"))
}

fn code_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"`([^`]+)`").expect("code regex compiles"))
}

fn wikilink_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[\[([^\]]+)\]\]").expect("wikilink regex compiles"))
}

/// Apply lightweight markdown styling to `thought` text.
///
/// When `use_color` is `false`, returns `text` byte-for-byte unchanged so
/// `NO_COLOR` runs and fixture comparisons stay stable. When `true`,
/// applies three transforms in order (bold → inline code → wikilink) so
/// markers don't chew into one another:
///
/// - `**bold**` → ANSI bold (markers stripped)
/// - `` `code` `` → cyan (backticks stripped)
/// - `[[slug]]` → cyan + underline (brackets preserved as visible text)
///
/// Partial / unmatched markers (e.g. a lone `**`) are left as-is — the
/// regex requires both delimiters and a non-empty inner run.
pub fn style_thought_text(text: &str, use_color: bool) -> String {
    if !use_color {
        return text.to_string();
    }

    let bolded = bold_re().replace_all(text, "\x1b[1m$1\x1b[22m");
    let coded = code_re().replace_all(&bolded, "\x1b[36m$1\x1b[39m");
    let linked = wikilink_re().replace_all(&coded, "\x1b[36m\x1b[4m[[$1]]\x1b[24m\x1b[39m");
    linked.into_owned()
}

/// Wrap `text` in an OSC 8 hyperlink escape pointing at `uri`.
///
/// Parameter order is `(uri, text)` — read as "the URI of the link, then
/// the visible text", mirroring HTML `<a href="...">text</a>`.
///
/// Emits exactly `ESC ]8;; <uri> ESC \ <text> ESC ]8;; ESC \` with no
/// conditional logic. Caller is responsible for:
/// - not double-wrapping already-wrapped text,
/// - gating on `supports-hyperlinks` detection and `use_color`.
pub fn wrap_osc8(uri: &str, text: &str) -> String {
    format!("\x1b]8;;{uri}\x1b\\{text}\x1b]8;;\x1b\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_marker_renders_with_ansi_bold_escape() {
        let out = style_thought_text("**important**", true);
        assert!(out.contains("\x1b[1m"), "missing bold start: {out:?}");
        assert!(out.contains("\x1b[22m"), "missing bold end: {out:?}");
        assert!(out.contains("important"), "missing inner text: {out:?}");
        assert!(!out.contains("**"), "raw `**` markers leaked: {out:?}");
    }

    #[test]
    fn inline_code_renders_cyan() {
        let out = style_thought_text("`slug`", true);
        assert!(out.contains("\x1b[36m"), "missing cyan start: {out:?}");
        assert!(out.contains("\x1b[39m"), "missing cyan end: {out:?}");
        assert!(out.contains("slug"), "missing inner text: {out:?}");
        assert!(!out.contains('`'), "raw backticks leaked: {out:?}");
    }

    #[test]
    fn wikilink_renders_cyan_with_underline() {
        let out = style_thought_text("[[some-slug]]", true);
        assert!(out.contains("\x1b[36m"), "missing cyan: {out:?}");
        assert!(out.contains("\x1b[4m"), "missing underline: {out:?}");
        assert!(
            out.contains("[[some-slug]]"),
            "brackets must be preserved: {out:?}"
        );
    }

    #[test]
    fn use_color_false_produces_no_styling() {
        let input = "**bold** and `code` and [[wikilink]]";
        let out = style_thought_text(input, false);
        assert_eq!(out.as_bytes(), input.as_bytes());
    }

    #[test]
    fn tool_event_text_is_not_styled() {
        // Pure function is event-kind-agnostic. With use_color=false, even
        // text that LOOKS like markers must be returned verbatim. Caller
        // (TerminalRenderer) is responsible for not invoking this on
        // tool_use / tool_result payloads.
        let raw = r#"{"path": "**not bold**"}"#;
        let out = style_thought_text(raw, false);
        assert_eq!(out.as_bytes(), raw.as_bytes());
    }

    #[test]
    fn wrap_osc8_emits_correct_escape_sequence() {
        let out = wrap_osc8("obsidian://open?vault=abc&file=foo", "[[foo]]");
        assert_eq!(
            out,
            "\x1b]8;;obsidian://open?vault=abc&file=foo\x1b\\[[foo]]\x1b]8;;\x1b\\"
        );
    }
}
