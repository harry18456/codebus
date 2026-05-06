use crate::pii::scanners::null_scanner::NullScanner;
use crate::pii::{OnHit, PiiMatch, PiiScanner};
use ignore::WalkBuilder;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ALWAYS_SKIP_AT_ROOT: &[&str] = &[".codebus", ".git", ".env"];
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

/// Mirror `repo_root` into `raw_dir` using gitignore-aware traversal.
/// Thin wrapper preserved for callers that don't go through `cfg.pii` —
/// dispatches to [`sync_repo_to_raw_with_scanner`] with a [`NullScanner`]
/// and [`OnHit::Warn`]. With a NullScanner the scanner returns no matches
/// and the on_hit branch never fires, so output is byte-equal to a build
/// without PII filter wired in.
pub fn sync_repo_to_raw(repo_root: impl AsRef<Path>, raw_dir: impl AsRef<Path>) -> io::Result<()> {
    let null = NullScanner::new();
    sync_repo_to_raw_with_scanner(repo_root, raw_dir, &null, OnHit::Warn)
}

/// Mirror `repo_root` into `raw_dir` using gitignore-aware traversal,
/// invoking `scanner` against each candidate UTF-8 text file before it is
/// written to the destination. Behavior on a hit is decided by `on_hit`.
///
/// Skips:
///   - `.codebus/`, `.git/`, `.env` at repo root
///   - any path matched by repo's `.gitignore`
///   - hidden dotfiles (default `ignore::WalkBuilder` behavior)
///   - files larger than 5 MiB (lint / agent context budget)
///
/// Files that are not valid UTF-8 fall through to a byte-for-byte
/// `fs::copy` — the scanner is not invoked. Binary blobs cannot
/// reliably be matched by `&str`-typed regex without losing fidelity.
pub fn sync_repo_to_raw_with_scanner(
    repo_root: impl AsRef<Path>,
    raw_dir: impl AsRef<Path>,
    scanner: &dyn PiiScanner,
    on_hit: OnHit,
) -> io::Result<()> {
    let mut stderr_w = io::stderr();
    sync_repo_to_raw_inner(
        repo_root.as_ref(),
        raw_dir.as_ref(),
        scanner,
        on_hit,
        &mut stderr_w,
    )
}

fn sync_repo_to_raw_inner(
    repo_root: &Path,
    raw_dir: &Path,
    scanner: &dyn PiiScanner,
    on_hit: OnHit,
    stderr_w: &mut dyn io::Write,
) -> io::Result<()> {
    if raw_dir.exists() {
        fs::remove_dir_all(raw_dir)?;
    }
    fs::create_dir_all(raw_dir)?;

    let mut builder = WalkBuilder::new(repo_root);
    builder
        .standard_filters(true)
        .hidden(false) // we filter our own root-only hidden set
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .parents(false);
    // `git_ignore(true)` only consults .gitignore when an ancestor `.git/`
    // exists. For non-git source repos we still want to honor a top-level
    // .gitignore — explicitly add it as a custom ignore file. add_ignore
    // returns Some(error) on parse failure; we ignore it (best-effort).
    let gi = repo_root.join(".gitignore");
    if gi.exists() {
        let _ = builder.add_ignore(&gi);
    }
    let walker = builder.build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path == repo_root {
            continue;
        }

        let rel = match path.strip_prefix(repo_root) {
            Ok(r) => r.to_path_buf(),
            Err(_) => continue,
        };

        // Top-level always-skip list (matches TS ALWAYS_SKIP_AT_ROOT).
        let first_seg = rel.iter().next().and_then(|s| s.to_str()).unwrap_or("");
        if ALWAYS_SKIP_AT_ROOT.contains(&first_seg) {
            continue;
        }

        let dst = raw_dir.join(&rel);

        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            fs::create_dir_all(&dst)?;
        } else if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            let meta = match fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.len() > MAX_FILE_BYTES {
                continue;
            }
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            // Use forward slashes for `rel_path` in scanner / stderr output
            // so messages are stable across OSes (spec pins `src/foo`).
            let rel_path_str = rel.to_string_lossy().replace('\\', "/");

            match fs::read_to_string(path) {
                Ok(content) => {
                    let matches = scanner.scan(&content, &rel_path_str);
                    if matches.is_empty() {
                        fs::write(&dst, &content)?;
                    } else {
                        apply_on_hit(on_hit, &content, &matches, &rel_path_str, &dst, stderr_w)?;
                    }
                }
                Err(_) => {
                    // Binary / non-UTF-8 / IO error → fall through to original copy path.
                    fs::copy(path, &dst)?;
                }
            }
        }
    }

    Ok(())
}

