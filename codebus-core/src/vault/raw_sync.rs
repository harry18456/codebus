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
    /// core-quality-residuals (F2): Number of files NOT mirrored because
    /// they exceeded the `MAX_FILE_BYTES` (5 MiB) size limit. Each skip
    /// also emits one `mirror skip: oversized at ...` line on the warn
    /// sink (per spec vault §Raw Mirror with PII Scanner). Always
    /// incremented from the mirror-writer path only; the drift-detection
    /// `walk_source_for_signal` skips silently without affecting this
    /// counter (which is exposed only on `SyncSummary`).
    pub oversized_skipped_files: usize,
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
        // Use forward slashes in the warning line so output is consistent
        // across Windows / Unix even though `Path` separators differ.
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        if meta.len() > MAX_FILE_BYTES {
            // core-quality-residuals (F2): emit one warn line + bump the
            // summary counter so oversized skips are observable instead of
            // silent (per spec vault §Raw Mirror with PII Scanner). The
            // mirror-writer path SHALL surface; the drift-detection
            // `walk_source_for_signal` continues to skip silently with no
            // counter side-effect.
            //
            // Warn-write failure (e.g. EPIPE / Windows ERROR_NO_DATA when
            // stderr is a closed pipe under Tauri) SHALL NOT abort the
            // sync — the SKIP itself is the load-bearing behavior; the
            // warn line is observability. Swallow the write error so a
            // closed warn sink degrades gracefully instead of failing
            // `run_init` for the whole vault.
            let _ = writeln!(
                warn_sink,
                "mirror skip: oversized at {} ({} bytes > 5 MiB limit)",
                rel_str,
                meta.len()
            );
            summary.oversized_skipped_files += 1;
            continue;
        }
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

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

