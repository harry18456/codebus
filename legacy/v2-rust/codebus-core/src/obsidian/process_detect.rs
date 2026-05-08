//! Detect whether an Obsidian process is currently running on the OS.
//!
//! # Why this exists
//!
//! When codebus writes `obsidian.json` to register a vault, Obsidian's own
//! vault-list flush may race-overwrite the file if Obsidian is currently
//! running. The registry module calls [`is_obsidian_running`] before writing
//! and skips the write when the answer is `true`, deferring registration to
//! the next codebus run when Obsidian has quit.
//!
//! # Fail-safe semantics
//!
//! Detection is deliberately fail-safe in the **skip-write** direction:
//!
//! - **False positive** (we say "running" but Obsidian isn't) → codebus
//!   declines to write the registry. The user's vault simply isn't auto-
//!   registered this run; no data loss, no security harm. A `Scoundrel`
//!   audit lens — an attacker renaming a malicious binary to `obsidian.exe`
//!   to "trick" codebus — only achieves making us *not* write. That is the
//!   safer side; we accept this trade.
//! - **False negative** (we say "not running" but Obsidian is) → codebus
//!   writes the registry while Obsidian is live. Obsidian may overwrite our
//!   entry on its next flush. No security harm, only a re-run cost.
//!
//! Returning `bool` (not `Result<bool>`) is intentional: even if `sysinfo`
//! cannot enumerate processes for any reason, we report `false` (not
//! running → proceed with the write attempt). This matches `sysinfo`'s own
//! best-effort contract and keeps the registry write path linear — no
//! `Result` plumbing leaks into the registry caller.
//!
//! # Matching rule
//!
//! Per OS we compare each enumerated process name with `eq_ignore_ascii_case`
//! against an exact target. **No substring matching** — a process named
//! `obsidian-helper` or `myobsidian-tool` is NOT a hit (Lazy-Developer audit).

use sysinfo::{ProcessesToUpdate, System};

/// Returns `true` if at least one Obsidian process is detected on the system.
///
/// Detection rule per OS:
/// - Windows: process name matches `obsidian.exe` (case-insensitive)
/// - macOS:   process name matches `Obsidian` (case-insensitive)
/// - Linux:   process name matches `obsidian` (case-insensitive)
///
/// **Always returns a `bool`, never panics, never errors.** If the underlying
/// `sysinfo` call fails to populate any processes, the iterator is empty and
/// this returns `false` (fail-safe — see module docs).
pub fn is_obsidian_running() -> bool {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let names: Vec<String> = sys
        .processes()
        .values()
        .map(|p| p.name().to_string_lossy().into_owned())
        .collect();
    any_name_matches_obsidian(names.iter().map(|s| s.as_str()))
}

/// Pure: does any name in `names` match the OS-specific Obsidian binary?
///
/// Comparison is `eq_ignore_ascii_case` (exact match, case-insensitive). No
/// substring matching — see module-level Lazy-Developer note.
pub(crate) fn any_name_matches_obsidian<'a>(names: impl IntoIterator<Item = &'a str>) -> bool {
    let target = obsidian_binary_name();
    names.into_iter().any(|n| n.eq_ignore_ascii_case(target))
}

/// The exact process-name string we look for on the current target OS.
///
/// All three OS branches live in one function on purpose: keeping this as
/// per-OS modules would be overhead for three string constants.
pub(crate) fn obsidian_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "obsidian.exe"
    } else if cfg!(target_os = "macos") {
        "Obsidian"
    } else {
        // Linux + any other Unix-like fallback.
        "obsidian"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matcher_returns_true_for_exact_obsidian_match() {
        // Pass the OS-specific target as-is → must match.
        let target = obsidian_binary_name();
        assert!(
            any_name_matches_obsidian([target]),
            "exact target name must match"
        );
    }

    #[test]
    fn matcher_returns_true_for_case_variations() {
        // Case-insensitivity check. Build mixed-case variants of whatever
        // target this OS uses so the test stays correct cross-platform.
        let target = obsidian_binary_name();
        let upper = target.to_ascii_uppercase();
        let lower = target.to_ascii_lowercase();

        assert!(
            any_name_matches_obsidian([upper.as_str()]),
            "uppercase variant must match"
        );
        assert!(
            any_name_matches_obsidian([lower.as_str()]),
            "lowercase variant must match"
        );
    }

    #[test]
    fn matcher_returns_false_for_unrelated_processes() {
        // A representative selection of common process names across platforms.
        // None of these should match the Obsidian target on any OS.
        let unrelated = [
            "explorer.exe",
            "code.exe",
            "rust-analyzer",
            "bash",
            "Finder",
            "systemd",
            "kernel_task",
            "powershell.exe",
        ];
        assert!(
            !any_name_matches_obsidian(unrelated),
            "unrelated process names must not match"
        );
    }

    #[test]
    fn matcher_returns_false_for_empty_list() {
        let empty: [&str; 0] = [];
        assert!(
            !any_name_matches_obsidian(empty),
            "empty input must yield false"
        );
    }

    #[test]
    fn matcher_does_not_match_obsidian_substring_in_unrelated_name() {
        // Lazy-Developer audit: substring containment must NOT trigger a hit.
        // Both names contain "obsidian" as a substring but are not the binary.
        let near_misses = [
            "obsidian-helper",
            "myobsidian",
            "obsidian-tool",
            "obsidiana",
        ];
        assert!(
            !any_name_matches_obsidian(near_misses),
            "substring matches must not be treated as Obsidian"
        );
    }

    #[test]
    fn is_obsidian_running_returns_bool_without_panic() {
        // Smoke test: just call it. We don't know whether Obsidian is running
        // on the runner, so we don't assert the value — only that the call
        // returns a `bool` and doesn't panic.
        let _running: bool = is_obsidian_running();
    }
}
