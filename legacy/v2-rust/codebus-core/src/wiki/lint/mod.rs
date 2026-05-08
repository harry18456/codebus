//! Wiki linter — checks an Obsidian-compatible vault for structural issues.
//!
//! The linter is a thin orchestrator over a `Vec<Box<dyn LintRule>>`: each
//! rule is implemented under [`rules`] and registered via
//! [`factory::build_default_rules`]. Rules are pure read; the orchestrator
//! never writes.
//!
//! Adding a rule = one new file under `rules/<rule>.rs` + one entry in
//! `factory.rs`. No edits anywhere else.

pub mod factory;
pub mod rule;
pub mod rules;

pub use factory::build_default_rules;
pub use rule::{LintRule, LoadedPage, NavFile, RECOGNIZED_ROOT_DIRS, SPECIAL_FILES, VaultContext};

use crate::wiki::types::{LintIssue, LintResult, LintSeverity};
use std::path::Path;

/// Validate a vault's `wiki/` subtree. Pure read — never writes. Returns
/// coverage counts plus `Vec<LintIssue>`; callers (auto-lint after ingest,
/// `--check` standalone) decide how to surface based on `error_count` vs
/// `warn_count`.
///
/// `vault_root` is the `.codebus/` path (e.g. `/repo/.codebus/`).
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

    // Stable-sort issues by path rank so the renderer's first-appearance
    // grouping reproduces the legacy single-file emission order:
    //   type-folder pages (rank 1) → root .md files (rank 2) → nav files
    //   (rank 3). Folder warnings (rank 0) come first.
    // `Vec::sort_by` is stable, so within-rank emission order from the rule
    // sequence in `build_default_rules` is preserved.
    issues.sort_by_key(|i| path_rank(&i.path));

    summarize(pages_scanned, nav_files_scanned, issues)
}

