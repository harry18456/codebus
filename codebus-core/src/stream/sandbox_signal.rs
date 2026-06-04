//! Locale-independent OS sandbox / permission-denial detector
//! (run-outcome-lifecycle-integrity, Part B).
//!
//! ## Why this exists
//!
//! codex `exec` exits `0` at the top level even when an inner shell command
//! was blocked by the OS sandbox (PoC-verified 0.135.0). codebus derives a
//! run's `outcome` from the top-level exit code, so a sandbox-blocked run is
//! silently logged as `succeeded`. This detector lets `agent::invoke` count
//! such denials so they are observable in the `RunLog` even though `outcome`
//! is (intentionally, for this change) left unchanged. See the `verb-library`
//! capability `Sandbox Denial Signal Observability` requirement.
//!
//! ## Precision over recall (anti-false-positive)
//!
//! An inner command exiting non-zero is NOT, by itself, a denial — `grep`
//! with no match exits `1`, a failing test exits non-zero, a missing file
//! errors. So this detector keys on a curated set of HIGH-SPECIFICITY,
//! LOCALE-INDEPENDENT permission/sandbox markers that essentially never
//! appear in a benign command failure. It deliberately UNDER-reports
//! (localized-only messages with no .NET / errno token are missed) rather
//! than over-report (寧可少報、不要誤報). The caller MUST only feed outputs
//! from results whose `is_error == true`.
//!
//! ## Locale grounding
//!
//! The authoritative PoC fixture (`agent-cli-research/poc/codex-sandbox/
//! write-acl-run/write_normal_acl.jsonl`) was captured on a zh-TW Windows
//! host: its human-readable denial line is "拒絕存取路徑 …", NOT the English
//! "Access is denied". A naive English-substring detector would have SILENTLY
//! MISSED a real denial on the developer's own machine. The same output,
//! however, carries the locale-independent tokens `PermissionDenied`
//! (PowerShell `CategoryInfo`) and `GetContentWriterUnauthorizedAccessError`
//! (the `FullyQualifiedErrorId`). Note the .NET type name
//! `UnauthorizedAccessException` is wrapped across a line break in that
//! output ("Unauthorized\r\n   AccessException") and is therefore UNRELIABLE
//! as a marker — `PermissionDenied` / `UnauthorizedAccessError` are matched
//! instead.

/// Curated locale-independent sandbox / permission-denial markers, matched
/// case-insensitively as substrings. Each is high-specificity: it does not
/// occur in benign command failures (grep-no-match, test failures, missing
/// files). Keep this list conservative — adding a low-specificity token here
/// would trade the anti-false-positive guarantee for marginal recall.
const DENIAL_MARKERS: &[&str] = &[
    // Windows English locale / generic Win32 error text.
    "access is denied",
    // PowerShell `CategoryInfo` category (locale-independent enum name).
    // Matched the zh-TW PoC where the human-readable message was Chinese.
    "permissiondenied",
    // PowerShell `FullyQualifiedErrorId` fragment (e.g.
    // `GetContentWriterUnauthorizedAccessError`). Deliberately NOT the
    // wrap-prone `UnauthorizedAccessException` .NET type name.
    "unauthorizedaccesserror",
    // Unix EACCES `strerror` text (English).
    "permission denied",
    // Unix EPERM `strerror` text (English).
    "operation not permitted",
];

/// Return `true` when `output` contains any curated locale-independent
/// sandbox / permission-denial marker (case-insensitive). The caller SHALL
/// only pass outputs from tool results whose `is_error == true`; this
/// function does not inspect exit status.
pub fn is_sandbox_denial(output: &str) -> bool {
    let lower = output.to_lowercase();
    DENIAL_MARKERS.iter().any(|m| lower.contains(m))
}

