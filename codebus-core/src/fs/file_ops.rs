use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// SHA-256 hex digest of a file's bytes. Streams in 64 KiB chunks so very
/// large files (raw-sync caps at 5 MiB but `sha256_file` is reused for
/// arbitrary paths in lint and stale-detect) don't pin RAM.
pub fn sha256_file(path: impl AsRef<Path>) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    Ok(encode_hex(&digest))
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Recursively list relative file paths under `root`. Forward-slash
/// separators on output for cross-platform stable comparison.
pub fn list_files_recursive(root: impl AsRef<Path>) -> io::Result<Vec<String>> {
    let root = root.as_ref();
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            walk(root, &path, out)?;
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let s = rel.to_string_lossy().replace('\\', "/");
            out.push(s);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("codebus-fileops-{name}-{}-{}", std::process::id(), nanos()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn nanos() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos()
    }

    #[test]
    fn sha256_of_known_input() {
        let dir = tmp("known");
        let p = dir.join("hello.txt");
        fs::write(&p, "hello").unwrap();
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        assert_eq!(
            sha256_file(&p).unwrap(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha256_of_empty_file() {
        let dir = tmp("empty");
        let p = dir.join("empty.txt");
        fs::write(&p, "").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_files_recursive_yields_forward_slash_paths() {
        let dir = tmp("list");
        fs::create_dir_all(dir.join("a/b")).unwrap();
        fs::write(dir.join("a/b/c.txt"), "x").unwrap();
        fs::write(dir.join("d.txt"), "y").unwrap();
        let mut files = list_files_recursive(&dir).unwrap();
        files.sort();
        assert_eq!(files, vec!["a/b/c.txt".to_string(), "d.txt".to_string()]);
        let _ = fs::remove_dir_all(&dir);
    }
}
