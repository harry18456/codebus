//! Wiki linter — checks an Obsidian-compatible vault for structural issues.
//!
//! Architecture: rule-based. Each rule lives under [`rules`] and is registered
//! in [`factory::build_default_rules`]. Rules are pure read; the orchestrator
//! never writes (Lint Read-Only Invariant).
//!
//! Adding a rule = one new file under `rules/<rule>.rs` + one entry in
//! `factory.rs` + (if it emits a new finding kind) a stable `rule_id` string.

pub mod factory;
pub mod locate;
pub mod output;
pub mod rule;
pub mod rules;

pub use factory::build_default_rules;
pub use locate::{LocateError, locate_vault_root};
pub use output::{format_json, format_text};
// `is_wiki_relative_path` is defined below and used by both formatters; it is
// part of the public lint surface so the `render` text formatter can share it.
pub use rule::{LintRule, LoadedPage, NavFile, RECOGNIZED_ROOT_DIRS, SPECIAL_FILES, VaultContext};

use crate::wiki::types::{LintIssue, LintResult, LintSeverity};
use std::path::Path;

/// Vault-internal config files that live OUTSIDE the `wiki/` subtree and whose
/// lint issue paths must therefore NOT be prefixed with `wiki/` (text format)
/// nor joined under `wiki/` (JSON format). Currently just the PreToolUse gate
/// config flagged by the `vault-gate-integrity` rule.
const NON_WIKI_ISSUE_PREFIXES: &[&str] = &[".claude/"];

