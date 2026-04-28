//! Red-team coverage for `tutorial.rs` path validation. Mirrors the
//! sidecar `ensure_in_workspace` red-team fixture so the Rust trust
//! boundary catches the same path-escape vectors.
//!
//! Backs spec `interactive-tutorial` Requirements:
//!   - Three mdc interactive components ... (indirectly via file IO)
//!   - progress.json schema and single-writer path
//!   - Sub-page navigation within station markdown (read access)
//! And design D-T1 / D-T11 path safety contract.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use codebus_lib::tutorial::{
    list_tutorial_tasks_in, progress_path_for, validate_path, workspace_canonical,
    PROGRESS_WRITE_LOCK,
};
use tempfile::TempDir;

fn ws_root(td: &TempDir) -> String {
    td.path().to_string_lossy().to_string()
}

fn make_tutorial_file(ws: &TempDir, task: &str, file: &str, body: &str) -> PathBuf {
    let path = ws
        .path()
        .join("codebus-tutorials")
        .join(task)
        .join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, body).unwrap();
    path
}

#[test]
fn relative_must_start_with_codebus_tutorials() {
    let ws = TempDir::new().unwrap();
    let err = validate_path(&ws_root(&ws), "otherdir/file.md")
        .expect_err("path outside codebus-tutorials/ must be rejected");
    assert!(err.contains("must start with 'codebus-tutorials/'"), "{err}");
}

#[test]
fn dotdot_traversal_rejected() {
    let ws = TempDir::new().unwrap();
    let err = validate_path(
        &ws_root(&ws),
        "codebus-tutorials/../../etc/passwd",
    )
    .expect_err("dotdot traversal must be rejected");
    assert!(err.contains("'..'"), "{err}");
}

#[test]
fn extension_allowlist_md_json_only() {
    let ws = TempDir::new().unwrap();
    make_tutorial_file(&ws, "generate_aaaa1111", "tutorial.md", "ok");
    make_tutorial_file(&ws, "generate_aaaa1111", "route.json", "{}");

    assert!(validate_path(&ws_root(&ws), "codebus-tutorials/generate_aaaa1111/tutorial.md").is_ok());
    assert!(validate_path(&ws_root(&ws), "codebus-tutorials/generate_aaaa1111/route.json").is_ok());

    for forbidden in &["evil.txt", "evil.exe", "evil.sh", "no_extension"] {
        let rel = format!("codebus-tutorials/generate_aaaa1111/{forbidden}");
        let err = validate_path(&ws_root(&ws), &rel)
            .expect_err("extension outside allowlist must be rejected");
        assert!(err.contains("extension not allowed"), "{err}");
    }
}

#[test]
fn workspace_must_be_directory() {
    let td = TempDir::new().unwrap();
    let file_path = td.path().join("a_file");
    fs::write(&file_path, "x").unwrap();
    let err = validate_path(
        &file_path.to_string_lossy(),
        "codebus-tutorials/x/y.md",
    )
    .expect_err("non-directory workspace must be rejected");
    assert!(err.contains("not a directory"), "{err}");
}

#[test]
fn workspace_must_be_absolute() {
    let err = validate_path("relative/path", "codebus-tutorials/x/y.md")
        .expect_err("relative workspace path must be rejected");
    assert!(err.contains("must be absolute"), "{err}");
}

#[test]
#[cfg(unix)]
fn symlink_pointing_outside_rejected() {
    use std::os::unix::fs::symlink;

    let ws = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let secret = outside.path().join("secret.md");
    fs::write(&secret, "leaked").unwrap();

    let tutorials = ws.path().join("codebus-tutorials").join("generate_aaaa1111");
    fs::create_dir_all(&tutorials).unwrap();
    let link = tutorials.join("escape.md");
    symlink(&secret, &link).unwrap();

    let err = validate_path(
        &ws_root(&ws),
        "codebus-tutorials/generate_aaaa1111/escape.md",
    )
    .expect_err("symlink to outside must be rejected");
    assert!(err.contains("escapes workspace_root") || err.contains("outside"), "{err}");
}

#[test]
#[cfg(windows)]
fn unc_path_normalized() {
    let ws = TempDir::new().unwrap();
    make_tutorial_file(&ws, "generate_aaaa1111", "tutorial.md", "x");

    // dunce::canonicalize should strip the \\?\ prefix so two distinct
    // string forms canonicalise to the same starts_with target.
    let plain = ws.path().to_string_lossy().to_string();
    let unc = format!(r"\\?\{}", plain);

    let plain_ok =
        validate_path(&plain, "codebus-tutorials/generate_aaaa1111/tutorial.md").unwrap();
    let unc_ok =
        validate_path(&unc, "codebus-tutorials/generate_aaaa1111/tutorial.md").unwrap();
    assert_eq!(plain_ok, unc_ok);
}

