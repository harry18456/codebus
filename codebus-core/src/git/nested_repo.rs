use std::io;
use std::path::Path;
use std::process::Command;

/// Initialize a nested git repo at `vault_root` if absent. Configures local
/// `user.email` / `user.name` so codebus's auto-commits don't depend on the
/// user's global git config.
pub fn init_nested_repo(vault_root: impl AsRef<Path>) -> io::Result<()> {
    let vault_root = vault_root.as_ref();
    if vault_root.join(".git").exists() {
        return Ok(());
    }
    run_git(vault_root, &["init", "-b", "main", "-q"])?;
    run_git(vault_root, &["config", "user.email", "codebus@local"])?;
    run_git(vault_root, &["config", "user.name", "codebus"])?;
    Ok(())
}

/// `git add -A` then commit with `message` if the working tree has any
/// staged change. Returns the resulting `HEAD` sha (or the existing HEAD
/// when there was nothing to commit).
pub fn auto_commit(vault_root: impl AsRef<Path>, message: &str) -> io::Result<String> {
    let vault_root = vault_root.as_ref();
    run_git(vault_root, &["add", "-A"])?;

    let status = capture_git(vault_root, &["status", "--porcelain"])?;
    if status.trim().is_empty() {
        let head = capture_git(vault_root, &["rev-parse", "HEAD"])?;
        return Ok(head.trim().to_string());
    }

    run_git(vault_root, &["commit", "-m", message, "-q"])?;
    let head = capture_git(vault_root, &["rev-parse", "HEAD"])?;
    Ok(head.trim().to_string())
}

fn run_git(cwd: &Path, args: &[&str]) -> io::Result<()> {
    let out = Command::new("git").current_dir(cwd).args(args).output()?;
    if !out.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ));
    }
    Ok(())
}

fn capture_git(cwd: &Path, args: &[&str]) -> io::Result<String> {
    let out = Command::new("git").current_dir(cwd).args(args).output()?;
    if !out.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("codebus-nested-{name}-{}-{}", std::process::id(), nanos()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn nanos() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos()
    }

    #[test]
    fn init_creates_dot_git_with_codebus_identity() {
        let v = tmp("init");
        init_nested_repo(&v).unwrap();
        assert!(v.join(".git").exists());
        let email = capture_git(&v, &["config", "--get", "user.email"]).unwrap();
        assert_eq!(email.trim(), "codebus@local");
        let name = capture_git(&v, &["config", "--get", "user.name"]).unwrap();
        assert_eq!(name.trim(), "codebus");
        let _ = fs::remove_dir_all(&v);
    }

    #[test]
    fn init_is_idempotent() {
        let v = tmp("idem");
        init_nested_repo(&v).unwrap();
        init_nested_repo(&v).expect("second init should be a no-op");
        let _ = fs::remove_dir_all(&v);
    }

    #[test]
    fn auto_commit_writes_changes() {
        let v = tmp("commit");
        init_nested_repo(&v).unwrap();
        fs::write(v.join("a.md"), "x").unwrap();
        let sha = auto_commit(&v, "first").unwrap();
        assert_eq!(sha.len(), 40);
        // working tree clean after commit
        let st = capture_git(&v, &["status", "--porcelain"]).unwrap();
        assert!(st.trim().is_empty());
        let _ = fs::remove_dir_all(&v);
    }

    #[test]
    fn auto_commit_returns_existing_head_when_clean() {
        let v = tmp("noop");
        init_nested_repo(&v).unwrap();
        fs::write(v.join("a.md"), "x").unwrap();
        let sha1 = auto_commit(&v, "first").unwrap();
        let sha2 = auto_commit(&v, "second-no-op").unwrap();
        // No changes to commit ⇒ HEAD unchanged.
        assert_eq!(sha1, sha2);
        let _ = fs::remove_dir_all(&v);
    }

    #[test]
    fn auto_commit_message_appears_in_log() {
        let v = tmp("msg");
        init_nested_repo(&v).unwrap();
        fs::write(v.join("a.md"), "x").unwrap();
        auto_commit(&v, "wiki: explore X").unwrap();
        let log = capture_git(&v, &["log", "--pretty=format:%s", "-1"]).unwrap();
        assert_eq!(log.trim(), "wiki: explore X");
        let _ = fs::remove_dir_all(&v);
    }
}
