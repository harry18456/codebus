//! Mirror source files from repo root into `.codebus/raw/code/`. Each
//! mirrored file's content is scanned with a caller-supplied `PiiScanner`,
//! and the per-match outcome is determined by the caller-supplied [`OnHit`]
//! policy (`Warn` / `Skip` / `Mask`). raw_sync does not select a default —
//! that is the caller's job (see `codebus-core::config::pii::PiiConfig`).
//!
//! v3-config behavior matrix:
//!   - clean file (zero matches) → mirrored byte-identical regardless of `on_hit`
//!   - non-UTF-8 file → mirrored byte-identical (no scan, no warn, no mask) regardless of `on_hit`
//!   - matches + `OnHit::Warn` → mirrored byte-identical, one warn line per match
//!   - matches + `OnHit::Skip` → file NOT mirrored, one warn line per match, `pii_skipped_files += 1`
//!   - matches + `OnHit::Mask` → mirrored with each `matched_text` replaced by
//!     `[REDACTED:<pattern_name>]` (descending byte-offset substitution so
//!     earlier replacements don't shift later match offsets), one warn line
//!     per match, `pii_masked_matches += matches.len()`

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::pii::PiiScanner;
use crate::pii::provider::{OnHit, PiiSeverity};

/// Top-level entries that SHALL be skipped only when they appear at the
/// repository root (relative path's first segment). `.codebus/` and
/// `.env` are tied to the repo's own conventions; the same names at a
/// deeper depth are user content (e.g. `docs/.codebus/notes.md`) and
/// SHALL be mirrored.
const ALWAYS_SKIP_AT_ROOT: &[&str] = &[".codebus", ".env"];

/// Directory names that SHALL be skipped wherever they appear in the
/// path — at the repo root, in a submodule, or inside an embedded
/// repository. Without this rule, a nested `.git/` introduced by a
/// submodule would be mirrored along with its config (potentially
/// containing token-bearing remote URLs) and its packed objects (which
/// the regex PII scanner cannot redact since they are binary). The
/// `vault` spec's `Raw Mirror with PII Scanner` requirement formalises
/// this contract.
const SKIP_DIR_NAME_ANYWHERE: &[&str] = &[".git"];

/// True iff `rel` (a path relative to the source repo root) falls
/// under one of the always-skipped locations. Shared by both
/// [`sync_with_scanner_into`] (mirror writer) and
/// [`walk_source_for_signal`] (drift detector) so the two filters
/// cannot drift apart — a recurrence of the `v3-bug-fixes` init→goal
/// re-sync regression.
fn is_excluded_path(rel: &std::path::Path) -> bool {
    let first_seg = rel.iter().next().and_then(|s| s.to_str()).unwrap_or("");
    if ALWAYS_SKIP_AT_ROOT.contains(&first_seg) {
        return true;
    }
    for seg in rel.iter() {
        let s = match seg.to_str() {
            Some(s) => s,
            None => continue,
        };
        if SKIP_DIR_NAME_ANYWHERE.contains(&s) {
            return true;
        }
    }
    false
}

/// Multi-segment paths (relative to repo root) the source-signal walk
/// MUST skip regardless of `.gitignore` state. These are codebus-managed
/// — written by `codebus init` itself — and including them in the source
/// signal would falsely trigger drift detection on every subsequent verb
/// invocation. The patterns cover the v3-lint repo-root skill bundle
/// dual-write locations.
const ALWAYS_SKIP_PATH_PREFIXES: &[&str] = &[
    ".claude/skills/codebus-goal",
    ".claude/skills/codebus-query",
    ".claude/skills/codebus-fix",
];

fn skip_codebus_managed(rel_path: &std::path::Path) -> bool {
    let s = rel_path.to_string_lossy().replace('\\', "/");
    ALWAYS_SKIP_PATH_PREFIXES
        .iter()
        .any(|prefix| s == *prefix || s.starts_with(&format!("{prefix}/")))
}
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncSummary {
    pub files: usize,
    pub bytes: u64,
    pub pii_matches: usize,
    /// Number of files NOT mirrored due to `OnHit::Skip`. Always zero when
    /// `on_hit != Skip` — only `Skip` skips files based on PII content.
    pub pii_skipped_files: usize,
    /// Total number of `[REDACTED:<pattern>]` substitutions written across
    /// all mirrored files. Always zero when `on_hit != Mask`.
    pub pii_masked_matches: usize,
}