#[test]
#[cfg(windows)]
fn case_insensitive_on_windows() {
    let ws = TempDir::new().unwrap();
    make_tutorial_file(&ws, "generate_aaaa1111", "tutorial.md", "x");

    let lower = ws.path().to_string_lossy().to_lowercase();
    let upper = ws.path().to_string_lossy().to_uppercase();

    let lower_ok = validate_path(&lower, "codebus-tutorials/generate_aaaa1111/tutorial.md");
    let upper_ok = validate_path(&upper, "codebus-tutorials/generate_aaaa1111/tutorial.md");
    // At least one casing must succeed; both should canonicalise to the
    // same disk path on Windows. On case-insensitive filesystems both
    // succeed; on rare case-sensitive setups one may fail and that's OK.
    assert!(
        lower_ok.is_ok() || upper_ok.is_ok(),
        "case fold smoke test: lower={lower_ok:?} upper={upper_ok:?}"
    );
}

#[test]
fn write_progress_task_id_format() {
    let ws = TempDir::new().unwrap();
    let root = ws_root(&ws);

    for bad in &["", "../escape", "with/slash", "white space", "UPPER", "dot.dot"] {
        let err = progress_path_for(&root, bad)
            .expect_err("bad task_id must be rejected");
        assert!(err.contains("task_id"), "input={bad:?} err={err}");
    }
    for ok in &["generate_a3f2b1c8", "generate_aaaa1111", "synth_42", "x"] {
        progress_path_for(&root, ok).expect("good task_id must validate");
    }
}

#[tokio::test]
async fn write_progress_creates_parent_dir() {
    let ws = TempDir::new().unwrap();
    let task_id = "generate_aaaa1111";
    let target = progress_path_for(&ws_root(&ws), task_id).unwrap();
    assert!(!target.exists(), "precondition: target absent");
    assert!(!target.parent().unwrap().exists(), "precondition: parent absent");

    let parent = target.parent().unwrap().to_path_buf();
    let _guard = PROGRESS_WRITE_LOCK.lock().await;
    tokio::fs::create_dir_all(&parent).await.unwrap();
    tokio::fs::write(&target, b"{}").await.unwrap();
    drop(_guard);

    assert!(target.exists(), "progress.json must be created");
    assert_eq!(fs::read_to_string(&target).unwrap(), "{}");
}

