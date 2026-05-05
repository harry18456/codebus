use ignore::WalkBuilder;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ALWAYS_SKIP_AT_ROOT: &[&str] = &[".codebus", ".git", ".env"];
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

/// Mirror `repo_root` into `raw_dir` using gitignore-aware traversal.
/// Skips:
///   - `.codebus/`, `.git/`, `.env` at repo root
///   - any path matched by repo's `.gitignore`
///   - hidden dotfiles (default `ignore::WalkBuilder` behavior)
///   - files larger than 5 MiB (lint / agent context budget)
///
/// Behavior matches TS `syncRepoToRaw` semantics. The `ignore` crate uses
/// the same gitignore parser as `ripgrep`, so edge cases (negation,
/// directory-only patterns, multiple .gitignore files) are handled
/// correctly out of the box — broader than the TS hand-rolled matcher.
pub fn sync_repo_to_raw(repo_root: impl AsRef<Path>, raw_dir: impl AsRef<Path>) -> io::Result<()> {
    let repo_root = repo_root.as_ref();
    let raw_dir = raw_dir.as_ref();

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
            fs::copy(path, &dst)?;
        }
    }

    Ok(())
}

#[allow(dead_code)]
fn dummy(_p: PathBuf) {} // keeps PathBuf import alive across feature flags

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("codebus-raw-{name}-{}-{}", std::process::id(), nanos()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn nanos() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos()
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
}
