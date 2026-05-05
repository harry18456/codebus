use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct LockHandle {
    pub path: PathBuf,
    pub released: bool,
}

#[derive(Debug)]
pub enum LockError {
    AlreadyHeld(PathBuf),
    Io(io::Error),
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::AlreadyHeld(p) => write!(f, "Lock already held at {}", p.display()),
            LockError::Io(e) => write!(f, "lock io error: {e}"),
        }
    }
}

impl std::error::Error for LockError {}

impl From<io::Error> for LockError {
    fn from(e: io::Error) -> Self {
        LockError::Io(e)
    }
}

/// Acquire an exclusive file lock by atomically creating `lock_path`.
/// Mirrors TS `writeFile(path, pid, { flag: 'wx' })` semantics: if the
/// file already exists the call fails with [`LockError::AlreadyHeld`].
pub fn acquire_lock(lock_path: impl AsRef<Path>) -> Result<LockHandle, LockError> {
    let path = lock_path.as_ref().to_path_buf();
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(LockError::AlreadyHeld(path));
        }
        Err(e) => return Err(LockError::Io(e)),
    };
    let pid = std::process::id().to_string();
    file.write_all(pid.as_bytes())?;
    let _ = File::sync_all(&file);
    Ok(LockHandle {
        path,
        released: false,
    })
}

/// Release a previously acquired lock. Idempotent — safe to call twice
/// or on a handle whose underlying file was already removed externally
/// (returns Ok in both cases).
pub fn release_lock(handle: &mut LockHandle) -> Result<(), LockError> {
    if handle.released {
        return Ok(());
    }
    match std::fs::remove_file(&handle.path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(LockError::Io(e)),
    }
    handle.released = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_lock(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("codebus-lock-{name}-{}", std::process::id()));
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
        fs::create_dir_all(&dir).unwrap();
        dir.join(".lock")
    }

    #[test]
    fn acquire_then_release_roundtrip() {
        let p = tmp_lock("rt");
        let mut h = acquire_lock(&p).expect("first acquire should succeed");
        assert!(p.exists());
        release_lock(&mut h).unwrap();
        assert!(!p.exists());
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn second_acquire_fails_with_already_held() {
        let p = tmp_lock("contend");
        let mut h1 = acquire_lock(&p).unwrap();
        match acquire_lock(&p) {
            Err(LockError::AlreadyHeld(path)) => {
                assert_eq!(path, p);
            }
            other => panic!("expected AlreadyHeld, got {other:?}"),
        }
        release_lock(&mut h1).unwrap();
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn release_is_idempotent() {
        let p = tmp_lock("idem");
        let mut h = acquire_lock(&p).unwrap();
        release_lock(&mut h).unwrap();
        release_lock(&mut h).unwrap(); // second release is a no-op
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn release_tolerates_externally_deleted_lock_file() {
        let p = tmp_lock("ext");
        let mut h = acquire_lock(&p).unwrap();
        fs::remove_file(&p).unwrap();
        release_lock(&mut h).expect("release should not error when file already gone");
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn pid_written_to_lock_file() {
        let p = tmp_lock("pid");
        let mut h = acquire_lock(&p).unwrap();
        let contents = fs::read_to_string(&p).unwrap();
        let pid = contents.parse::<u32>().expect("file should contain pid");
        assert_eq!(pid, std::process::id());
        release_lock(&mut h).unwrap();
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }
}
