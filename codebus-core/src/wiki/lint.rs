use crate::wiki::frontmatter::parse_page;
use crate::wiki::types::{LintIssue, LintResult, LintSeverity, PageType};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const SPECIAL_FILES: &[&str] = &["index.md", "log.md"];
const RECOGNIZED_ROOT_DIRS: &[&str] = &["concepts", "entities", "modules", "processes", "synthesis", "goals"];

// Page-size thresholds per file type (bytes, strict greater-than). log.md is
// unlimited — it grows by design.
const INDEX_MD_THRESHOLD: usize = 1024;
const SYNTHESIS_THRESHOLD: usize = 5120;
const TYPE_FOLDER_THRESHOLD: usize = 8192;

// Body wikilink regex — matches [[slug]], [[slug|display]], [[slug#heading]],
// [[slug#heading|display]]; captures slug only. The slug class excludes
// backslash so markdown table escapes `[[slug\|alias]]` parse with slug=`slug`
// (not `slug\`); the alias separator accepts either `|` or `\|`.
static BODY_WIKILINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]|#\s\\]+)(?:#[^\]|]+)?(?:\\?\|[^\]]+)?\]\]").unwrap());

static RELATED_STRIP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\[\[([^\]]+)\]\]\s*$").unwrap());

// Fenced code block (greedy across lines).
static FENCED_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```.*?```").unwrap());

// Inline code span — single line, no embedded backticks.
static INLINE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`\n]+`").unwrap());

#[derive(Debug, Clone)]
struct PageEntry {
    folder: &'static str,
    filename: String,
    slug: String,
    rel_path: String,
    full_path: PathBuf,
}

fn folder_name(t: PageType) -> &'static str {
    t.folder()
}

/// Strip markdown code regions (fenced first, then inline) so [[wikilink]]
/// occurrences inside them are not scanned. Obsidian renders these as
/// literal text. Order matters: fenced before inline.
fn strip_code_regions(content: &str) -> String {
    let no_fenced = FENCED_REGEX.replace_all(content, "");
    let stripped = INLINE_REGEX.replace_all(&no_fenced, "");
    stripped.into_owned()
}

fn scan_body_wikilinks(content: &str, rel_path: &str, page_slugs: &HashSet<String>, issues: &mut Vec<LintIssue>) {
    let stripped = strip_code_regions(content);
    let mut seen = HashSet::new();
    for caps in BODY_WIKILINK_REGEX.captures_iter(&stripped) {
        let slug = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        if slug.is_empty() || !seen.insert(slug.clone()) {
            continue;
        }
        if !page_slugs.contains(&slug) {
            issues.push(LintIssue {
                path: rel_path.to_string(),
                severity: LintSeverity::Warn,
                message: format!(
                    "broken wikilink in body: [[{slug}]] (no page named {slug}.md in any wiki/<type>/ folder)"
                ),
            });
        }
    }
}

fn page_size_threshold(rel_path: &str) -> Option<usize> {
    if rel_path == "index.md" {
        return Some(INDEX_MD_THRESHOLD);
    }
    if rel_path == "log.md" || rel_path == "overview.md" {
        return None;
    }
    let folder = rel_path.split('/').next()?;
    match folder {
        "synthesis" => Some(SYNTHESIS_THRESHOLD),
        "concepts" | "entities" | "modules" | "processes" => Some(TYPE_FOLDER_THRESHOLD),
        _ => None,
    }
}

fn check_page_size(rel_path: &str, content: &str, issues: &mut Vec<LintIssue>) {
    let Some(threshold) = page_size_threshold(rel_path) else {
        return;
    };
    let size = content.len();
    if size > threshold {
        issues.push(LintIssue {
            path: rel_path.to_string(),
            severity: LintSeverity::Warn,
            message: format!(
                "oversize page (size {size} bytes, threshold {threshold} bytes) — split or extract sub-page"
            ),
        });
    }
}

/// Walk `wiki/` looking for unexpected entries. Hidden entries (starting
/// with `.`) skip silently. Recognized root dirs (5 type folders + goals/)
/// are not flagged. Files at root other than nav specials are handled by
/// the existing root-page rule, not this scan.
fn scan_unexpected_root_dirs(wiki_root: &Path, issues: &mut Vec<LintIssue>) {
    let Ok(entries) = fs::read_dir(wiki_root) else { return };
    for e in entries.flatten() {
        let name = match e.file_name().into_string() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if name.starts_with('.') {
            continue;
        }
        let ft = match e.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if !ft.is_dir() {
            continue;
        }
        if !RECOGNIZED_ROOT_DIRS.contains(&name.as_str()) {
            issues.push(LintIssue {
                path: name.clone(),
                severity: LintSeverity::Warn,
                message: format!("unrecognized folder under wiki/: {name}"),
            });
        }
    }
}