fn apply_on_hit(
    on_hit: OnHit,
    content: &str,
    matches: &[PiiMatch],
    rel_path: &str,
    dst: &Path,
    stderr_w: &mut dyn io::Write,
) -> io::Result<()> {
    match on_hit {
        OnHit::Warn => {
            for m in matches {
                writeln!(
                    stderr_w,
                    "warning: PII match in {rel_path}: {} at offset {}",
                    m.pattern_name, m.start
                )?;
            }
            fs::write(dst, content)?;
        }
        OnHit::Skip => {
            // Spec: "naming the first match's pattern". `RegexBasicScanner`
            // returns matches sorted by start offset, so [0] is deterministic.
            let first = &matches[0];
            writeln!(
                stderr_w,
                "skipped: {rel_path} (reason: pii hit {})",
                first.pattern_name
            )?;
            // Skip = file is omitted from mirror entirely (no write, no placeholder).
        }
        OnHit::Mask => {
            let masked = apply_mask(content, matches);
            fs::write(dst, masked)?;
        }
    }
    Ok(())
}

/// Replace each matched byte range with `[REDACTED:<pattern_name>]`.
///
/// Walks `matches` from highest offset to lowest so earlier offsets are
/// not invalidated by replacements. When two matches overlap, the
/// later (higher-offset) match has already been applied and the earlier
/// match is dropped — last-match-wins per design.
fn apply_mask(content: &str, matches: &[PiiMatch]) -> String {
    let mut out = content.to_string();
    let mut last_end = content.len();
    for m in matches.iter().rev() {
        if m.end > last_end {
            // Overlap with an already-replaced range — drop earlier match.
            continue;
        }
        out.replace_range(m.start..m.end, &format!("[REDACTED:{}]", m.pattern_name));
        last_end = m.start;
    }
    out
}