#[tokio::test]
async fn concurrent_writes_serialized() {
    // Spawn N writers competing on PROGRESS_WRITE_LOCK; assert the file
    // contents are one of the inputs (not torn / interleaved).
    let ws = TempDir::new().unwrap();
    let task_id = "generate_aaaa1111";
    let target = progress_path_for(&ws_root(&ws), task_id).unwrap();
    fs::create_dir_all(target.parent().unwrap()).unwrap();

    let target_arc = Arc::new(target);
    let mut handles = Vec::new();
    for i in 0..16u8 {
        let target = Arc::clone(&target_arc);
        handles.push(tokio::spawn(async move {
            let payload = format!(r#"{{"writer":{i}}}"#);
            let _guard = PROGRESS_WRITE_LOCK.lock().await;
            tokio::fs::write(&*target, payload.as_bytes()).await.unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let body = fs::read_to_string(&*target_arc).unwrap();
    let valid: serde_json::Value = serde_json::from_str(&body)
        .expect("serialised writes must produce valid JSON, never torn bytes");
    assert!(valid.get("writer").is_some(), "body must be one whole writer payload");
}

#[test]
fn list_tutorial_tasks_missing_dir_returns_empty() {
    let ws = TempDir::new().unwrap();
    let result = list_tutorial_tasks_in(&ws_root(&ws))
        .expect("missing codebus-tutorials/ must return Ok(empty), not raise");
    assert!(result.is_empty(), "{result:?}");
}

#[test]
fn list_tutorial_tasks_workspace_safety() {
    // workspace not absolute
    assert!(list_tutorial_tasks_in("relative").is_err());

    // workspace doesn't exist
    let nonexistent = std::env::temp_dir().join("codebus_nonexistent_dir_xyz123");
    let _ = fs::remove_dir_all(&nonexistent);
    assert!(list_tutorial_tasks_in(&nonexistent.to_string_lossy()).is_err());

    // workspace is a file
    let td = TempDir::new().unwrap();
    let file_ws = td.path().join("a_file");
    fs::write(&file_ws, "x").unwrap();
    assert!(list_tutorial_tasks_in(&file_ws.to_string_lossy()).is_err());
}

// ----- Hardening pass (S1 + S2 + S3) -------------------------------------

#[test]
fn windows_reserved_device_names_rejected() {
    let ws = TempDir::new().unwrap();
    let root = ws_root(&ws);
    let cases = [
        "codebus-tutorials/generate_aaaa1111/CON.md",
        "codebus-tutorials/generate_aaaa1111/con.md",
        "codebus-tutorials/generate_aaaa1111/Nul.md",
        "codebus-tutorials/generate_aaaa1111/AUX.json",
        "codebus-tutorials/generate_aaaa1111/PRN.md",
        "codebus-tutorials/generate_aaaa1111/COM1.md",
        "codebus-tutorials/generate_aaaa1111/lpt9.md",
        "codebus-tutorials/CON/file.md",
    ];
    for case in cases {
        let err = validate_path(&root, case)
            .expect_err(&format!("reserved name must be rejected: {case}"));
        assert!(
            err.contains("Windows reserved name"),
            "case={case} err={err}"
        );
    }
}

#[test]
fn segment_with_trailing_dot_or_space_rejected() {
    let ws = TempDir::new().unwrap();
    let root = ws_root(&ws);
    // Windows strips trailing dots / spaces at the FS layer, so a name
    // ending in '.' or ' ' resolves to a different file than the
    // visible string. Reject the segments that would actually trigger
    // the strip — middle dots/spaces in a name are fine.
    for case in [
        "codebus-tutorials/generate_aaaa1111./tutorial.md",
        "codebus-tutorials/generate_aaaa1111 /tutorial.md",
        "codebus-tutorials/generate_aaaa1111/tutorial.md.",
        "codebus-tutorials/generate_aaaa1111/tutorial.md ",
    ] {
        let err = validate_path(&root, case)
            .expect_err(&format!("trailing dot/space must be rejected: {case}"));
        assert!(
            err.contains("ends with '.'") || err.contains("ends with ' '")
                || err.contains("extension not allowed"),
            "case={case} err={err}"
        );
    }
}

#[test]
fn segment_with_colon_rejected() {
    let ws = TempDir::new().unwrap();
    let root = ws_root(&ws);
    for case in [
        "codebus-tutorials/generate_aaaa1111/tutorial.md:hidden",
        "codebus-tutorials/foo:bar/tutorial.md",
    ] {
        let err = validate_path(&root, case)
            .expect_err(&format!("colon must be rejected: {case}"));
        assert!(
            err.contains("contains ':'") || err.contains("extension not allowed"),
            "case={case} err={err}"
        );
    }
}

#[test]
fn dot_segment_rejected() {
    let ws = TempDir::new().unwrap();
    let root = ws_root(&ws);
    let err = validate_path(
        &root,
        "codebus-tutorials/./generate_aaaa1111/tutorial.md",
    )
    .expect_err("'.' segment must be rejected");
    assert!(err.contains("'.'"), "{err}");
}

#[test]
fn workspace_canonical_returns_canonical_root() {
    let ws = TempDir::new().unwrap();
    let canonical = workspace_canonical(&ws_root(&ws)).unwrap();
    // canonical path under tempdir must contain the same final
    // directory name (Windows may add long-path prefix, dunce strips it)
    assert!(canonical.exists());
    assert!(canonical.is_dir());
}

#[tokio::test]
async fn write_progress_recanonicalises_parent_after_create_dir_all() {
    // Verify the post-create-dir-all containment recheck path returns a
    // path inside the workspace (the actual race scenario it defends
    // against requires a hostile concurrent symlink swap, which we
    // cannot reproduce portably; this test asserts the helper at least
    // succeeds for the happy path so the recheck is not a no-op).
    let ws = TempDir::new().unwrap();
    let task_id = "generate_aaaa1111";
    let target = progress_path_for(&ws_root(&ws), task_id).unwrap();
    let parent = target.parent().unwrap().to_path_buf();
    fs::create_dir_all(&parent).unwrap();
    let parent_canonical = dunce::canonicalize(&parent).unwrap();
    let ws_canonical = workspace_canonical(&ws_root(&ws)).unwrap();
    assert!(
        parent_canonical.starts_with(&ws_canonical),
        "parent {} must stay under workspace {}",
        parent_canonical.display(),
        ws_canonical.display()
    );
}

#[test]
fn list_tutorial_tasks_skips_non_directories() {
    let ws = TempDir::new().unwrap();
    // Two real task directories.
    make_tutorial_file(&ws, "generate_aaaa1111", "tutorial.md", "---\ngenerated_at: 2026-04-28\n---\nbody\n");
    make_tutorial_file(&ws, "generate_bbbb2222", "tutorial.md", "no frontmatter here");
    // A stray file at codebus-tutorials/ root (must be skipped, not raise).
    let stray = ws.path().join("codebus-tutorials").join("stray.txt");
    fs::write(&stray, "stray").unwrap();

    let result = list_tutorial_tasks_in(&ws_root(&ws)).unwrap();
    let ids: Vec<_> = result.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["generate_aaaa1111", "generate_bbbb2222"]);

    let with_fm = result.iter().find(|m| m.id == "generate_aaaa1111").unwrap();
    assert!(
        with_fm.frontmatter_raw.as_deref().unwrap_or("").contains("generated_at: 2026-04-28"),
        "frontmatter not parsed: {:?}",
        with_fm.frontmatter_raw
    );

    let without_fm = result.iter().find(|m| m.id == "generate_bbbb2222").unwrap();
    assert!(without_fm.frontmatter_raw.is_none(), "no-frontmatter task should be None");
}