fn scan_type_folder_for_unexpected(folder_path: &Path, folder: &str, issues: &mut Vec<LintIssue>) {
    let Ok(entries) = fs::read_dir(folder_path) else { return };
    for e in entries.flatten() {
        let name = match e.file_name().into_string() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if name.starts_with('.') {
            continue;
        }
        let ft = match e.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if ft.is_dir() {
            issues.push(LintIssue {
                path: format!("{folder}/{name}"),
                severity: LintSeverity::Warn,
                message: format!("nested sub-folder in type folder: {folder}/{name}"),
            });
        } else if ft.is_file() && !name.ends_with(".md") {
            issues.push(LintIssue {
                path: format!("{folder}/{name}"),
                severity: LintSeverity::Warn,
                message: format!("non-.md file in type folder: {folder}/{name}"),
            });
        }
    }
}

/// Validate a vault's `wiki/` subtree. Pure read — never writes. Mirrors TS
/// `lintWiki(vaultRoot)`. Returns coverage counts plus `Vec<LintIssue>`;
/// callers (auto-lint after ingest, `--check` standalone) decide how to
/// surface based on `error_count` vs `warn_count`.
///
/// `vault_root` is the `.codebus/` path (e.g. `/repo/.codebus/`).
pub fn lint_wiki(vault_root: impl AsRef<Path>) -> LintResult {
    let wiki_root = vault_root.as_ref().join("wiki");
    let mut issues: Vec<LintIssue> = Vec::new();
    let mut pages_scanned: usize = 0;
    let mut nav_files_scanned: usize = 0;

    if !wiki_root.exists() {
        return summarize(pages_scanned, nav_files_scanned, issues);
    }

    // 1. Catalog pages across the 5 type folders.
    let mut all_pages: Vec<PageEntry> = Vec::new();
    let mut slug_to_pages: HashMap<String, Vec<PageEntry>> = HashMap::new();
    for t in PageType::ALL {
        let folder = folder_name(t);
        let folder_path = wiki_root.join(folder);
        if !folder_path.exists() {
            continue;
        }
        // Unexpected-file scan happens here (per-type-folder readdir).
        scan_type_folder_for_unexpected(&folder_path, folder, &mut issues);

        let Ok(rd) = fs::read_dir(&folder_path) else { continue };
        for e in rd.flatten() {
            let name = match e.file_name().into_string() {
                Ok(s) => s,
                Err(_) => continue,
            };
            if !name.ends_with(".md") {
                continue;
            }
            let slug = name.trim_end_matches(".md").to_string();
            let entry = PageEntry {
                folder,
                filename: name.clone(),
                slug: slug.clone(),
                rel_path: format!("{folder}/{name}"),
                full_path: folder_path.join(&name),
            };
            all_pages.push(entry.clone());
            slug_to_pages.entry(slug).or_default().push(entry);
        }
    }
    let mut page_slugs: HashSet<String> = all_pages.iter().map(|p| p.slug.clone()).collect();

    // 1b. Special files at root that exist are also valid wikilink targets.
    for sf in SPECIAL_FILES {
        if wiki_root.join(sf).exists() {
            page_slugs.insert(sf.trim_end_matches(".md").to_string());
        }
    }

    // 1c. Unrecognized folders directly under wiki/.
    scan_unexpected_root_dirs(&wiki_root, &mut issues);

    // 2. Cross-folder slug collision.
    for (slug, entries) in &slug_to_pages {
        if entries.len() > 1 {
            let others: Vec<&str> = entries.iter().map(|e| e.rel_path.as_str()).collect();
            let others_str = others.join(", ");
            for e in entries {
                issues.push(LintIssue {
                    path: e.rel_path.clone(),
                    severity: LintSeverity::Warn,
                    message: format!(
                        "duplicate slug '{slug}' across folders: {others_str} — wikilink [[{slug}]] becomes ambiguous"
                    ),
                });
            }
        }
    }

    // 3. Walk pages — parse, validate related[], scan body wikilinks, page-size.
    for entry in &all_pages {
        let content = match fs::read_to_string(&entry.full_path) {
            Ok(s) => s,
            Err(e) => {
                issues.push(LintIssue {
                    path: entry.rel_path.clone(),
                    severity: LintSeverity::Error,
                    message: format!("file read failed: {e}"),
                });
                continue;
            }
        };

        check_page_size(&entry.rel_path, &content, &mut issues);

        let parsed = match parse_page(&content) {
            Ok(p) => {
                pages_scanned += 1;
                p
            }
            Err(e) => {
                issues.push(LintIssue {
                    path: entry.rel_path.clone(),
                    severity: LintSeverity::Error,
                    message: format!("frontmatter parse failed: {e}"),
                });
                continue;
            }
        };

        for r in &parsed.frontmatter.related {
            let m = RELATED_STRIP_REGEX.captures(r);
            let slug = match m {
                Some(caps) => caps.get(1).unwrap().as_str().trim().to_string(),
                None => {
                    issues.push(LintIssue {
                        path: entry.rel_path.clone(),
                        severity: LintSeverity::Error,
                        message: format!("related[] entry not in [[wikilink]] format: {r}"),
                    });
                    continue;
                }
            };
            if !page_slugs.contains(&slug) {
                issues.push(LintIssue {
                    path: entry.rel_path.clone(),
                    severity: LintSeverity::Error,
                    message: format!(
                        "broken wikilink in related: [[{slug}]] (no page named {slug}.md in any wiki/<type>/ folder)"
                    ),
                });
            }
        }

        scan_body_wikilinks(&parsed.body, &entry.rel_path, &page_slugs, &mut issues);
    }

    // 4. Pages directly under wiki/ root (other than nav specials).
    if let Ok(rd) = fs::read_dir(&wiki_root) {
        for e in rd.flatten() {
            let Ok(name) = e.file_name().into_string() else { continue };
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_file() && name.ends_with(".md") && !SPECIAL_FILES.contains(&name.as_str()) {
                issues.push(LintIssue {
                    path: name.clone(),
                    severity: LintSeverity::Warn,
                    message: format!(
                        "page lives in wiki/ root — schema §3 expects wiki/<type>/{name} (one of: concepts, entities, modules, processes, synthesis)"
                    ),
                });
            }
        }
    }

    // 5. Nav files presence + body wikilinks. index.md also gets page-size.
    for sf in SPECIAL_FILES {
        let full_path = wiki_root.join(sf);
        if !full_path.exists() {
            issues.push(LintIssue {
                path: sf.to_string(),
                severity: LintSeverity::Warn,
                message: format!("{sf} missing — schema §3 expects this special file"),
            });
            continue;
        }
        nav_files_scanned += 1;
        let Ok(content) = fs::read_to_string(&full_path) else { continue };
        check_page_size(sf, &content, &mut issues);
        scan_body_wikilinks(&content, sf, &page_slugs, &mut issues);
    }

    summarize(pages_scanned, nav_files_scanned, issues)
}

