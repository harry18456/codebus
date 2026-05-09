//! Wiki linter — checks an Obsidian-compatible vault for structural issues.
//!
//! v3 port: rule-based architecture from `legacy/v2-rust/codebus-core/src/wiki/lint/`.
//! Per v3-lint design: lint takes the vault root path (the `.codebus/` directory)
//! and runs rules against `<vault_root>/wiki/`. Vault root resolution from any
//! cwd is the caller's responsibility (see [`locate`]).

pub mod locate;

use crate::wiki::types::{LintIssue, LintResult, LintSeverity};
use std::path::Path;

pub use locate::{LocateError, locate_vault_root};

/// Validate a vault's `wiki/` subtree. Pure read — never writes.
///
/// `vault_root` is the `.codebus/` path (e.g. `/repo/.codebus/`).
///
/// Returns coverage counts plus `Vec<LintIssue>`. Callers (CLI lint subcommand,
/// fix loop) decide how to surface based on `error_count` vs `warn_count`.
pub fn lint_wiki(vault_root: impl AsRef<Path>) -> LintResult {
    let wiki_root = vault_root.as_ref().join("wiki");

    if !wiki_root.exists() {
        return summarize(0, 0, Vec::new());
    }

    // TODO(v3-lint #1.3-1.9): wire individual rules here.
    // For now, return empty result so the public API compiles and integrators
    // can scaffold against the type. Each rule lands in a subsequent task.
    let issues: Vec<LintIssue> = Vec::new();

    summarize(0, 0, issues)
}

fn summarize(pages_scanned: usize, nav_files_scanned: usize, issues: Vec<LintIssue>) -> LintResult {
    let error_count = issues
        .iter()
        .filter(|i| i.severity == LintSeverity::Error)
        .count();
    let warn_count = issues
        .iter()
        .filter(|i| i.severity == LintSeverity::Warn)
        .count();
    LintResult {
        pages_scanned,
        nav_files_scanned,
        issues,
        error_count,
        warn_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lint_returns_empty_when_wiki_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let r = lint_wiki(tmp.path());
        assert_eq!(r.error_count, 0);
        assert_eq!(r.warn_count, 0);
        assert_eq!(r.pages_scanned, 0);
        assert_eq!(r.nav_files_scanned, 0);
    }
}