/// Walk the source repository under the same rules as [`sync_with_scanner`]
/// (gitignore-aware, root dot-dirs skipped, files over 5 MiB skipped) but
/// without copying or scanning content. Returns `(file_count, total_bytes)`.
/// Used by verb commands (e.g., `goal`) to compute the current source signal
/// for drift detection without paying the cost of a full re-mirror.
pub fn walk_source_for_signal(repo_root: &Path) -> io::Result<(usize, u64)> {
    let mut builder = WalkBuilder::new(repo_root);
    builder
        .standard_filters(true)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .parents(false);
    let gi = repo_root.join(".gitignore");
    if gi.exists() {
        let _ = builder.add_ignore(&gi);
    }

    let mut files: usize = 0;
    let mut bytes: u64 = 0;

    for entry in builder.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path == repo_root {
            continue;
        }
        let rel = match path.strip_prefix(repo_root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if is_excluded_path(rel) {
            continue;
        }
        if skip_codebus_managed(rel) {
            continue;
        }
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }
        files += 1;
        bytes += meta.len();
    }

    Ok((files, bytes))
}

/// Mirror `repo_root` into `raw_code_dir` with `on_hit` policy applied to
/// PII matches. Warns are written to `io::stderr().lock()`. For deterministic
/// stderr capture in tests, call [`sync_with_scanner_into`] directly with a
/// caller-supplied `Write` sink.
pub fn sync_with_scanner(
    repo_root: &Path,
    raw_code_dir: &Path,
    scanner: &dyn PiiScanner,
    on_hit: OnHit,
) -> io::Result<SyncSummary> {
    let mut stderr = io::stderr().lock();
    sync_with_scanner_into(repo_root, raw_code_dir, scanner, on_hit, &mut stderr)
}

