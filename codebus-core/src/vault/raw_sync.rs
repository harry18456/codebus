//! Mirror source files from repo root into `.codebus/raw/code/`. Each
//! mirrored file's content is scanned with a caller-supplied `PiiScanner`.
//! Default on-hit behavior is Warn: every match emits one stderr line in
//! the format `pii warn: <pattern_name> at <rel_path>:<byte_offset>` and
//! the file is mirrored unchanged. Caller picks the scanner; raw_sync
//! does not select a default.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::pii::PiiScanner;

const ALWAYS_SKIP_AT_ROOT: &[&str] = &[".codebus", ".git", ".env"];
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncSummary {
    pub files: usize,
    pub bytes: u64,
    pub pii_matches: usize,
}

/// Mirror `repo_root` into `raw_code_dir` using gitignore-aware traversal,
/// invoking `scanner` against each mirrored file's UTF-8 content. Skips the
/// always-skip root entries, oversize files, and any entry the source
/// `.gitignore` excludes. PII matches emit a warning line per match to
/// stderr; files are mirrored regardless (Warn on-hit behavior is hardcoded
/// for v3-pii).
///
/// This is a thin wrapper over [`sync_with_scanner_into`] that pipes
/// warnings to `io::stderr().lock()`. Tests requiring deterministic stderr
/// capture should call [`sync_with_scanner_into`] directly with their own
/// `Write` buffer.
pub fn sync_with_scanner(
    repo_root: &Path,
    raw_code_dir: &Path,
    scanner: &dyn PiiScanner,
) -> io::Result<SyncSummary> {
    let mut stderr = io::stderr().lock();
    sync_with_scanner_into(repo_root, raw_code_dir, scanner, &mut stderr)
}

/// Same as [`sync_with_scanner`] but writes PII warning lines into the
/// caller-supplied `warn_sink` instead of stderr. Exposed primarily so
/// integration tests can verify warning line format and content without
/// process-level stderr capture. Production callers should use
/// [`sync_with_scanner`].
///
/// Warning line format: `pii warn: <pattern_name> at <relative_path>:<byte_offset>\n`.
/// The `matched_text` of each match is intentionally NOT written to the
/// sink — emitting the literal secret would defeat the redaction intent.
pub fn sync_with_scanner_into<W: io::Write>(
    repo_root: &Path,
    raw_code_dir: &Path,
    scanner: &dyn PiiScanner,
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

        let first_seg = rel.iter().next().and_then(|s| s.to_str()).unwrap_or("");
        if ALWAYS_SKIP_AT_ROOT.contains(&first_seg) {
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

        // Read content for scanning. Non-UTF8 files (binary blobs) yield no
        // matches but are still mirrored verbatim below — the regex scanner
        // is a textual grep, not a binary analyzer.
        // Use forward slashes in the warning line so output is consistent
        // across Windows / Unix even though `Path` separators differ.
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if let Ok(content) = fs::read_to_string(path) {
            let matches = scanner.scan(&content, &rel_str);
            for m in &matches {
                writeln!(
                    warn_sink,
                    "pii warn: {} at {}:{}",
                    m.pattern_name, rel_str, m.start
                )?;
            }
            summary.pii_matches += matches.len();
        }

        let written = fs::copy(path, &dst)?;
        summary.files += 1;
        summary.bytes += written;
    }

    Ok(summary)
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

    #[test]
    fn copies_plain_files_preserving_structure() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("a.rs"), b"fn main() {}");
        write(&src.path().join("nested/b.rs"), b"// b");
        let s = sync_with_scanner(src.path(), raw.path(), &null()).unwrap();
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
        sync_with_scanner(src.path(), raw.path(), &null()).unwrap();
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
        sync_with_scanner(src.path(), raw.path(), &null()).unwrap();
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
        sync_with_scanner(src.path(), raw.path(), &null()).unwrap();
        assert!(!raw.path().join("huge.bin").exists());
        assert!(raw.path().join("small.txt").exists());
    }

    #[test]
    fn raw_dir_is_replaced_idempotently() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("a.rs"), b"x");
        write(&raw.path().join("stale.txt"), b"remove me");
        sync_with_scanner(src.path(), raw.path(), &null()).unwrap();
        assert!(!raw.path().join("stale.txt").exists());
        assert!(raw.path().join("a.rs").exists());
    }

    #[test]
    fn null_scanner_yields_zero_pii_matches_for_secret_lookalike() {
        // Drives Task 11's "NullScanner happy-path" criterion: even when the
        // file content matches what RegexBasic would catch, NullScanner SHALL
        // count zero (defensive contract pin against accidental scanner swap).
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(
            &src.path().join("aws.py"),
            b"AWS_KEY=AKIAIOSFODNN7EXAMPLE\n",
        );
        let s = sync_with_scanner(src.path(), raw.path(), &null()).unwrap();
        assert_eq!(s.pii_matches, 0);
        assert!(raw.path().join("aws.py").exists());
    }

    #[test]
    fn regex_basic_scanner_counts_aws_match_and_still_mirrors_file() {
        // Drives Task 11's "RegexBasic + AKIA shape" criterion: count
        // accumulates AND file remains mirrored under Warn policy.
        // Stderr line format is verified separately by the integration test
        // in `tests/vault_init.rs` (Task 15) via process-level stderr capture.
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(
            &src.path().join("aws.py"),
            b"AWS_KEY=AKIAIOSFODNN7EXAMPLE\n",
        );
        let scanner = RegexBasicScanner::new(&[]).unwrap();
        let s = sync_with_scanner(src.path(), raw.path(), &scanner).unwrap();
        assert_eq!(s.pii_matches, 1, "expected 1 PII match, got {}", s.pii_matches);
        assert!(
            raw.path().join("aws.py").exists(),
            "file SHALL be mirrored unchanged under Warn on-hit policy"
        );
    }
}