/// Replace each match's span in `content` with `[REDACTED:<pattern_name>]`.
///
/// Overlapping or nested matches across rules (most plausibly when a custom
/// `patterns_extra` regex frames a region containing an embedded builtin hit
/// like email/ipv4/key) are merged into disjoint spans before substitution —
/// without the merge, the descending `replace_range` strategy would cut into
/// already-substituted regions and either corrupt the output or leave the
/// inner secret partly visible. Each merged span uses the earliest-starting
/// match's `pattern_name` as the `[REDACTED:...]` label; adjacent
/// non-overlapping matches stay separate.
fn mask_matches(content: &str, matches: &[crate::pii::provider::PiiMatch]) -> String {
    // Coalesce overlapping/nested intervals. Empty `matches` short-circuits
    // to verbatim content so callers do not need a special-case branch.
    let mut sorted: Vec<&crate::pii::provider::PiiMatch> = matches.iter().collect();
    sorted.sort_by_key(|m| (m.start, m.end));
    let mut merged: Vec<(usize, usize, String)> = Vec::with_capacity(sorted.len());
    for m in sorted {
        if m.end <= m.start || m.end > content.len() {
            continue;
        }
        match merged.last_mut() {
            // Strict `<` so spans that merely touch (e.g. `[0,5)` then `[5,8)`)
            // stay separate and keep their own labels — only true overlap merges.
            Some(last) if m.start < last.1 => {
                if m.end > last.1 {
                    last.1 = m.end;
                }
            }
            _ => merged.push((m.start, m.end, m.pattern_name.clone())),
        }
    }
    let mut out = content.to_string();
    // Iterate merged spans in descending order so each replacement does not
    // invalidate the offsets of yet-to-be-processed spans.
    for (start, end, name) in merged.into_iter().rev() {
        let replacement = format!("[REDACTED:{name}]");
        out.replace_range(start..end, &replacement);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pii::provider::{PiiMatch, PiiSeverity};
    use crate::pii::scanners::null_scanner::NullScanner;
    use crate::pii::scanners::regex_basic::RegexBasicScanner;
    use tempfile::TempDir;

    fn pm(name: &str, start: usize, end: usize) -> PiiMatch {
        PiiMatch {
            pattern_name: name.to_string(),
            start,
            end,
            matched_text: String::new(),
            severity: PiiSeverity::Warn,
        }
    }

    /// Empty match list returns content verbatim (callers no longer special-case).
    #[test]
    fn mask_matches_empty_returns_verbatim() {
        assert_eq!(mask_matches("hello world", &[]), "hello world");
    }

    /// Disjoint, non-touching matches each get their own `[REDACTED:...]` token.
    #[test]
    fn mask_matches_disjoint_keeps_both_labels() {
        // "aaa BBB ccc DDD"; mask BBB[4..7] and DDD[12..15] separately.
        let s = "aaa BBB ccc DDD";
        let m = vec![pm("alpha", 4, 7), pm("beta", 12, 15)];
        let out = mask_matches(s, &m);
        assert_eq!(out, "aaa [REDACTED:alpha] ccc [REDACTED:beta]");
    }

    /// Adjacent (touching but not overlapping) ranges stay separate — strict
    /// `<` in the merge predicate prevents collapsing two distinct patterns
    /// that just happen to abut.
    #[test]
    fn mask_matches_adjacent_ranges_stay_separate() {
        // "aaabbb"; alpha covers [0..3), beta covers [3..6).
        let s = "aaabbb";
        let m = vec![pm("alpha", 0, 3), pm("beta", 3, 6)];
        let out = mask_matches(s, &m);
        assert_eq!(out, "[REDACTED:alpha][REDACTED:beta]");
    }

    /// Outer (custom) span fully contains an inner builtin (e.g. email) span —
    /// the realistic shape of the F1 bug. With the merge, the union is replaced
    /// once and the inner secret cannot leak.
    #[test]
    fn mask_matches_nested_inner_match_is_subsumed() {
        // "tag=<alice@example.com>"; outer custom covers [4..23), inner email
        // covers [5..22) (the address itself, no angle brackets).
        let s = "tag=<alice@example.com>";
        // sanity-check the offsets so the test fails loudly if string layout shifts.
        assert_eq!(&s[4..23], "<alice@example.com>");
        assert_eq!(&s[5..22], "alice@example.com");
        let m = vec![pm("custom-conn", 4, 23), pm("email", 5, 22)];
        let out = mask_matches(s, &m);
        assert_eq!(out, "tag=[REDACTED:custom-conn]");
        assert!(
            !out.contains("alice@example.com"),
            "inner secret must not survive the merge: {out:?}"
        );
    }

    /// Two ranges with partial overlap (A end overlaps B start) merge into one
    /// span; before the F1 fix the descending `replace_range` would corrupt
    /// the trailing part of A after B's substitution shifted offsets.
    #[test]
    fn mask_matches_partial_overlap_merges_to_union() {
        // "0123456789ABCDE"; alpha covers [2..8), beta covers [6..12).
        let s = "0123456789ABCDE";
        let m = vec![pm("alpha", 2, 8), pm("beta", 6, 12)];
        let out = mask_matches(s, &m);
        // Merged span [2..12) replaced by alpha's label (earliest start).
        assert_eq!(out, "01[REDACTED:alpha]CDE");
    }

    /// Identical span from two patterns (duplicate hit) merges to a single
    /// token rather than producing nested `[REDACTED:[REDACTED:...]]` after
    /// the descending overwrite.
    #[test]
    fn mask_matches_identical_spans_dedup_to_one() {
        let s = "left middle right";
        let m = vec![pm("alpha", 5, 11), pm("beta", 5, 11)];
        let out = mask_matches(s, &m);
        assert_eq!(out, "left [REDACTED:alpha] right");
    }

    /// Out-of-range and degenerate matches are silently skipped (defensive —
    /// preserves the pre-F1 behavior for buggy scanners).
    #[test]
    fn mask_matches_out_of_range_and_degenerate_are_skipped() {
        let s = "short";
        let m = vec![
            pm("oob", 100, 200),
            pm("zero", 2, 2),
            pm("ok", 0, 3),
        ];
        let out = mask_matches(s, &m);
        assert_eq!(out, "[REDACTED:ok]rt");
    }

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
        // core-quality-residuals (F2): the oversized file SHALL be skipped
        // AND the warn sink SHALL contain exactly one `mirror skip: oversized`
        // line AND the summary's `oversized_skipped_files` counter SHALL equal
        // one (per spec vault §Raw Mirror with PII Scanner). Use `run_sync`
        // helper to capture both the summary and the warn sink content.
        let (summary, warn) = run_sync(src.path(), raw.path(), &null(), OnHit::Warn);
        assert!(!raw.path().join("huge.bin").exists());
        assert!(raw.path().join("small.txt").exists());
        assert_eq!(
            summary.oversized_skipped_files, 1,
            "summary SHALL record one oversized skip; warn sink was: {warn}"
        );
        assert!(
            warn.contains("mirror skip: oversized at huge.bin"),
            "warn sink SHALL contain a `mirror skip: oversized at huge.bin` line; got: {warn}"
        );
        assert!(
            warn.contains("> 5 MiB limit"),
            "warn line SHALL include the `> 5 MiB limit` reason; got: {warn}"
        );
    }

    /// core-quality-residuals (F2): single oversized file produces a warn line
    /// containing the byte count AND increments `oversized_skipped_files` by
    /// exactly one. Mirrors spec vault §Raw Mirror with PII Scanner scenario
    /// "Mirror skips files exceeding the size limit and emits a stderr warning".
    #[test]
    fn oversized_file_warn_line_includes_byte_count_and_increments_counter() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        let oversized_bytes = (MAX_FILE_BYTES + 42) as usize;
        let big = vec![0u8; oversized_bytes];
        write(&src.path().join("docs/huge.bin"), &big);
        let (summary, warn) = run_sync(src.path(), raw.path(), &null(), OnHit::Warn);
        assert_eq!(summary.oversized_skipped_files, 1);
        // Warn line SHALL contain the byte count AND identify the rule.
        assert!(
            warn.contains(&format!("({oversized_bytes} bytes")),
            "warn line SHALL include the literal byte count; got: {warn}"
        );
        assert!(
            warn.contains("> 5 MiB limit"),
            "warn line SHALL include the `> 5 MiB limit` reason; got: {warn}"
        );
        // Forward-slash path normalisation (consistent across Windows / Unix).
        assert!(
            warn.contains("mirror skip: oversized at docs/huge.bin"),
            "warn line SHALL contain forward-slash-normalised relative path; got: {warn}"
        );
    }

    /// core-quality-residuals (F2): multiple oversized files aggregate the
    /// counter AND emit one warn line per file; small files are still
    /// mirrored alongside. Mirrors spec vault scenario "Oversized counter
    /// aggregates across multiple skipped files".
    #[test]
    fn multiple_oversized_files_aggregate_counter_and_warns() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        let big = vec![0u8; (MAX_FILE_BYTES + 1) as usize];
        write(&src.path().join("a.bin"), &big);
        write(&src.path().join("b.bin"), &big);
        write(&src.path().join("small.txt"), b"ok");
        let (summary, warn) = run_sync(src.path(), raw.path(), &null(), OnHit::Warn);
        assert_eq!(
            summary.oversized_skipped_files, 2,
            "summary SHALL aggregate to 2 oversized skips; warn sink was: {warn}"
        );
        // Each oversized file SHALL produce exactly one warn line.
        let a_count = warn.matches("mirror skip: oversized at a.bin").count();
        let b_count = warn.matches("mirror skip: oversized at b.bin").count();
        assert_eq!(a_count, 1, "a.bin SHALL appear in exactly one warn line; got {a_count} in {warn}");
        assert_eq!(b_count, 1, "b.bin SHALL appear in exactly one warn line; got {b_count} in {warn}");
        // Small files SHALL still be mirrored alongside the oversized skips.
        assert!(raw.path().join("small.txt").exists());
        assert!(!raw.path().join("a.bin").exists());
        assert!(!raw.path().join("b.bin").exists());
    }

    /// core-quality-residuals (F2): when the `warn_sink` itself fails to
    /// accept writes (e.g. EPIPE / Windows ERROR_NO_DATA when stderr is a
    /// closed pipe under Tauri's GUI process), the sync SHALL still:
    ///   - skip the oversized file from the mirror (load-bearing behavior)
    ///   - increment `summary.oversized_skipped_files` (observable to caller)
    ///   - return `Ok(SyncSummary)` for the whole sync (NOT abort)
    /// The warn line itself is best-effort observability — losing it on a
    /// broken sink SHALL NOT bubble up as init failure. Discovered via
    /// real-binary GUI verification (codebus-app + CDP add_vault path).
    #[test]
    fn oversized_skip_survives_failing_warn_sink() {
        struct AlwaysErrSink;
        impl io::Write for AlwaysErrSink {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "simulated EPIPE",
                ))
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "simulated EPIPE",
                ))
            }
        }
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        let big = vec![0u8; (MAX_FILE_BYTES + 1) as usize];
        write(&src.path().join("huge.bin"), &big);
        write(&src.path().join("small.txt"), b"ok");
        // Use the failing sink directly — sync_with_scanner_into SHALL still
        // complete successfully despite warn-write errors.
        let summary = sync_with_scanner_into(
            src.path(),
            raw.path(),
            &null(),
            OnHit::Warn,
            &mut AlwaysErrSink,
        )
        .expect("sync SHALL NOT abort when warn sink errors");
        assert_eq!(
            summary.oversized_skipped_files, 1,
            "counter SHALL still increment even when warn write fails"
        );
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