/// Same as [`sync_with_scanner`] but writes PII warning lines into the
/// caller-supplied `warn_sink` instead of stderr. Exposed primarily so
/// integration tests can verify warning line format and content without
/// process-level stderr capture.
///
/// Warning line format: `pii warn: <pattern_name> at <relative_path>:<byte_offset>\n`.
/// The `matched_text` of each match is intentionally NOT written to the
/// sink — emitting the literal secret would defeat the redaction intent.
pub fn sync_with_scanner_into<W: io::Write>(
    repo_root: &Path,
    raw_code_dir: &Path,
    scanner: &dyn PiiScanner,
    on_hit: OnHit,
    warn_sink: &mut W,
) -> io::Result<SyncSummary> {
    if raw_code_dir.exists() {
        fs::remove_dir_all(raw_code_dir)?;
    }
    fs::create_dir_all(raw_code_dir)?;

    let mut builder = WalkBuilder::new(repo_root);
    builder
        .standard_filters(true)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .parents(false);
    let gi = repo_root.join(".gitignore");
    if gi.exists() {
        let _ = builder.add_ignore(&gi);
    }

    let mut summary = SyncSummary::default();

    for entry in builder.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path == repo_root {
            continue;
        }

        let rel: PathBuf = match path.strip_prefix(repo_root) {
            Ok(r) => r.to_path_buf(),
            Err(_) => continue,
        };

        if is_excluded_path(&rel) {
            continue;
        }
        if skip_codebus_managed(&rel) {
            continue;
        }

        let dst = raw_code_dir.join(&rel);

        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            fs::create_dir_all(&dst)?;
            continue;
        }
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }

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

        // Use forward slashes in the warning line so output is consistent
        // across Windows / Unix even though `Path` separators differ.
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        // Branch on UTF-8 readability:
        //   UTF-8 → scan + on_hit branching (Warn / Skip / Mask)
        //   non-UTF-8 → fall through to verbatim copy (no scan, no warn)
        let utf8_content = fs::read_to_string(path).ok();
        let matches = match &utf8_content {
            Some(content) => scanner.scan(content, &rel_str),
            None => Vec::new(),
        };

        // Emit warn lines for every match before deciding mirror action.
        // Order: ascending byte offset (scanner contract). One line per match.
        for m in &matches {
            writeln!(
                warn_sink,
                "pii warn: {} at {}:{}",
                m.pattern_name, rel_str, m.start
            )?;
        }
        summary.pii_matches += matches.len();

        // v3-bug-fixes: `summary.bytes` SHALL track source-side bytes
        // (`meta.len()`) regardless of on_hit mode. The walk used by
        // `walk_source_for_signal` (consumed by drift detection in `goal`)
        // counts source-side bytes the same way, so accumulating them
        // identically here keeps the manifest's `total_bytes` field in
        // sync with what subsequent verb invocations re-compute. Mixing
        // source-bytes (walk) with destination-bytes (Mask `masked.len()` /
        // Skip omitted) caused init→goal drift detection to fire spuriously.
        summary.bytes += meta.len();

        // Decide what to write to dst based on on_hit + match presence.
        if matches.is_empty() {
            // No matches (or non-UTF-8): byte-identical copy.
            fs::copy(path, &dst)?;
            summary.files += 1;
            continue;
        }

        // v3-pii-severity-dispatch: split matches by severity. Critical
        // matches (AWS / Anthropic key) MUST be masked regardless of
        // user-configured `on_hit` — the security floor prevents real
        // credentials from entering the raw mirror recoverably. Warn
        // matches (email / ipv4) follow the user-configured `on_hit`.
        let has_critical = matches.iter().any(|m| m.severity == PiiSeverity::Critical);

        if has_critical {
            // Critical floor: mask every Critical match (and any Warn match
            // alongside it ONLY when on_hit also says Mask). The file is
            // ALWAYS mirrored even if on_hit was Skip — Skip dropping a
            // file that contains a real credential would lose the audit
            // trail the warn lines provide.
            let original = utf8_content.expect("matches non-empty implies UTF-8 content");
            let matches_to_mask: Vec<_> = matches
                .iter()
                .filter(|m| m.severity == PiiSeverity::Critical || on_hit == OnHit::Mask)
                .cloned()
                .collect();
            let masked = mask_matches(&original, &matches_to_mask);
            fs::write(&dst, masked.as_bytes())?;
            summary.files += 1;
            summary.pii_masked_matches += matches_to_mask.len();
        } else {
            // Warn-only file: follow user's on_hit policy strictly.
            match on_hit {
                OnHit::Warn => {
                    fs::copy(path, &dst)?;
                    summary.files += 1;
                }
                OnHit::Skip => {
                    summary.pii_skipped_files += 1;
                    // Do NOT copy — file is intentionally absent from mirror.
                }
                OnHit::Mask => {
                    let original = utf8_content.expect("matches non-empty implies UTF-8 content");
                    let masked = mask_matches(&original, &matches);
                    fs::write(&dst, masked.as_bytes())?;
                    summary.files += 1;
                    summary.pii_masked_matches += matches.len();
                }
            }
        }
    }

    Ok(summary)
}