#[allow(dead_code)]
fn dummy(_p: PathBuf) {} // keeps PathBuf import alive across feature flags

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "codebus-raw-{name}-{}-{}",
            std::process::id(),
            nanos()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn nanos() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }

    fn write(p: &Path, content: &str) {
        if let Some(par) = p.parent() {
            fs::create_dir_all(par).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    fn list_relative(root: &Path) -> Vec<String> {
        super::super::file_ops::list_files_recursive(root).unwrap()
    }

    #[test]
    fn copies_plain_files_preserving_structure() {
        let src = tmp("plain");
        let raw = tmp("plainraw");
        write(&src.join("a.rs"), "src a");
        write(&src.join("nested/b.rs"), "src b");
        sync_repo_to_raw(&src, &raw).unwrap();
        let mut files = list_relative(&raw);
        files.sort();
        assert_eq!(files, vec!["a.rs".to_string(), "nested/b.rs".into()]);
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn always_skip_root_dot_codebus_dot_git_dot_env() {
        let src = tmp("skiproot");
        let raw = tmp("skiprootraw");
        write(&src.join("real.rs"), "x");
        write(&src.join(".codebus/CLAUDE.md"), "schema");
        write(&src.join(".git/config"), "[core]");
        write(&src.join(".env"), "API_KEY=secret");
        sync_repo_to_raw(&src, &raw).unwrap();
        let files = list_relative(&raw);
        assert!(files.contains(&"real.rs".to_string()));
        assert!(!files.iter().any(|f| f.starts_with(".codebus")));
        assert!(!files.iter().any(|f| f.starts_with(".git")));
        assert!(!files.iter().any(|f| f == ".env"));
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn gitignore_patterns_are_respected() {
        let src = tmp("gi");
        let raw = tmp("giraw");
        write(&src.join(".gitignore"), "node_modules\ntarget\n*.log\n");
        write(&src.join("real.rs"), "x");
        write(&src.join("node_modules/foo.js"), "x");
        write(&src.join("target/debug/output.txt"), "x");
        write(&src.join("debug.log"), "x");
        sync_repo_to_raw(&src, &raw).unwrap();
        let files = list_relative(&raw);
        assert!(files.contains(&"real.rs".to_string()));
        assert!(!files.iter().any(|f| f.starts_with("node_modules")));
        assert!(!files.iter().any(|f| f.starts_with("target")));
        assert!(!files.iter().any(|f| f.ends_with(".log")));
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn files_over_size_limit_are_skipped() {
        let src = tmp("big");
        let raw = tmp("bigraw");
        let big = vec![0u8; (super::MAX_FILE_BYTES + 1) as usize];
        fs::write(src.join("huge.bin"), &big).unwrap();
        write(&src.join("small.txt"), "ok");
        sync_repo_to_raw(&src, &raw).unwrap();
        let files = list_relative(&raw);
        assert!(!files.contains(&"huge.bin".to_string()));
        assert!(files.contains(&"small.txt".to_string()));
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn raw_dir_is_replaced_idempotently() {
        let src = tmp("idem");
        let raw = tmp("idemraw");
        write(&src.join("a.rs"), "x");
        write(&raw.join("stale.txt"), "should be wiped");
        sync_repo_to_raw(&src, &raw).unwrap();
        let files = list_relative(&raw);
        assert!(!files.contains(&"stale.txt".to_string()));
        assert!(files.contains(&"a.rs".to_string()));
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    // ---------------------------------------------------------------
    // PII wiring tests (with-scanner variant)
    // ---------------------------------------------------------------

    use crate::pii::scanners::null_scanner::NullScanner;
    #[allow(unused_imports)]
    use crate::pii::scanners::regex_basic::RegexBasicScanner;
    use crate::pii::{OnHit, PiiMatch, PiiScanner};

    /// Build a fixture repo with mixed file content used across the
    /// PII wiring tests.
    fn build_pii_fixture(src: &Path) {
        write(&src.join("clean.rs"), "fn main() {}\n");
        write(&src.join("nested/sub.txt"), "hello world\n");
        write(&src.join("src/secrets.py"), "KEY=AKIAIOSFODNN7EXAMPLE\n");
    }

    /// Recording scanner that captures every (content, path) pair it sees.
    /// Used to assert that scan is / isn't invoked on specific files.
    struct RecordingScanner {
        seen: std::sync::Mutex<Vec<(String, String)>>,
    }

    impl RecordingScanner {
        fn new() -> Self {
            Self {
                seen: std::sync::Mutex::new(Vec::new()),
            }
        }
        fn paths(&self) -> Vec<String> {
            self.seen
                .lock()
                .unwrap()
                .iter()
                .map(|(_, p)| p.clone())
                .collect()
        }
    }

    impl PiiScanner for RecordingScanner {
        fn name(&self) -> &str {
            "recording"
        }
        fn scan(&self, content: &str, path: &str) -> Vec<PiiMatch> {
            self.seen
                .lock()
                .unwrap()
                .push((content.to_string(), path.to_string()));
            Vec::new()
        }
    }

    #[test]
    fn scanner_is_invoked_on_utf8_text_files() {
        // Spec: Invoke PiiScanner on each candidate text file before mirroring
        // — UTF-8 text file is scanned before mirror.
        let src = tmp("utf8scan");
        let raw = tmp("utf8scanraw");
        write(&src.join("src/secrets.py"), "KEY=AKIAIOSFODNN7EXAMPLE\n");
        let rec = RecordingScanner::new();
        sync_repo_to_raw_with_scanner(&src, &raw, &rec, OnHit::Warn).unwrap();
        let paths = rec.paths();
        assert!(
            paths.contains(&"src/secrets.py".to_string()),
            "expected scan invoked on src/secrets.py, got {paths:?}"
        );
        // File is mirrored regardless (clean / ignored matches).
        assert!(raw.join("src/secrets.py").exists());
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn binary_file_is_mirrored_byte_for_byte_without_scanning() {
        // Spec: Non-UTF-8 binary file is mirrored without scanning.
        let src = tmp("binarymirror");
        let raw = tmp("binarymirrorraw");
        // 0xFF 0xFE is invalid UTF-8 (UTF-16 BOM in different byte order, no
        // valid 1-byte / 2-byte / 3-byte / 4-byte sequence starts with 0xFF).
        let bytes: &[u8] = &[0xFF, 0xFE, 0x00, 0x42, 0x00, 0x49, 0x00, 0x4E];
        let p = src.join("assets/logo.png");
        if let Some(par) = p.parent() {
            fs::create_dir_all(par).unwrap();
        }
        fs::write(&p, bytes).unwrap();

        let rec = RecordingScanner::new();
        sync_repo_to_raw_with_scanner(&src, &raw, &rec, OnHit::Warn).unwrap();

        let paths = rec.paths();
        assert!(
            !paths.contains(&"assets/logo.png".to_string()),
            "scanner must NOT be invoked on binary file, got {paths:?}"
        );
        let dst = raw.join("assets/logo.png");
        assert!(dst.exists(), "binary file must still be mirrored");
        let dst_bytes = fs::read(&dst).unwrap();
        assert_eq!(dst_bytes, bytes, "binary mirror must be byte-for-byte");

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn empty_utf8_file_is_scanned_and_mirrored_empty() {
        // Spec: Empty file is mirrored without producing matches.
        let src = tmp("emptyfile");
        let raw = tmp("emptyfileraw");
        write(&src.join("empty.txt"), "");
        let rec = RecordingScanner::new();
        sync_repo_to_raw_with_scanner(&src, &raw, &rec, OnHit::Warn).unwrap();
        let paths = rec.paths();
        assert!(paths.contains(&"empty.txt".to_string()));
        let dst = raw.join("empty.txt");
        assert!(dst.exists());
        let body = fs::read(&dst).unwrap();
        assert!(body.is_empty(), "empty file should mirror empty");
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    use crate::pii::PiiSeverity;

    fn email_match(start: usize, end: usize) -> PiiMatch {
        PiiMatch {
            pattern_name: "email".to_string(),
            start,
            end,
            matched_text: String::new(),
            severity: PiiSeverity::Warn,
        }
    }

    fn ipv4_match(start: usize, end: usize) -> PiiMatch {
        PiiMatch {
            pattern_name: "ipv4".to_string(),
            start,
            end,
            matched_text: String::new(),
            severity: PiiSeverity::Warn,
        }
    }

    // -------- 3.1 OnHit::Warn --------

    #[test]
    fn warn_writes_one_stderr_line_per_match_and_mirrors_file() {
        // Spec: Single match warns and mirrors.
        // RegexBasicScanner finds "AKIAIOSFODNN7EXAMPLE" at offset 4 in
        // "KEY=AKIAIOSFODNN7EXAMPLE\n" — offset/pattern_name are stable.
        let src = tmp("warnsingle");
        let raw = tmp("warnsingleraw");
        let body = "KEY=AKIAIOSFODNN7EXAMPLE\n";
        write(&src.join("src/secrets.py"), body);

        let scanner = RegexBasicScanner::new(&[]).expect("builtin compiles");
        let mut stderr_buf: Vec<u8> = Vec::new();
        sync_repo_to_raw_inner(&src, &raw, &scanner, OnHit::Warn, &mut stderr_buf).unwrap();

        // File mirrored byte-for-byte
        let dst = raw.join("src/secrets.py");
        assert!(dst.exists());
        assert_eq!(fs::read_to_string(&dst).unwrap(), body);

        // Stderr line format pinned by spec
        let stderr_str = String::from_utf8(stderr_buf).unwrap();
        assert_eq!(
            stderr_str.trim_end_matches('\n'),
            "warning: PII match in src/secrets.py: aws-access-key at offset 4"
        );

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn warn_multiple_matches_one_line_per_match_in_ascending_offset_order() {
        // Spec: Multiple matches in one file produce one stderr line per match,
        // in ascending offset order.
        let src = tmp("warnmulti");
        let raw = tmp("warnmultiraw");
        // Two emails + one ipv4. Scanner sorts by start offset.
        let body = "alice@a.com and 10.0.0.1 and bob@b.com\n";
        write(&src.join("docs/contact.md"), body);

        let scanner = RegexBasicScanner::new(&[]).expect("builtin compiles");
        let mut stderr_buf: Vec<u8> = Vec::new();
        sync_repo_to_raw_inner(&src, &raw, &scanner, OnHit::Warn, &mut stderr_buf).unwrap();

        let stderr_str = String::from_utf8(stderr_buf).unwrap();
        let lines: Vec<&str> = stderr_str.lines().collect();
        assert_eq!(lines.len(), 3, "expected 3 lines, got: {stderr_str:?}");
        // All three must be `warning: PII match in docs/contact.md: ...`
        for l in &lines {
            assert!(
                l.starts_with("warning: PII match in docs/contact.md: "),
                "line did not match prefix: {l}"
            );
        }
        // Ascending-offset assertion: offsets parseable from line tail
        let offsets: Vec<usize> = lines
            .iter()
            .map(|l| {
                let tail = l.rsplit("at offset ").next().unwrap();
                tail.parse::<usize>().unwrap()
            })
            .collect();
        assert!(
            offsets.windows(2).all(|w| w[0] <= w[1]),
            "stderr lines not in ascending offset order: {offsets:?}"
        );

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    // -------- 3.2 OnHit::Skip --------

    #[test]
    fn skip_omits_file_and_writes_stderr_line() {
        // Spec: Match causes file to be skipped.
        let src = tmp("skipfile");
        let raw = tmp("skipfileraw");
        let body = "KEY=AKIAIOSFODNN7EXAMPLE\n";
        write(&src.join("secrets.env"), body);

        let scanner = RegexBasicScanner::new(&[]).expect("builtin compiles");
        let mut stderr_buf: Vec<u8> = Vec::new();
        sync_repo_to_raw_inner(&src, &raw, &scanner, OnHit::Skip, &mut stderr_buf).unwrap();

        // Destination must NOT exist
        assert!(
            !raw.join("secrets.env").exists(),
            "skipped file must be omitted from mirror"
        );

        let stderr_str = String::from_utf8(stderr_buf).unwrap();
        assert_eq!(
            stderr_str.trim_end_matches('\n'),
            "skipped: secrets.env (reason: pii hit aws-access-key)"
        );

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn skip_does_not_block_clean_sibling_files() {
        // Spec: Skipped file does not block sibling files.
        let src = tmp("skipsibling");
        let raw = tmp("skipsiblingraw");
        write(&src.join("clean.txt"), "nothing to see here\n");
        write(&src.join("dirty.txt"), "AKIAIOSFODNN7EXAMPLE\n");

        let scanner = RegexBasicScanner::new(&[]).expect("builtin compiles");
        let mut stderr_buf: Vec<u8> = Vec::new();
        sync_repo_to_raw_inner(&src, &raw, &scanner, OnHit::Skip, &mut stderr_buf).unwrap();

        let clean_dst = raw.join("clean.txt");
        assert!(clean_dst.exists());
        assert_eq!(
            fs::read_to_string(&clean_dst).unwrap(),
            "nothing to see here\n"
        );
        assert!(!raw.join("dirty.txt").exists());

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    // -------- 3.3 OnHit::Mask --------

    #[test]
    fn mask_replaces_single_match_in_place() {
        // Spec: Single match is replaced in place.
        // Spec values: pattern_name=email, start=10, end=27, content
        // "contact:  alice@example.com\n" (spaces match the spec offsets).
        let content = "contact:  alice@example.com\n";
        // Sanity-check the spec offsets against the actual string bytes.
        assert_eq!(&content[10..27], "alice@example.com");
        let matches = vec![email_match(10, 27)];

        let masked = apply_mask(content, &matches);
        assert_eq!(masked, "contact:  [REDACTED:email]\n");
    }

    #[test]
    fn mask_replaces_all_non_overlapping_matches() {
        // Spec: Multiple non-overlapping matches all replaced.
        let content = "email alice@a.com and ip 10.0.0.1 here\n";
        // Hand-pick offsets that match the substrings.
        let alice_start = content.find("alice@a.com").unwrap();
        let alice_end = alice_start + "alice@a.com".len();
        let ip_start = content.find("10.0.0.1").unwrap();
        let ip_end = ip_start + "10.0.0.1".len();
        let matches = vec![
            email_match(alice_start, alice_end),
            ipv4_match(ip_start, ip_end),
        ];

        let masked = apply_mask(content, &matches);
        assert!(masked.contains("[REDACTED:email]"));
        assert!(masked.contains("[REDACTED:ipv4]"));
        // Bytes outside the matched ranges are preserved.
        assert!(masked.starts_with("email "));
        assert!(masked.contains(" and ip "));
        assert!(masked.ends_with(" here\n"));
    }

    #[test]
    fn mask_preserves_line_count() {
        // Spec: Mask preserves line count when matches contain no '\n'.
        let content = "line1 alice@a.com\nline2\nline3 10.0.0.1\nline4\n";
        let alice_start = content.find("alice@a.com").unwrap();
        let alice_end = alice_start + "alice@a.com".len();
        let ip_start = content.find("10.0.0.1").unwrap();
        let ip_end = ip_start + "10.0.0.1".len();
        let matches = vec![
            email_match(alice_start, alice_end),
            ipv4_match(ip_start, ip_end),
        ];

        let masked = apply_mask(content, &matches);
        assert_eq!(
            content.lines().count(),
            masked.lines().count(),
            "line count must be preserved"
        );
    }

    #[test]
    fn mask_emits_no_stderr_lines_for_matches() {
        // Spec: Mask mode emits no stderr output for matches.
        let src = tmp("masknostderr");
        let raw = tmp("masknostderrraw");
        write(&src.join("contact.md"), "contact alice@example.com\n");

        let scanner = RegexBasicScanner::new(&[]).expect("builtin compiles");
        let mut stderr_buf: Vec<u8> = Vec::new();
        sync_repo_to_raw_inner(&src, &raw, &scanner, OnHit::Mask, &mut stderr_buf).unwrap();

        let stderr_str = String::from_utf8(stderr_buf).unwrap();
        assert!(
            !stderr_str.contains("warning:"),
            "mask must not emit warnings: {stderr_str:?}"
        );
        assert!(
            !stderr_str.contains("skipped:"),
            "mask must not emit skipped: {stderr_str:?}"
        );

        // And the file content was actually masked
        let dst_body = fs::read_to_string(raw.join("contact.md")).unwrap();
        assert!(dst_body.contains("[REDACTED:email]"));

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    // -------- 4.1 patterns_extra trigger --------

    #[test]
    fn patterns_extra_custom_rule_triggers_warn_with_custom_label() {
        // Spec: Custom regex hits trigger configured on_hit.
        let src = tmp("patternsextra");
        let raw = tmp("patternsextraraw");
        write(&src.join("notes.md"), "ticket INTERNAL-123456 closed\n");

        let extras = vec![r"\bINTERNAL-\d{6}\b".to_string()];
        let scanner = RegexBasicScanner::new(&extras).expect("custom pattern compiles");
        let mut stderr_buf: Vec<u8> = Vec::new();
        sync_repo_to_raw_inner(&src, &raw, &scanner, OnHit::Warn, &mut stderr_buf).unwrap();

        let stderr_str = String::from_utf8(stderr_buf).unwrap();
        assert!(
            stderr_str.contains("custom-0"),
            "expected custom-0 label in stderr, got: {stderr_str:?}"
        );
        assert!(stderr_str.contains("notes.md"));
        // File still mirrored (Warn mode).
        assert!(raw.join("notes.md").exists());

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn with_scanner_null_is_byte_equal_to_legacy_alias() {
        // Default scanner configuration preserves 0.2.0 behavior:
        // a NullScanner + OnHit::Warn run must produce exactly the
        // same mirror output as the legacy `sync_repo_to_raw`.
        let src = tmp("withnullsrc");
        let raw_legacy = tmp("withnullrawlegacy");
        let raw_with = tmp("withnullrawwith");

        build_pii_fixture(&src);

        sync_repo_to_raw(&src, &raw_legacy).unwrap();
        let null = NullScanner::new();
        sync_repo_to_raw_with_scanner(&src, &raw_with, &null, OnHit::Warn).unwrap();

        let mut files_legacy = list_relative(&raw_legacy);
        let mut files_with = list_relative(&raw_with);
        files_legacy.sort();
        files_with.sort();
        assert_eq!(files_legacy, files_with);

        for rel in &files_legacy {
            let a = fs::read(raw_legacy.join(rel)).unwrap();
            let b = fs::read(raw_with.join(rel)).unwrap();
            assert_eq!(a, b, "{rel} differs between legacy and with-scanner runs");
        }

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&raw_legacy);
        let _ = fs::remove_dir_all(&raw_with);
    }
}