/// Read `reader` line-by-line, count how many lines match
/// [`is_sandbox_denial`], and dispose of EVERY line per `forward`: when
/// `forward` is `true` each line is written to `out` (the parent terminal in
/// production); when `false` the line is discarded (`out` may be an
/// [`std::io::Sink`]). Returns the denial count.
///
/// ## Why classification is independent of `forward`
///
/// (agent-run-integrity, vertical A) The child's stderr is the ONLY surface
/// for a sandbox denial that never produced a stdout `ToolResult`. The
/// `CODEBUS_FORWARD_AGENT_STDERR` escape hatch only decides whether the
/// dev terminal SEES the raw stream — it must NOT gate observability. So
/// this helper always classifies, regardless of the forward disposition, and
/// the caller sums the count into `InvokeReport.sandbox_denial_count` (no
/// de-dup against the stdout source; over-count is acceptable).
///
/// A line that fails UTF-8 decoding is skipped for both classification and
/// forwarding (best-effort, matching the existing "stderr is diagnostic
/// noise" posture). I/O errors mid-stream stop the loop and return the count
/// accumulated so far — the thread's job is best-effort, never fatal.
pub fn classify_stderr_lines<R: std::io::BufRead, W: std::io::Write>(
    reader: R,
    mut out: W,
    forward: bool,
) -> usize {
    let mut count = 0usize;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if is_sandbox_denial(&line) {
            count += 1;
        }
        if forward {
            // Best-effort passthrough; a write failure to the parent
            // terminal must not abort classification of later lines.
            let _ = writeln!(out, "{line}");
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The captured PoC denial output (zh-TW host). Reproduces the
    /// locale-independent tokens from `write_normal_acl.jsonl`'s
    /// `aggregated_output`, including the line-wrapped (and thus unreliable)
    /// `Unauthorized\r\n   AccessException`.
    const POC_DENIAL_OUTPUT: &str = "Set-Content : 拒絕存取路徑 'D:\\side_project\\...\\written.txt'。\r\n\
位於 線路:2 字元:1\r\n\
+ Set-Content -LiteralPath '...'\r\n\
    + CategoryInfo          : PermissionDenied: (D:\\side_project...written.txt:String) [Set-Content], Unauthorized \r\n\
   AccessException\r\n\
    + FullyQualifiedErrorId : GetContentWriterUnauthorizedAccessError,Microsoft.PowerShell.Commands.SetContentCommand\r\n";

    /// Positive: the real PoC denial (zh-TW message) is detected via its
    /// locale-independent tokens.
    #[test]
    fn detects_poc_localized_denial_via_locale_independent_markers() {
        assert!(is_sandbox_denial(POC_DENIAL_OUTPUT));
    }

    /// The detector must NOT depend on the wrap-prone
    /// `UnauthorizedAccessException` token: even an output where that token is
    /// split across a newline (as in the PoC) is still caught — proving the
    /// `PermissionDenied` / `UnauthorizedAccessError` markers carry it.
    #[test]
    fn does_not_rely_on_wrapped_unauthorized_access_exception() {
        let wrapped = "foo [Set-Content], Unauthorized \r\n   AccessException bar PermissionDenied baz";
        assert!(is_sandbox_denial(wrapped));
        // A string that ONLY contains the wrapped exception with no reliable
        // marker is (acceptably) NOT detected — documents the limitation.
        let only_wrapped = "Unauthorized \r\n   AccessException";
        assert!(!is_sandbox_denial(only_wrapped));
    }

    /// Negative (KEY false-positive guard): an ordinary grep-no-match style
    /// failure (exits non-zero, but no permission marker) is NOT a denial.
    #[test]
    fn ordinary_grep_no_match_is_not_a_denial() {
        // grep with no match prints nothing.
        assert!(!is_sandbox_denial(""));
        // ripgrep-style "no matches" summary, a missing file, a failing test.
        assert!(!is_sandbox_denial("No files were searched"));
        assert!(!is_sandbox_denial("error: could not find file 'x.rs'"));
        assert!(!is_sandbox_denial("test result: FAILED. 1 failed"));
        assert!(!is_sandbox_denial("fatal: not a git repository"));
    }

    /// English-locale Windows denial is detected.
    #[test]
    fn detects_english_access_is_denied() {
        assert!(is_sandbox_denial("Set-Content : Access is denied."));
    }

    /// Unix EACCES / EPERM strerror text is detected.
    #[test]
    fn detects_unix_permission_strerror() {
        assert!(is_sandbox_denial("cp: cannot create regular file 'x': Permission denied"));
        assert!(is_sandbox_denial("kill: (1234): Operation not permitted"));
    }

    /// Matching is case-insensitive.
    #[test]
    fn marker_match_is_case_insensitive() {
        assert!(is_sandbox_denial("ACCESS IS DENIED"));
        assert!(is_sandbox_denial("permissiondenied"));
        assert!(is_sandbox_denial("PERMISSION DENIED"));
    }

    // === agent-run-integrity (vertical A): classify_stderr_lines ===

    use std::io::{BufReader, Cursor};

    /// A buffer carrying exactly one curated denial marker line (plus benign
    /// noise lines) yields `count == 1`, when forwarding is on.
    #[test]
    fn classify_counts_single_denial_line_when_forwarding() {
        let input = "init: loading model\n\
                     Set-Content : Access is denied.\n\
                     done\n";
        let reader = BufReader::new(Cursor::new(input));
        let mut sink: Vec<u8> = Vec::new();
        let count = classify_stderr_lines(reader, &mut sink, true);
        assert_eq!(count, 1, "exactly one line carries a denial marker");
        // Forwarding on → every line (denial + benign) is written out.
        let forwarded = String::from_utf8(sink).unwrap();
        assert_eq!(forwarded.lines().count(), 3);
        assert!(forwarded.contains("Access is denied"));
    }

    /// Classification is INDEPENDENT of the forward toggle: the same buffer
    /// with `forward == false` still counts the denial as 1, and discards
    /// (does not write) any lines.
    #[test]
    fn classify_counts_denial_independent_of_forward_flag() {
        let input = "init: loading model\n\
                     Set-Content : Access is denied.\n\
                     done\n";
        let reader = BufReader::new(Cursor::new(input));
        let mut sink: Vec<u8> = Vec::new();
        let count = classify_stderr_lines(reader, &mut sink, false);
        assert_eq!(
            count, 1,
            "denial counted regardless of CODEBUS_FORWARD_AGENT_STDERR toggle"
        );
        assert!(
            sink.is_empty(),
            "forward == false discards lines (nothing written): {sink:?}"
        );
    }

    /// Multiple denial lines are each counted; benign lines (grep-no-match,
    /// missing file) are not — proving per-line application of the same
    /// curated marker set.
    #[test]
    fn classify_counts_every_denial_line_and_excludes_benign() {
        let input = "cp: x: Permission denied\n\
                     no matches found\n\
                     kill: Operation not permitted\n\
                     error: could not find file 'y'\n";
        let reader = BufReader::new(Cursor::new(input));
        let mut sink = std::io::sink();
        let count = classify_stderr_lines(reader, &mut sink, false);
        assert_eq!(count, 2, "two denial lines, two benign lines");
    }
}
