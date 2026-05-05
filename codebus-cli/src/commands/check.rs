use codebus_core::vault::layout::vault_paths;
use codebus_core::wiki::lint::lint_wiki;
use codebus_core::wiki::types::LintResult;
use std::io;
use std::path::Path;

pub fn run_check(repo_root: impl AsRef<Path>) -> io::Result<LintResult> {
    let repo_root = repo_root.as_ref();
    let p = vault_paths(repo_root);
    if !p.root.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "No codebus vault at {} — run `codebus --repo {}` first to init, or `codebus --repo {} --goal \"...\"` to ingest",
                p.root.display(),
                repo_root.display(),
                repo_root.display()
            ),
        ));
    }
    Ok(lint_wiki(&p.root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn nanos() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "codebus-checkcmd-{name}-{}-{}",
            std::process::id(),
            nanos()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn check_errors_when_vault_missing() {
        let repo = tmp("novault");
        let r = run_check(&repo);
        assert!(r.is_err());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn check_returns_lint_result_when_vault_exists() {
        let repo = tmp("hasvault");
        fs::create_dir_all(repo.join(".codebus/wiki/concepts")).unwrap();
        fs::create_dir_all(repo.join(".codebus/wiki/entities")).unwrap();
        fs::create_dir_all(repo.join(".codebus/wiki/modules")).unwrap();
        fs::create_dir_all(repo.join(".codebus/wiki/processes")).unwrap();
        fs::create_dir_all(repo.join(".codebus/wiki/synthesis")).unwrap();
        fs::write(repo.join(".codebus/wiki/index.md"), "x").unwrap();
        fs::write(repo.join(".codebus/wiki/log.md"), "x").unwrap();
        let r = run_check(&repo).unwrap();
        assert_eq!(r.error_count, 0);
        let _ = fs::remove_dir_all(&repo);
    }

    /// Phase C conformance: replay TS-recorded `--check` output against
    /// the uv vault snapshot and verify Rust-side warnings cover the
    /// same categories. The exact line-by-line byte equality is more
    /// brittle than productive (issue ordering, message wording subject
    /// to fixture regen) — we instead assert structural parity:
    /// 0 errors, ≥5 warnings including the 5 known categories from the
    /// TS baseline (1 root-page + 4 broken body wikilinks).
    #[test]
    fn check_against_uv_vault_fixture_matches_ts_baseline_categories() {
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/uv-vault-snapshot/uv-wiki-snapshot");
        // Wrap fixture as `<tmp>/.codebus/wiki/...`.
        let stage = tmp("uvconform");
        let codebus = stage.join(".codebus");
        fs::create_dir_all(codebus.join("wiki")).unwrap();
        copy_dir(&fixture_root, &codebus.join("wiki")).unwrap();

        let r = run_check(&stage).unwrap();
        assert_eq!(
            r.error_count, 0,
            "TS baseline reports 0 errors; got {}",
            r.error_count
        );
        assert!(
            r.warn_count >= 5,
            "TS baseline reports 5 warnings; got {}: {:#?}",
            r.warn_count,
            r.issues
        );
        assert_eq!(
            r.pages_scanned, 14,
            "uv fixture has 14 knowledge pages, lint scanned {}",
            r.pages_scanned
        );
        assert_eq!(
            r.nav_files_scanned, 2,
            "uv fixture has index + log nav files, lint scanned {}",
            r.nav_files_scanned
        );

        let root_warn = r
            .issues
            .iter()
            .filter(|i| i.message.contains("page lives in wiki/ root"))
            .count();
        assert_eq!(
            root_warn, 1,
            "expected 1 root-page warning (overview.md), got {root_warn}"
        );

        let broken_body = r
            .issues
            .iter()
            .filter(|i| i.message.contains("broken wikilink in body"))
            .count();
        assert!(
            broken_body >= 4,
            "expected ≥4 broken body wikilinks, got {broken_body}"
        );

        let _ = fs::remove_dir_all(&stage);
    }

    fn copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            let dst_path = dst.join(entry.file_name());
            if ft.is_dir() {
                copy_dir(&entry.path(), &dst_path)?;
            } else {
                fs::copy(entry.path(), dst_path)?;
            }
        }
        Ok(())
    }
}
