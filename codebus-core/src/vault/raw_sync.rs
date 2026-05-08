//! Mirror source files from repo root into `.codebus/raw/code/` in
//! NullScanner mode (no PII redaction; raw content copied verbatim).
//! PII filter wires in via change #3 v3-pii.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

const ALWAYS_SKIP_AT_ROOT: &[&str] = &[".codebus", ".git", ".env"];
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncSummary {
    pub files: usize,
    pub bytes: u64,
}

/// Mirror `repo_root` into `raw_code_dir` using gitignore-aware traversal.
/// NullScanner mode — file content is copied byte-for-byte. Skips the
/// always-skip root entries, oversize files, and any entry the source
/// `.gitignore` excludes.
pub fn sync_with_null_scanner(
    repo_root: &Path,
    raw_code_dir: &Path,
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

        let written = fs::copy(path, &dst)?;
        summary.files += 1;
        summary.bytes += written;
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(p: &Path, content: &[u8]) {
        if let Some(par) = p.parent() {
            fs::create_dir_all(par).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    #[test]
    fn copies_plain_files_preserving_structure() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("a.rs"), b"fn main() {}");
        write(&src.path().join("nested/b.rs"), b"// b");
        let s = sync_with_null_scanner(src.path(), raw.path()).unwrap();
        assert_eq!(s.files, 2);
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
        sync_with_null_scanner(src.path(), raw.path()).unwrap();
        assert!(raw.path().join("real.rs").exists());
        assert!(!raw.path().join(".codebus").exists());
        assert!(!raw.path().join(".git").exists());
        assert!(!raw.path().join(".env").exists());
    }

    #[test]
    fn gitignore_patterns_are_respected() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join(".gitignore"), b"node_modules\ntarget\n*.log\n");
        write(&src.path().join("real.rs"), b"x");
        write(&src.path().join("node_modules/foo.js"), b"x");
        write(&src.path().join("target/debug/output.txt"), b"x");
        write(&src.path().join("debug.log"), b"x");
        sync_with_null_scanner(src.path(), raw.path()).unwrap();
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
        sync_with_null_scanner(src.path(), raw.path()).unwrap();
        assert!(!raw.path().join("huge.bin").exists());
        assert!(raw.path().join("small.txt").exists());
    }

    #[test]
    fn raw_dir_is_replaced_idempotently() {
        let src = TempDir::new().unwrap();
        let raw = TempDir::new().unwrap();
        write(&src.path().join("a.rs"), b"x");
        write(&raw.path().join("stale.txt"), b"remove me");
        sync_with_null_scanner(src.path(), raw.path()).unwrap();
        assert!(!raw.path().join("stale.txt").exists());
        assert!(raw.path().join("a.rs").exists());
    }
}