/// Replace each match's `matched_text` substring in `content` with
/// `[REDACTED:<pattern_name>]`, processing matches in descending byte-offset
/// order so earlier substitutions do not shift later match offsets.
///
/// Assumes `matches` are non-overlapping and sorted ascending by byte offset
/// (scanner contract). Caller MUST NOT call this with empty matches.
fn mask_matches(content: &str, matches: &[crate::pii::provider::PiiMatch]) -> String {
    let mut out = content.to_string();
    // Iterate matches in descending order so each replacement does not
    // invalidate the offsets of yet-to-be-processed matches.
    for m in matches.iter().rev() {
        // Defensive bounds check: scanner is supposed to give us byte offsets
        // into the input slice, but if a buggy scanner emits out-of-range
        // offsets we'd rather skip the substitution than panic.
        if m.end > out.len() || m.start > m.end {
            continue;
        }
        let replacement = format!("[REDACTED:{}]", m.pattern_name);
        out.replace_range(m.start..m.end, &replacement);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pii::scanners::null_scanner::NullScanner;
    use crate::pii::scanners::regex_basic::RegexBasicScanner;
    use tempfile::TempDir;

    fn write(p: &Path, content: &[u8]) {
        if let Some(par) = p.parent() {
            fs::create_dir_all(par).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    fn null() -> NullScanner {
        NullScanner::new()
    }

    /// Capture-friendly helper: run sync into a `Vec<u8>` warn sink and
    /// return both the resulting summary and the captured warn output as a
    /// String. Used in OnHit branch tests to assert warn lines without
    /// depending on process-level stderr capture.
    fn run_sync(
        src: &Path,
        raw: &Path,
        scanner: &dyn PiiScanner,
        on_hit: OnHit,
    ) -> (SyncSummary, String) {
        let mut buf: Vec<u8> = Vec::new();
        let summary = sync_with_scanner_into(src, raw, scanner, on_hit, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        (summary, s)
    }

    #[test]
    fn copies_plain_files_preserving_structure() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("a.rs"), b"fn main() {}");
        write(&src.path().join("nested/b.rs"), b"// b");
        let s = sync_with_scanner(src.path(), raw.path(), &null(), OnHit::Warn).unwrap();
        assert_eq!(s.files, 2);
        assert_eq!(s.pii_matches, 0);
        assert!(raw.path().join("a.rs").exists());
        assert!(raw.path().join("nested/b.rs").exists());
    }

    #[test]
    fn always_skip_root_dot_codebus_dot_git_dot_env() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("real.rs"), b"x");
        write(&src.path().join(".codebus/CLAUDE.md"), b"schema");
        write(&src.path().join(".git/config"), b"[core]");
        write(&src.path().join(".env"), b"API_KEY=secret");
        sync_with_scanner(src.path(), raw.path(), &null(), OnHit::Warn).unwrap();
        assert!(raw.path().join("real.rs").exists());
        assert!(!raw.path().join(".codebus").exists());
        assert!(!raw.path().join(".git").exists());
        assert!(!raw.path().join(".env").exists());
    }

    #[test]
    fn gitignore_patterns_are_respected() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(
            &src.path().join(".gitignore"),
            b"node_modules\ntarget\n*.log\n",
        );
        write(&src.path().join("real.rs"), b"x");
        write(&src.path().join("node_modules/foo.js"), b"x");
        write(&src.path().join("target/debug/output.txt"), b"x");
        write(&src.path().join("debug.log"), b"x");
        sync_with_scanner(src.path(), raw.path(), &null(), OnHit::Warn).unwrap();
        assert!(raw.path().join("real.rs").exists());
        assert!(!raw.path().join("node_modules").exists());
        assert!(!raw.path().join("target").exists());
        assert!(!raw.path().join("debug.log").exists());
    }

    #[test]
    fn files_over_5_mib_are_skipped() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        let big = vec![0u8; (MAX_FILE_BYTES + 1) as usize];
        write(&src.path().join("huge.bin"), &big);
        write(&src.path().join("small.txt"), b"ok");
        sync_with_scanner(src.path(), raw.path(), &null(), OnHit::Warn).unwrap();
        assert!(!raw.path().join("huge.bin").exists());
        assert!(raw.path().join("small.txt").exists());
    }

    #[test]
    fn raw_dir_is_replaced_idempotently() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("a.rs"), b"x");
        write(&raw.path().join("stale.txt"), b"remove me");
        sync_with_scanner(src.path(), raw.path(), &null(), OnHit::Warn).unwrap();
        assert!(!raw.path().join("stale.txt").exists());
        assert!(raw.path().join("a.rs").exists());
    }

    #[test]
    fn null_scanner_yields_zero_pii_matches_for_secret_lookalike() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(
            &src.path().join("aws.py"),
            b"AWS_KEY=AKIAIOSFODNN7EXAMPLE\n",
        );
        let s = sync_with_scanner(src.path(), raw.path(), &null(), OnHit::Warn).unwrap();
        assert_eq!(s.pii_matches, 0);
        assert!(raw.path().join("aws.py").exists());
    }

    /// `OnHit::Warn`: file is mirrored byte-identically AND warn line is
    /// emitted per match.
    #[test]
    fn warn_mode_copies_file_and_emits_warn() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("docs.md"), b"contact alice@example.com\n");
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        // v3-pii-severity-dispatch: Warn-severity match (email) under Warn
        // policy → file mirrored byte-identical, warn line emitted, no mask.
        // (Critical-severity matches under Warn now mask — covered by the
        // dedicated `critical_match_under_warn_policy_is_masked` test below.)
        let (summary, warns) = run_sync(src.path(), raw.path(), &scanner, OnHit::Warn);
        assert_eq!(summary.pii_matches, 1);
        assert_eq!(summary.pii_skipped_files, 0);
        assert_eq!(summary.pii_masked_matches, 0);
        assert!(raw.path().join("docs.md").exists());
        let mirrored = fs::read_to_string(raw.path().join("docs.md")).unwrap();
        assert_eq!(mirrored, "contact alice@example.com\n");
        assert!(
            warns.contains("pii warn: email at docs.md:"),
            "warn line missing or wrong format: {warns:?}"
        );
    }

    /// `OnHit::Skip` for Warn-severity-only file: file is NOT mirrored;
    /// warn line still emitted; `pii_skipped_files` counter increments.
    /// (Critical-severity matches under Skip → file STILL mirrored with
    /// mask per the security floor; covered by
    /// `critical_match_under_skip_policy_is_masked_not_skipped`.)
    #[test]
    fn skip_mode_omits_matched_file() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("docs.md"), b"contact alice@example.com\n");
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        let (summary, warns) = run_sync(src.path(), raw.path(), &scanner, OnHit::Skip);
        assert_eq!(summary.pii_matches, 1);
        assert_eq!(summary.pii_skipped_files, 1);
        assert_eq!(summary.files, 0);
        assert!(!raw.path().join("docs.md").exists());
        assert!(
            warns.contains("pii warn: email"),
            "warn line missing under Skip mode: {warns:?}"
        );
    }

    /// `OnHit::Skip` with mixed input — clean files mirrored, Warn-only
    /// dirty files skipped. `pii_skipped_files` counts dirty files exactly
    /// (one per file, not per match). (Files with Critical matches would
    /// be force-mirrored under mask — covered by Critical-floor tests.)
    #[test]
    fn skip_mode_records_skipped_count() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("clean.rs"), b"fn ok() {}");
        write(&src.path().join("dirty1.py"), b"contact alice@example.com");
        write(
            &src.path().join("dirty2.py"),
            b"e1=alice@example.com\ne2=bob@example.com",
        );
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        let (summary, _) = run_sync(src.path(), raw.path(), &scanner, OnHit::Skip);
        assert_eq!(summary.pii_skipped_files, 2);
        assert_eq!(summary.files, 1, "only clean.rs should be mirrored");
        assert!(raw.path().join("clean.rs").exists());
        assert!(!raw.path().join("dirty1.py").exists());
        assert!(!raw.path().join("dirty2.py").exists());
    }

    /// `OnHit::Mask`: single match replaced with redaction marker.
    #[test]
    fn mask_mode_replaces_single_match() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(
            &src.path().join("creds.py"),
            b"pre AKIAIOSFODNN7EXAMPLE post",
        );
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        let (summary, _) = run_sync(src.path(), raw.path(), &scanner, OnHit::Mask);
        assert_eq!(summary.pii_matches, 1);
        assert_eq!(summary.pii_masked_matches, 1);
        assert_eq!(summary.files, 1);
        let mirrored = fs::read_to_string(raw.path().join("creds.py")).unwrap();
        assert_eq!(mirrored, "pre [REDACTED:aws-access-key] post");
    }

    /// `OnHit::Mask` with multiple matches: descending-order replacement
    /// preserves later offsets correctly. Spec example.
    #[test]
    fn mask_mode_replaces_multiple_matches_in_descending_order() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(
            &src.path().join("two.py"),
            b"start AKIAIOSFODNN7EXAMPLE middle alice@example.com end",
        );
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        let (summary, _) = run_sync(src.path(), raw.path(), &scanner, OnHit::Mask);
        assert_eq!(summary.pii_matches, 2);
        assert_eq!(summary.pii_masked_matches, 2);
        let mirrored = fs::read_to_string(raw.path().join("two.py")).unwrap();
        assert_eq!(
            mirrored,
            "start [REDACTED:aws-access-key] middle [REDACTED:email] end"
        );
    }

    /// `OnHit::Mask` against non-UTF-8 file: fall through to verbatim copy.
    /// No warn lines (regex scanner produces zero matches against non-UTF-8
    /// because we skipped the scan when read_to_string failed).
    #[test]
    fn mask_mode_falls_through_to_copy_for_non_utf8() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        // Bytes that are NOT valid UTF-8 (lone continuation byte 0x80).
        let bytes = vec![0xFFu8, 0xFE, 0x00, 0x80, 0xC0];
        write(&src.path().join("blob.bin"), &bytes);
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        let (summary, warns) = run_sync(src.path(), raw.path(), &scanner, OnHit::Mask);
        assert_eq!(summary.pii_matches, 0);
        assert_eq!(summary.pii_masked_matches, 0);
        assert_eq!(summary.files, 1);
        let mirrored = fs::read(raw.path().join("blob.bin")).unwrap();
        assert_eq!(mirrored, bytes);
        assert!(
            warns.is_empty(),
            "no warn lines for non-UTF-8 input: {warns:?}"
        );
    }

    /// `OnHit::Mask` summary: per-match counter accumulates across multiple
    /// matches in one file.
    #[test]
    fn mask_mode_records_masked_count() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(
            &src.path().join("multi.py"),
            b"a AKIAIOSFODNN7EXAMPLE b alice@example.com c 192.168.1.1 d",
        );
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        let (summary, _) = run_sync(src.path(), raw.path(), &scanner, OnHit::Mask);
        assert_eq!(summary.pii_matches, 3);
        assert_eq!(summary.pii_masked_matches, 3);
        assert_eq!(summary.files, 1);
        let mirrored = fs::read_to_string(raw.path().join("multi.py")).unwrap();
        // All three patterns replaced; surrounding text preserved.
        assert!(mirrored.contains("[REDACTED:aws-access-key]"));
        assert!(mirrored.contains("[REDACTED:email]"));
        assert!(mirrored.contains("[REDACTED:ipv4]"));
        assert!(!mirrored.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!mirrored.contains("alice@example.com"));
        assert!(!mirrored.contains("192.168.1.1"));
    }

    /// `OnHit::Warn` with multi-match file: file copied unchanged, all warn
    /// lines emitted, pii_matches accumulates exactly.
    #[test]
    fn warn_mode_accumulates_match_count_unchanged_copy() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(
            &src.path().join("logs.txt"),
            b"key1=alice@example.com\nkey2=192.168.1.1\n",
        );
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        // v3-pii-severity-dispatch: only Warn-severity matches in this file
        // (email + ipv4) → under Warn policy, file is mirrored byte-identical
        // and no mask occurs.
        let (summary, warns) = run_sync(src.path(), raw.path(), &scanner, OnHit::Warn);
        assert_eq!(summary.pii_matches, 2);
        assert_eq!(summary.pii_masked_matches, 0);
        assert_eq!(summary.pii_skipped_files, 0);
        let mirrored = fs::read_to_string(raw.path().join("logs.txt")).unwrap();
        assert_eq!(mirrored, "key1=alice@example.com\nkey2=192.168.1.1\n");
        assert_eq!(
            warns.matches("pii warn:").count(),
            2,
            "exactly 2 warn lines expected: {warns:?}"
        );
    }

    // === v3-pii-severity-dispatch: Critical floor overrides on_hit ===

    /// Spec scenario: "Critical severity ignores on_hit configuration" —
    /// AWS key (Critical) under Warn policy SHALL be masked in mirror,
    /// not just warn-logged.
    #[test]
    fn critical_match_under_warn_policy_is_masked() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("creds.py"), b"key = AKIAIOSFODNN7EXAMPLE");
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        let (summary, warns) = run_sync(src.path(), raw.path(), &scanner, OnHit::Warn);
        assert_eq!(summary.pii_matches, 1);
        assert_eq!(
            summary.pii_masked_matches, 1,
            "Critical match under Warn policy SHALL still be masked"
        );
        assert_eq!(summary.files, 1, "file SHALL still be mirrored");
        let mirrored = fs::read_to_string(raw.path().join("creds.py")).unwrap();
        assert_eq!(mirrored, "key = [REDACTED:aws-access-key]");
        assert!(warns.contains("pii warn: aws-access-key"));
    }

    /// Spec scenario: "File with Critical matches is masked even under
    /// Skip policy" — Critical floor overrides Skip, file is mirrored
    /// (with mask) instead of dropped.
    #[test]
    fn critical_match_under_skip_policy_is_masked_not_skipped() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("creds.py"), b"key = AKIAIOSFODNN7EXAMPLE");
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        let (summary, _) = run_sync(src.path(), raw.path(), &scanner, OnHit::Skip);
        assert_eq!(
            summary.pii_skipped_files, 0,
            "Critical floor SHALL prevent Skip"
        );
        assert_eq!(summary.pii_masked_matches, 1);
        assert_eq!(
            summary.files, 1,
            "file SHALL be mirrored despite Skip policy"
        );
        assert!(raw.path().join("creds.py").exists());
        let mirrored = fs::read_to_string(raw.path().join("creds.py")).unwrap();
        assert!(mirrored.contains("[REDACTED:aws-access-key]"));
    }

    /// Spec scenario "Critical-only mask under Warn policy": file with
    /// both Critical (AWS key) and Warn (email) matches under Warn policy
    /// SHALL mask only the Critical, leave the Warn match intact.
    #[test]
    fn mixed_severity_under_warn_policy_only_critical_masked() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(
            &src.path().join("contact.md"),
            b"creds: AKIAIOSFODNN7EXAMPLE -- contact alice@example.com",
        );
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        let (summary, warns) = run_sync(src.path(), raw.path(), &scanner, OnHit::Warn);
        assert_eq!(
            summary.pii_matches, 2,
            "scanner SHALL still find both matches"
        );
        assert_eq!(
            summary.pii_masked_matches, 1,
            "only Critical SHALL be masked under Warn"
        );
        let mirrored = fs::read_to_string(raw.path().join("contact.md")).unwrap();
        assert!(
            mirrored.contains("[REDACTED:aws-access-key]"),
            "AWS key SHALL be masked: {mirrored}"
        );
        assert!(
            mirrored.contains("alice@example.com"),
            "email SHALL be preserved unchanged: {mirrored}"
        );
        // warn sink contains both lines (audit trail).
        assert!(warns.contains("aws-access-key"));
        assert!(warns.contains("email"));
    }

    /// Spec scenario "File with only Warn matches is omitted from mirror
    /// under Skip" — but here under Warn policy, the file SHALL be mirrored
    /// byte-identical (the prior on_hit=Warn behavior, unchanged for files
    /// without Critical content).
    #[test]
    fn warn_only_file_under_warn_policy_unchanged() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        let original = b"contact alice@example.com please";
        write(&src.path().join("docs.md"), original);
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        let (summary, warns) = run_sync(src.path(), raw.path(), &scanner, OnHit::Warn);
        assert_eq!(summary.pii_matches, 1);
        assert_eq!(
            summary.pii_masked_matches, 0,
            "Warn under Warn SHALL NOT mask"
        );
        assert_eq!(summary.pii_skipped_files, 0);
        assert_eq!(summary.files, 1);
        let mirrored = fs::read(raw.path().join("docs.md")).unwrap();
        assert_eq!(
            mirrored, original,
            "Warn under Warn SHALL leave file byte-identical"
        );
        assert!(warns.contains("pii warn: email"));
    }

    // === raw-sync-nested-git-leak ===

    /// Helper: minimal coverage of the `is_excluded_path` decision table.
    #[test]
    fn is_excluded_path_root_only_codebus_and_env() {
        // Root-level `.codebus` and `.env` SHALL be excluded.
        assert!(is_excluded_path(Path::new(".codebus/manifest.yaml")));
        assert!(is_excluded_path(Path::new(".env")));
        // The same names at deeper depths are user content and SHALL
        // NOT be excluded (covers the spec scenario "Nested .codebus
        // directories at deeper depths are user content").
        assert!(!is_excluded_path(Path::new("docs/.codebus/notes.md")));
        assert!(!is_excluded_path(Path::new("a/.env")));
    }

    #[test]
    fn is_excluded_path_dot_git_anywhere() {
        // Root `.git` (regression of the prior root-only behavior).
        assert!(is_excluded_path(Path::new(".git/HEAD")));
        // Submodule / nested-repo `.git` (the bug this change fixes —
        // spec scenario "Mirror skips nested .git directories at any
        // depth").
        assert!(is_excluded_path(Path::new("vendor/foo/.git/config")));
        assert!(is_excluded_path(Path::new("a/b/c/.git/objects/x")));
        // A file literally named `git` (no leading dot) MUST NOT be
        // excluded; only the exact `.git` segment matches.
        assert!(!is_excluded_path(Path::new("vendor/foo/git/config")));
    }

    /// Spec scenario `Mirror skips nested .git directories at any depth`.
    #[test]
    fn mirror_skips_nested_dot_git() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("vendor/foo/.git/config"), b"[core]\n");
        write(&src.path().join("vendor/foo/.git/objects/abc"), &[0u8, 1, 2]);
        write(&src.path().join("vendor/foo/src/main.rs"), b"fn main() {}");
        write(&src.path().join("README.md"), b"# x");
        sync_with_scanner(src.path(), raw.path(), &null(), OnHit::Warn).unwrap();
        // Nested `.git/` SHALL produce zero mirror entries.
        assert!(!raw.path().join("vendor/foo/.git").exists());
        // Sibling source under the submodule SHALL be mirrored.
        assert!(raw.path().join("vendor/foo/src/main.rs").exists());
        // Unrelated root file SHALL be mirrored.
        assert!(raw.path().join("README.md").exists());
    }

    /// Spec scenario `Source signal walk excludes nested .git
    /// identically to mirror` — the drift-detection walk MUST share
    /// the exclusion filter so init→goal does not falsely re-sync.
    #[test]
    fn walk_source_for_signal_skips_nested_dot_git() {
        let src = TempDir::new().unwrap();
        write(&src.path().join("vendor/foo/.git/config"), b"[core]\n");
        write(&src.path().join("vendor/foo/.git/objects/abc"), &[0u8; 16]);
        write(&src.path().join("vendor/foo/src/main.rs"), b"fn main(){}");
        let (files, bytes) = walk_source_for_signal(src.path()).unwrap();
        // Only `vendor/foo/src/main.rs` should count.
        assert_eq!(files, 1, "exactly one mirrorable file (the .rs)");
        let main_bytes = fs::metadata(src.path().join("vendor/foo/src/main.rs"))
            .unwrap()
            .len();
        assert_eq!(bytes, main_bytes);
    }

    /// Spec scenario `Nested .codebus directories at deeper depths are
    /// user content and are mirrored`.
    #[test]
    fn mirror_includes_nested_dot_codebus_user_content() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("docs/.codebus/notes.md"), b"user content");
        sync_with_scanner(src.path(), raw.path(), &null(), OnHit::Warn).unwrap();
        // The deeper `.codebus/` is user content and SHALL be mirrored;
        // only the root-level `.codebus/` is the vault and excluded.
        assert!(raw.path().join("docs/.codebus/notes.md").exists());
    }
}