/// True when `path` is a wiki-subtree issue path (the default — rendered with a
/// `wiki/` prefix). False for vault-internal issue paths like
/// `.claude/settings.json`, which are rendered verbatim / joined at vault root.
pub fn is_wiki_relative_path(path: &str) -> bool {
    !NON_WIKI_ISSUE_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

/// Validate a vault's `wiki/` subtree. Pure read — never writes.
///
/// `vault_root` is the `.codebus/` path (e.g. `/repo/.codebus/`). Use
/// [`locate_vault_root`] to resolve the vault from a cwd before calling.
pub fn lint_wiki(vault_root: impl AsRef<Path>) -> LintResult {
    let wiki_root = vault_root.as_ref().join("wiki");

    if !wiki_root.exists() {
        return summarize(0, 0, Vec::new());
    }

    let ctx = VaultContext::build(&wiki_root);
    let pages_scanned = ctx.pages_scanned();
    let nav_files_scanned = ctx.nav_files_scanned();

    let mut issues: Vec<LintIssue> = Vec::new();
    for rule in build_default_rules() {
        issues.extend(rule.check(&ctx));
    }

    issues.sort_by_key(|i| path_rank(&i.path));
    summarize(pages_scanned, nav_files_scanned, issues)
}

/// Rank to interleave issues from multiple rules so the report groups
/// pages → root files → nav files. Folder warnings (no `.md` leaf) sort
/// to the top.
fn path_rank(path: &str) -> u8 {
    if path.contains('/') {
        return 1;
    }
    if SPECIAL_FILES.contains(&path) {
        return 3;
    }
    if path.ends_with(".md") {
        return 2;
    }
    0
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
    use std::fs;
    use std::path::{Path, PathBuf};

    fn tmp_vault(name: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let dir = std::env::temp_dir().join(format!(
            "codebus-lint-{name}-{}-{}",
            std::process::id(),
            nanos
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("wiki")).unwrap();
        for f in ["concepts", "entities", "modules", "processes", "synthesis"] {
            fs::create_dir_all(dir.join("wiki").join(f)).unwrap();
        }
        fs::write(dir.join("wiki/index.md"), "# index\n").unwrap();
        fs::write(dir.join("wiki/log.md"), "# log\n").unwrap();
        dir
    }

    fn fm(title: &str, ty: &str, related: &[&str]) -> String {
        let related_yaml = if related.is_empty() {
            "[]".into()
        } else {
            let items: Vec<String> = related.iter().map(|r| format!("'{r}'")).collect();
            format!("\n  - {}", items.join("\n  - "))
        };
        format!(
            "title: {title}\ntype: {ty}\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: {related_yaml}\nstale: false\n"
        )
    }

    fn write_page(root: &Path, rel_path: &str, frontmatter: &str, body: &str) {
        let full = root.join("wiki").join(rel_path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        let content = format!("---\n{frontmatter}---\n{body}");
        fs::write(full, content).unwrap();
    }

    fn cleanup(p: &Path) {
        let _ = fs::remove_dir_all(p);
    }

    fn issues_for_path<'a>(result: &'a LintResult, path: &str) -> Vec<&'a LintIssue> {
        result.issues.iter().filter(|i| i.path == path).collect()
    }

    fn count_with(result: &LintResult, sev: LintSeverity, msg_substring: &str) -> usize {
        result
            .issues
            .iter()
            .filter(|i| i.severity == sev && i.message.contains(msg_substring))
            .count()
    }

    #[test]
    fn lint_returns_empty_when_wiki_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let r = lint_wiki(tmp.path());
        assert_eq!(r.error_count, 0);
        assert_eq!(r.warn_count, 0);
        assert_eq!(r.pages_scanned, 0);
        assert_eq!(r.nav_files_scanned, 0);
    }

    #[test]
    fn page_in_wiki_root_is_flagged_with_misplaced_root_page_rule() {
        let v = tmp_vault("rootpage");
        fs::write(
            v.join("wiki/test.md"),
            "---\ntitle: x\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\n",
        )
        .unwrap();
        let r = lint_wiki(&v);
        let warns = issues_for_path(&r, "test.md");
        assert!(warns.iter().any(|i| i.severity == LintSeverity::Warn
            && i.rule_id == "misplaced-root-page"
            && i.message.contains("page lives in wiki/ root")));
        cleanup(&v);
    }

    #[test]
    fn duplicate_slug_across_folders_flagged_on_every_occurrence() {
        let v = tmp_vault("dupslug");
        write_page(&v, "concepts/cart.md", &fm("Cart-c", "concept", &[]), "# c");
        write_page(&v, "entities/cart.md", &fm("Cart-e", "entity", &[]), "# e");
        let r = lint_wiki(&v);
        let dup_count = count_with(&r, LintSeverity::Warn, "duplicate slug 'cart'");
        assert_eq!(dup_count, 2);
        assert!(
            r.issues
                .iter()
                .filter(|i| i.message.contains("duplicate slug 'cart'"))
                .all(|i| i.rule_id == "duplicate-slug")
        );
        cleanup(&v);
    }

    #[test]
    fn missing_index_md_flagged_with_nav_missing_rule_id() {
        let v = tmp_vault("missindex");
        fs::remove_file(v.join("wiki/index.md")).unwrap();
        let r = lint_wiki(&v);
        let warns = issues_for_path(&r, "index.md");
        assert!(warns.iter().any(|i| i.rule_id == "nav-missing"));
        cleanup(&v);
    }

    #[test]
    fn body_wikilink_to_nonexistent_slug_flagged_at_warn() {
        let v = tmp_vault("bodyghost");
        write_page(
            &v,
            "concepts/foo.md",
            &fm("foo", "concept", &[]),
            "see [[ghost]]",
        );
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "concepts/foo.md");
        assert!(
            issues
                .iter()
                .any(|i| i.rule_id == "broken-wikilink-body" && i.message.contains("[[ghost]]"))
        );
        cleanup(&v);
    }

    #[test]
    fn related_entry_not_wikilink_is_error() {
        let v = tmp_vault("relbad");
        let mut frontmatter = fm("foo", "concept", &[]);
        frontmatter = frontmatter.replace("related: []", "related:\n  - 'broken-no-brackets'");
        write_page(&v, "concepts/foo.md", &frontmatter, "# foo");
        let r = lint_wiki(&v);
        let errs: Vec<_> = r
            .issues
            .iter()
            .filter(|i| i.rule_id == "related-format")
            .collect();
        assert!(!errs.is_empty());
        cleanup(&v);
    }

    #[test]
    fn related_entry_to_missing_slug_is_error() {
        let v = tmp_vault("relmissing");
        write_page(
            &v,
            "concepts/foo.md",
            &fm("foo", "concept", &["[[ghost]]"]),
            "# foo",
        );
        let r = lint_wiki(&v);
        assert!(
            r.issues
                .iter()
                .any(|i| i.rule_id == "broken-wikilink-related" && i.message.contains("[[ghost]]"))
        );
        cleanup(&v);
    }

    #[test]
    fn frontmatter_parse_failure_is_error() {
        let v = tmp_vault("fmparse");
        fs::write(
            v.join("wiki/concepts/broken.md"),
            "---\n: : not yaml\n---\n",
        )
        .unwrap();
        let r = lint_wiki(&v);
        assert!(r.issues.iter().any(|i| i.rule_id == "frontmatter-parse"));
        cleanup(&v);
    }

    #[test]
    fn body_wikilink_inside_inline_code_is_skipped() {
        let v = tmp_vault("inline");
        write_page(
            &v,
            "concepts/foo.md",
            &fm("foo", "concept", &[]),
            "use `[[wikilink]]` here",
        );
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "concepts/foo.md");
        assert!(issues.is_empty(), "expected zero issues, got {issues:?}");
        cleanup(&v);
    }

    #[test]
    fn body_wikilink_inside_fenced_block_is_skipped() {
        let v = tmp_vault("fenced");
        write_page(
            &v,
            "concepts/foo.md",
            &fm("foo", "concept", &[]),
            "\n```\n[[ghost]]\n```\n",
        );
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "concepts/foo.md");
        assert!(issues.is_empty());
        cleanup(&v);
    }

    #[test]
    fn nav_file_body_wikilink_is_flagged_with_broken_wikilink_nav_rule_id() {
        let v = tmp_vault("navlink");
        fs::write(v.join("wiki/index.md"), "see [[ghost]]").unwrap();
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "index.md");
        assert!(
            issues
                .iter()
                .any(|i| i.rule_id == "broken-wikilink-nav" && i.message.contains("[[ghost]]"))
        );
        cleanup(&v);
    }

    #[test]
    fn pages_scanned_counts_only_successfully_parsed() {
        let v = tmp_vault("counts");
        write_page(&v, "concepts/ok.md", &fm("ok", "concept", &[]), "# ok");
        fs::write(v.join("wiki/concepts/broken.md"), "---\n::not yaml\n---\n").unwrap();
        let r = lint_wiki(&v);
        assert_eq!(r.pages_scanned, 1);
        assert!(r.error_count >= 1);
        cleanup(&v);
    }

    #[test]
    fn nav_files_scanned_counts_only_existing_specials() {
        let v = tmp_vault("navcount");
        fs::remove_file(v.join("wiki/log.md")).unwrap();
        let r = lint_wiki(&v);
        assert_eq!(r.nav_files_scanned, 1);
        cleanup(&v);
    }

    /// Lint Read-Only Invariant — verifies vault contents are byte-identical
    /// before and after lint runs against a vault with multiple issues.
    #[test]
    fn lint_read_only_invariant_dirty_vault() {
        let v = tmp_vault("readonly");
        write_page(
            &v,
            "concepts/foo.md",
            &fm("foo", "concept", &["[[ghost]]"]),
            "see [[also-ghost]]",
        );
        write_page(&v, "concepts/cart.md", &fm("Cart-c", "concept", &[]), "# c");
        write_page(&v, "entities/cart.md", &fm("Cart-e", "entity", &[]), "# e");
        fs::write(v.join("wiki/orphan.md"), "---\ntitle: orphan\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\n").unwrap();

        let snap_before = snapshot_dir(&v.join("wiki"));
        let _ = lint_wiki(&v);
        let snap_after = snapshot_dir(&v.join("wiki"));
        assert_eq!(
            snap_before, snap_after,
            "lint must not modify any vault file"
        );
        cleanup(&v);
    }

    fn snapshot_dir(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut snap = Vec::new();
        fn recurse(dir: &Path, snap: &mut Vec<(PathBuf, Vec<u8>)>) {
            let Ok(rd) = fs::read_dir(dir) else { return };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    recurse(&p, snap);
                } else {
                    let data = fs::read(&p).unwrap_or_default();
                    snap.push((p, data));
                }
            }
        }
        recurse(dir, &mut snap);
        snap.sort_by(|a, b| a.0.cmp(&b.0));
        snap
    }
}