fn summarize(pages_scanned: usize, nav_files_scanned: usize, issues: Vec<LintIssue>) -> LintResult {
    let error_count = issues.iter().filter(|i| i.severity == LintSeverity::Error).count();
    let warn_count = issues.iter().filter(|i| i.severity == LintSeverity::Warn).count();
    LintResult { pages_scanned, nav_files_scanned, issues, error_count, warn_count }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_vault(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("codebus-lint-{name}-{}-{}", std::process::id(), rand_suffix()));
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
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();
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
        result.issues.iter().filter(|i| i.severity == sev && i.message.contains(msg_substring)).count()
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
        assert!(warns.iter().any(|i| i.severity == LintSeverity::Warn && i.message.contains("page lives in wiki/ root")));
        cleanup(&v);
    }

    #[test]
    fn folder_type_mismatch_is_not_flagged() {
        let v = tmp_vault("typefolder");
        write_page(&v, "concepts/foo.md", &fm("foo", "module", &[]), "# foo");
        let r = lint_wiki(&v);
        // No issue for foo.md other than possibly broken-wikilink (none here)
        let foo_issues = issues_for_path(&r, "concepts/foo.md");
        assert!(foo_issues.iter().all(|i| !i.message.contains("type") || !i.message.contains("folder")));
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
        assert!(warns.iter().any(|i| i.severity == LintSeverity::Warn && i.message.contains("missing")));
        cleanup(&v);
    }

    #[test]
    fn missing_log_md_is_flagged() {
        let v = tmp_vault("misslog");
        fs::remove_file(v.join("wiki/log.md")).unwrap();
        let r = lint_wiki(&v);
        let warns = issues_for_path(&r, "log.md");
        assert!(warns.iter().any(|i| i.severity == LintSeverity::Warn && i.message.contains("missing")));
        cleanup(&v);
    }

    #[test]
    fn missing_overview_md_is_not_flagged() {
        let v = tmp_vault("missoverview");
        // overview.md is intentionally absent in tmp_vault
        let r = lint_wiki(&v);
        assert!(r.issues.iter().all(|i| !(i.path == "overview.md" && i.message.contains("missing"))));
        cleanup(&v);
    }

    #[test]
    fn body_wikilink_to_nonexistent_slug_flagged_at_warn() {
        let v = tmp_vault("bodyghost");
        write_page(&v, "concepts/foo.md", &fm("foo", "concept", &[]), "see [[ghost]]");
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "concepts/foo.md");
        assert!(issues.iter().any(|i| i.severity == LintSeverity::Warn && i.message.contains("[[ghost]]")));
        cleanup(&v);
    }

    #[test]
    fn related_entry_not_wikilink_is_error() {
        let v = tmp_vault("relbad");
        let mut frontmatter = fm("foo", "concept", &[]);
        frontmatter = frontmatter.replace("related: []", "related:\n  - 'broken-no-brackets'");
        write_page(&v, "concepts/foo.md", &frontmatter, "# foo");
        let r = lint_wiki(&v);
        assert!(count(&r, LintSeverity::Error, "related[] entry not in [[wikilink]] format") > 0);
        cleanup(&v);
    }

    #[test]
    fn related_entry_to_missing_slug_is_error() {
        let v = tmp_vault("relmissing");
        write_page(&v, "concepts/foo.md", &fm("foo", "concept", &["[[ghost]]"]), "# foo");
        let r = lint_wiki(&v);
        assert!(count(&r, LintSeverity::Error, "broken wikilink in related") > 0);
        cleanup(&v);
    }

    #[test]
    fn frontmatter_parse_failure_is_error() {
        let v = tmp_vault("fmparse");
        fs::write(v.join("wiki/concepts/broken.md"), "---\n: : not yaml\n---\n").unwrap();
        let r = lint_wiki(&v);
        assert!(count(&r, LintSeverity::Error, "frontmatter parse failed") > 0);
        cleanup(&v);
    }

    #[test]
    fn body_wikilink_inside_inline_code_is_skipped() {
        let v = tmp_vault("inline");
        write_page(&v, "concepts/foo.md", &fm("foo", "concept", &[]), "use `[[wikilink]]` here");
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "concepts/foo.md");
        assert!(issues.is_empty(), "expected zero issues for inline-code wikilink, got {issues:?}");
        cleanup(&v);
    }

    #[test]
    fn body_wikilink_inside_fenced_block_is_skipped() {
        let v = tmp_vault("fenced");
        write_page(&v, "concepts/foo.md", &fm("foo", "concept", &[]), "\n```\n[[ghost]]\n```\n");
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "concepts/foo.md");
        assert!(issues.is_empty());
        cleanup(&v);
    }

    #[test]
    fn table_cell_wikilink_with_escaped_alias_resolves_to_bare_slug() {
        let v = tmp_vault("escape");
        write_page(&v, "concepts/resolver-resolve.md", &fm("resolve", "concept", &[]), "# resolve");
        write_page(&v, "concepts/foo.md", &fm("foo", "concept", &[]), "| col | [[resolver-resolve\\|Resolver]] |");
        let r = lint_wiki(&v);
        let foo_issues = issues_for_path(&r, "concepts/foo.md");
        assert!(foo_issues.is_empty());
        cleanup(&v);
    }

    #[test]
    fn table_cell_wikilink_with_escape_still_flags_broken_slug() {
        let v = tmp_vault("escapebroken");
        write_page(&v, "concepts/foo.md", &fm("foo", "concept", &[]), "| [[ghost\\|alias]] |");
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
        assert!(issues.iter().any(|i| i.severity == LintSeverity::Warn && i.message.contains("[[ghost]]")));
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
        let oversized: Vec<_> = issues.iter().filter(|i| i.message.contains("oversize")).collect();
        assert_eq!(oversized.len(), 1);
        assert!(oversized[0].message.contains("size 1500 bytes"));
        assert!(oversized[0].message.contains("threshold 1024 bytes"));
        cleanup(&v);
    }

    #[test]
    fn oversized_synthesis_page_is_flagged() {
        let v = tmp_vault("synsize");
        let body = "x".repeat(5800);
        write_page(&v, "synthesis/cart-flow.md", &fm("flow", "synthesis", &[]), &body);
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "synthesis/cart-flow.md");
        let oversized: Vec<_> = issues.iter().filter(|i| i.message.contains("oversize")).collect();
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
        let oversized: Vec<_> = issues.iter().filter(|i| i.message.contains("oversize")).collect();
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
        // Build content of EXACTLY 8192 bytes: TS test sentinel.
        // Construct a page whose total .md length equals 8192.
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
        write_page(&v, "concepts/foo.md", &fm("foo", "concept", &[]), &"z".repeat(1000));
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
        assert!(issues.iter().any(|i| i.severity == LintSeverity::Warn && i.message.contains("non-.md file in type folder")));
        cleanup(&v);
    }

    #[test]
    fn nested_sub_folder_in_type_folder_is_flagged() {
        let v = tmp_vault("nested");
        fs::create_dir_all(v.join("wiki/modules/legacy")).unwrap();
        fs::write(v.join("wiki/modules/legacy/old.md"), "x").unwrap();
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "modules/legacy");
        assert!(issues.iter().any(|i| i.message.contains("nested sub-folder in type folder")));
        cleanup(&v);
    }

    #[test]
    fn unrecognized_folder_under_wiki_is_flagged() {
        let v = tmp_vault("scratch");
        fs::create_dir_all(v.join("wiki/scratch")).unwrap();
        let r = lint_wiki(&v);
        let issues = issues_for_path(&r, "scratch");
        assert!(issues.iter().any(|i| i.message.contains("unrecognized folder under wiki/")));
        cleanup(&v);
    }

    #[test]
    fn hidden_entries_are_skipped_silently() {
        let v = tmp_vault("hidden");
        fs::create_dir_all(v.join("wiki/.obsidian")).unwrap();
        fs::write(v.join("wiki/.obsidian/app.json"), "{}").unwrap();
        fs::write(v.join("wiki/.gitkeep"), "").unwrap();
        let r = lint_wiki(&v);
        // No issue should be keyed to .obsidian or .gitkeep
        assert!(r.issues.iter().all(|i| !i.path.starts_with('.')));
        cleanup(&v);
    }

    // === fixture-level integration ===

    #[test]
    fn lint_uv_fixture_produces_known_warning_count() {
        // The pre-rewrite snapshot recorded TS lint output as
        // "0 error(s), 5 warning(s)" for the uv vault. Rust lint adds two
        // new rules (page-size, unexpected-file) — those are absent from
        // a vault that hasn't grown beyond thresholds (synthesis pages are
        // small) and has no extraneous files. The Rust port should produce
        // AT LEAST the same 5 warnings (legacy rules) and no extra ones
        // for the recorded fixture. Exact byte-equal stdout matching is
        // the job of Phase C task 4.10 once the CLI render layer exists.
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/uv-vault-snapshot/uv-wiki-snapshot");
        // Wrap fixture in a fake .codebus/wiki/ structure so lint_wiki
        // (which expects vaultRoot/wiki/) resolves correctly.
        let stage = std::env::temp_dir().join(format!(
            "codebus-lint-uvfixture-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        let _ = fs::remove_dir_all(&stage);
        fs::create_dir_all(stage.join("wiki")).unwrap();
        copy_dir_all(&fixture_root, &stage.join("wiki")).unwrap();

        let r = lint_wiki(&stage);
        // Legacy 5 warnings: 1 root-page (overview.md) + 4 broken body wikilinks.
        // Plus: any page-size or unexpected-file from the fixture state.
        assert_eq!(r.error_count, 0, "unexpected errors: {:?}", r.issues);
        assert!(r.warn_count >= 5, "expected ≥5 warnings, got {}: {:#?}", r.warn_count, r.issues);
        // Specifically: the 4 broken-body-wikilink warnings recorded in
        // tests/fixtures/uv-vault-snapshot/check-output.txt.
        let broken_body = count(&r, LintSeverity::Warn, "broken wikilink in body");
        assert!(broken_body >= 4, "expected ≥4 broken body wikilinks, got {broken_body}");
        // And the root-page warning for overview.md.
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