/// Rank used to interleave issues from multiple rules so the report groups
/// pages → root files → nav files (matching the legacy lint emission
/// order). Folder warnings (no `.md` suffix at the leaf) sort to the top.
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
        let dir = std::env::temp_dir().join(format!(
            "codebus-lint-{name}-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
        fs::create_dir_all(dir.join("wiki")).unwrap();
        for f in ["concepts", "entities", "modules", "processes", "synthesis"] {
            fs::create_dir_all(dir.join("wiki").join(f)).unwrap();
        }
        // Default nav files exist so missing-nav warnings don't pollute
        // unrelated test cases.
        fs::write(dir.join("wiki/index.md"), "# index\n").unwrap();
        fs::write(dir.join("wiki/log.md"), "# log\n").unwrap();
        dir
    }

    fn rand_suffix() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        format!("{nanos}")
    }

    fn write_page(root: &Path, rel_path: &str, frontmatter: &str, body: &str) {
        let full = root.join("wiki").join(rel_path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        let content = format!("---\n{frontmatter}---\n{body}");
        fs::write(full, content).unwrap();
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

    fn cleanup(p: &Path) {
        let _ = fs::remove_dir_all(p);
    }

    fn count(result: &LintResult, sev: LintSeverity, msg_substring: &str) -> usize {
        result
            .issues
            .iter()
            .filter(|i| i.severity == sev && i.message.contains(msg_substring))
            .count()
    }

    fn issues_for_path<'a>(result: &'a LintResult, path: &str) -> Vec<&'a LintIssue> {
        result.issues.iter().filter(|i| i.path == path).collect()
    }

    // === existing rules ===

    #[test]
    fn page_in_wiki_root_is_flagged() {
        let v = tmp_vault("rootpage");
        fs::write(v.join("wiki/test.md"), "---\ntitle: x\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\n").unwrap();
        let r = lint_wiki(&v);
        let warns = issues_for_path(&r, "test.md");
        assert!(
            warns.iter().any(|i| i.severity == LintSeverity::Warn
                && i.message.contains("page lives in wiki/ root"))
        );
        cleanup(&v);
    }

    #[test]
    fn folder_type_mismatch_is_not_flagged() {
        let v = tmp_vault("typefolder");
        write_page(&v, "concepts/foo.md", &fm("foo", "module", &[]), "# foo");
        let r = lint_wiki(&v);
        let foo_issues = issues_for_path(&r, "concepts/foo.md");
        assert!(
            foo_issues
                .iter()
                .all(|i| !i.message.contains("type") || !i.message.contains("folder"))
        );
        cleanup(&v);
    }

    #[test]
    fn duplicate_slug_across_folders_flagged_on_every_occurrence() {
        let v = tmp_vault("dupslug");
        write_page(&v, "concepts/cart.md", &fm("Cart-c", "concept", &[]), "# c");
        write_page(&v, "entities/cart.md", &fm("Cart-e", "entity", &[]), "# e");
        let r = lint_wiki(&v);
        let dup_count = count(&r, LintSeverity::Warn, "duplicate slug 'cart'");
        assert_eq!(dup_count, 2);
        cleanup(&v);
    }

    #[test]
    fn missing_index_md_is_flagged() {
        let v = tmp_vault("missindex");
        fs::remove_file(v.join("wiki/index.md")).unwrap();
        let r = lint_wiki(&v);
        let warns = issues_for_path(&r, "index.md");
        assert!(
            warns
                .iter()
                .any(|i| i.severity == LintSeverity::Warn && i.message.contains("missing"))
        );
        cleanup(&v);
    }

    #[test]
    fn missing_log_md_is_flagged() {
        let v = tmp_vault("misslog");
        fs::remove_file(v.join("wiki/log.md")).unwrap();
        let r = lint_wiki(&v);
        let warns = issues_for_path(&r, "log.md");
        assert!(
            warns
                .iter()
                .any(|i| i.severity == LintSeverity::Warn && i.message.contains("missing"))
        );
        cleanup(&v);
    }

    #[test]
    fn missing_overview_md_is_not_flagged() {
        let v = tmp_vault("missoverview");
        let r = lint_wiki(&v);
        assert!(
            r.issues
                .iter()
                .all(|i| !(i.path == "overview.md" && i.message.contains("missing")))
        );
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
                .any(|i| i.severity == LintSeverity::Warn && i.message.contains("[[ghost]]"))
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
        assert!(
            count(
                &r,
                LintSeverity::Error,
                "related[] entry not in [[wikilink]] format"
            ) > 0
        );
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
        assert!(count(&r, LintSeverity::Error, "broken wikilink in related") > 0);
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
        assert!(count(&r, LintSeverity::Error, "frontmatter parse failed") > 0);
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
        assert!(
            issues.is_empty(),
            "expected zero issues for inline-code wikilink, got {issues:?}"
        );
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
    fn table_cell_wikilink_with_escaped_alias_resolves_to_bare_slug() {
        let v = tmp_vault("escape");
        write_page(
            &v,
            "concepts/resolver-resolve.md",
            &fm("resolve", "concept", &[]),
            "# resolve",
        );
        write_page(
            &v,
            "concepts/foo.md",
            &fm("foo", "concept", &[]),
            "| col | [[resolver-resolve\\|Resolver]] |",
        );
        let r = lint_wiki(&v);
        let foo_issues = issues_for_path(&r, "concepts/foo.md");
        assert!(foo_issues.is_empty());
        cleanup(&v);
    }

    #[test]
    fn table_cell_wikilink_with_escape_still_flags_broken_slug() {
        let v = tmp_vault("escapebroken");
        write_page(
            &v,
            "concepts/foo.md",
            &fm("foo", "concept", &[]),
            "| [[ghost\\|alias]] |",
        );
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "concepts/foo.md");
        assert!(issues.iter().any(|i| i.message.contains("[[ghost]]")));
        cleanup(&v);
    }

    #[test]
    fn nav_file_body_wikilink_is_flagged() {
        let v = tmp_vault("navlink");
        fs::write(v.join("wiki/index.md"), "see [[ghost]]").unwrap();
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "index.md");
        assert!(
            issues
                .iter()
                .any(|i| i.severity == LintSeverity::Warn && i.message.contains("[[ghost]]"))
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

    // === wiki-hygiene-signals: page-size warn ===

    #[test]
    fn oversized_index_md_is_flagged_with_size_and_threshold_in_message() {
        let v = tmp_vault("idxsize");
        let big = "a".repeat(1500);
        fs::write(v.join("wiki/index.md"), &big).unwrap();
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "index.md");
        let oversized: Vec<_> = issues
            .iter()
            .filter(|i| i.message.contains("oversize"))
            .collect();
        assert_eq!(oversized.len(), 1);
        assert!(oversized[0].message.contains("size 1500 bytes"));
        assert!(oversized[0].message.contains("threshold 1024 bytes"));
        cleanup(&v);
    }

    #[test]
    fn oversized_synthesis_page_is_flagged() {
        let v = tmp_vault("synsize");
        let body = "x".repeat(5800);
        write_page(
            &v,
            "synthesis/cart-flow.md",
            &fm("flow", "synthesis", &[]),
            &body,
        );
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "synthesis/cart-flow.md");
        let oversized: Vec<_> = issues
            .iter()
            .filter(|i| i.message.contains("oversize"))
            .collect();
        assert_eq!(oversized.len(), 1);
        assert!(oversized[0].message.contains("threshold 5120 bytes"));
        cleanup(&v);
    }

    #[test]
    fn oversized_concepts_page_is_flagged() {
        let v = tmp_vault("concsize");
        let body = "y".repeat(8800);
        write_page(&v, "concepts/foo.md", &fm("foo", "concept", &[]), &body);
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "concepts/foo.md");
        let oversized: Vec<_> = issues
            .iter()
            .filter(|i| i.message.contains("oversize"))
            .collect();
        assert_eq!(oversized.len(), 1);
        assert!(oversized[0].message.contains("threshold 8192 bytes"));
        cleanup(&v);
    }

    #[test]
    fn oversized_log_md_is_not_flagged() {
        let v = tmp_vault("logbig");
        let big = "a".repeat(50_000);
        fs::write(v.join("wiki/log.md"), &big).unwrap();
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "log.md");
        assert!(issues.iter().all(|i| !i.message.contains("oversize")));
        cleanup(&v);
    }

    #[test]
    fn page_exactly_at_threshold_is_not_flagged() {
        let v = tmp_vault("exact");
        let frontmatter = fm("x", "concept", &[]);
        let prefix = format!("---\n{frontmatter}---\n");
        let pad = 8192usize.saturating_sub(prefix.len());
        let body = "a".repeat(pad);
        let content = format!("{prefix}{body}");
        assert_eq!(content.len(), 8192);
        fs::write(v.join("wiki/concepts/foo.md"), &content).unwrap();
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "concepts/foo.md");
        assert!(issues.iter().all(|i| !i.message.contains("oversize")));
        cleanup(&v);
    }

    #[test]
    fn page_below_threshold_is_not_flagged() {
        let v = tmp_vault("small");
        write_page(
            &v,
            "concepts/foo.md",
            &fm("foo", "concept", &[]),
            &"z".repeat(1000),
        );
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "concepts/foo.md");
        assert!(issues.iter().all(|i| !i.message.contains("oversize")));
        cleanup(&v);
    }

    // === wiki-hygiene-signals: unexpected-file warn ===

    #[test]
    fn non_md_file_in_type_folder_is_flagged() {
        let v = tmp_vault("nonmd");
        fs::write(v.join("wiki/concepts/foo.txt"), "not markdown").unwrap();
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "concepts/foo.txt");
        assert!(issues.iter().any(|i| i.severity == LintSeverity::Warn
            && i.message.contains("non-.md file in type folder")));
        cleanup(&v);
    }

    #[test]
    fn nested_sub_folder_in_type_folder_is_flagged() {
        let v = tmp_vault("nested");
        fs::create_dir_all(v.join("wiki/modules/legacy")).unwrap();
        fs::write(v.join("wiki/modules/legacy/old.md"), "x").unwrap();
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "modules/legacy");
        assert!(
            issues
                .iter()
                .any(|i| i.message.contains("nested sub-folder in type folder"))
        );
        cleanup(&v);
    }

    #[test]
    fn unrecognized_folder_under_wiki_is_flagged() {
        let v = tmp_vault("scratch");
        fs::create_dir_all(v.join("wiki/scratch")).unwrap();
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "scratch");
        assert!(
            issues
                .iter()
                .any(|i| i.message.contains("unrecognized folder under wiki/"))
        );
        cleanup(&v);
    }

    #[test]
    fn hidden_entries_are_skipped_silently() {
        let v = tmp_vault("hidden");
        fs::create_dir_all(v.join("wiki/.obsidian")).unwrap();
        fs::write(v.join("wiki/.obsidian/app.json"), "{}").unwrap();
        fs::write(v.join("wiki/.gitkeep"), "").unwrap();
        let r = lint_wiki(&v);
        assert!(r.issues.iter().all(|i| !i.path.starts_with('.')));
        cleanup(&v);
    }

    // === fixture-level integration ===

    #[test]
    fn lint_uv_fixture_produces_known_warning_count() {
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/uv-vault-snapshot/uv-wiki-snapshot");
        let stage = std::env::temp_dir().join(format!(
            "codebus-lint-uvfixture-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        let _ = fs::remove_dir_all(&stage);
        fs::create_dir_all(stage.join("wiki")).unwrap();
        copy_dir_all(&fixture_root, &stage.join("wiki")).unwrap();

        let r = lint_wiki(&stage);
        assert_eq!(r.error_count, 0, "unexpected errors: {:?}", r.issues);
        assert!(
            r.warn_count >= 5,
            "expected ≥5 warnings, got {}: {:#?}",
            r.warn_count,
            r.issues
        );
        let broken_body = count(&r, LintSeverity::Warn, "broken wikilink in body");
        assert!(
            broken_body >= 4,
            "expected ≥4 broken body wikilinks, got {broken_body}"
        );
        let root_warn = count(&r, LintSeverity::Warn, "page lives in wiki/ root");
        assert_eq!(root_warn, 1);

        let _ = fs::remove_dir_all(&stage);
    }

    fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            let dst_path = dst.join(entry.file_name());
            if ft.is_dir() {
                copy_dir_all(&entry.path(), &dst_path)?;
            } else {
                fs::copy(entry.path(), dst_path)?;
            }
        }
        Ok(())
    }
}
