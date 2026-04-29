//! Red-team coverage for `audit_files::validate_audit_path` +
//! `read_audit_jsonl`. Mirrors `path_safety.rs` (tutorial side) so the
//! audit IPC catches the same path-escape vectors and adds audit-only
//! checks (audit_kind enum gate, 5 MiB cap, JSONL parse-skip).
//!
//! Backs `llm-call-inspector` capability:
//!   - Requirement: `read_audit_jsonl` Tauri command exposes seven
//!     workspace audit JSONLs by enum

use std::fs;
use std::path::PathBuf;

use codebus_lib::audit_files::{
    read_audit_jsonl_inner, validate_audit_path, AUDIT_KIND_TO_FILENAME,
};
use tempfile::TempDir;

fn ws_root(td: &TempDir) -> String {
    td.path().to_string_lossy().to_string()
}

fn make_audit_file(ws: &TempDir, filename: &str, body: &str) -> PathBuf {
    let path = ws.path().join(".codebus").join(filename);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, body).unwrap();
    path
}

#[tokio::test]
async fn valid_llm_kind_returns_three_entries() {
    let ws = TempDir::new().unwrap();
    make_audit_file(
        &ws,
        "llm_calls.jsonl",
        "{\"timestamp\":\"a\",\"role\":\"chat\",\"model\":\"m\"}\n\
         {\"timestamp\":\"b\",\"role\":\"judge\",\"model\":\"m\"}\n\
         {\"timestamp\":\"c\",\"role\":\"reasoning\",\"model\":\"m\"}\n",
    );
    let entries = read_audit_jsonl_inner(&ws_root(&ws), "llm")
        .await
        .expect("valid llm kind must succeed");
    assert_eq!(entries.len(), 3);
    for entry in &entries {
        assert!(entry.get("timestamp").is_some());
        assert!(entry.get("role").is_some());
        assert!(entry.get("model").is_some());
    }
}

#[tokio::test]
async fn unknown_audit_kind_rejected_without_filesystem_access() {
    let ws = TempDir::new().unwrap();
    // Place a real `secrets.jsonl` to prove the rejection happens
    // before any file resolution: the file must NOT be read.
    make_audit_file(&ws, "secrets.jsonl", "{\"leak\":\"value\"}\n");

    let err = read_audit_jsonl_inner(&ws_root(&ws), "secrets")
        .await
        .expect_err("unknown audit_kind must be rejected");
    assert_eq!(err, "E_AUDIT_KIND_INVALID");
}

#[tokio::test]
async fn missing_audit_file_returns_empty_vec() {
    let ws = TempDir::new().unwrap();
    // No `.codebus/llm_calls.jsonl` on disk.
    let entries = read_audit_jsonl_inner(&ws_root(&ws), "llm")
        .await
        .expect("missing file must return Ok(vec![])");
    assert!(entries.is_empty());
}

#[tokio::test]
async fn workspace_must_be_absolute() {
    let err = validate_audit_path("relative/path", "llm")
        .expect_err("relative ws_root must be rejected");
    assert_eq!(err, "E_WORKSPACE_INVALID");
}

#[tokio::test]
async fn workspace_must_exist() {
    let err = validate_audit_path("/non/existent/path", "llm")
        .expect_err("non-existent ws_root must be rejected");
    assert_eq!(err, "E_WORKSPACE_INVALID");
}

#[tokio::test]
async fn workspace_must_be_directory() {
    let td = TempDir::new().unwrap();
    let file_path = td.path().join("a_file");
    fs::write(&file_path, "x").unwrap();
    let err = validate_audit_path(&file_path.to_string_lossy(), "llm")
        .expect_err("non-directory ws_root must be rejected");
    assert_eq!(err, "E_WORKSPACE_INVALID");
}

#[tokio::test]
async fn file_over_5_mib_rejected_pre_parse() {
    let ws = TempDir::new().unwrap();
    let big_line = format!("{{\"x\":\"{}\"}}\n", "a".repeat(1024));
    let mut content = String::new();
    while content.len() <= 5 * 1024 * 1024 {
        content.push_str(&big_line);
    }
    make_audit_file(&ws, "llm_calls.jsonl", &content);

    let err = read_audit_jsonl_inner(&ws_root(&ws), "llm")
        .await
        .expect_err("file > 5 MiB must be rejected");
    assert_eq!(err, "E_AUDIT_TOO_LARGE");
}

#[tokio::test]
async fn corrupt_line_skipped_without_poisoning_result() {
    let ws = TempDir::new().unwrap();
    make_audit_file(
        &ws,
        "llm_calls.jsonl",
        "{\"timestamp\":\"a\",\"role\":\"chat\"}\n\
         {not valid json\n\
         {\"timestamp\":\"b\",\"role\":\"judge\"}\n",
    );
    let entries = read_audit_jsonl_inner(&ws_root(&ws), "llm")
        .await
        .expect("corrupt line must NOT poison result");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get("timestamp").unwrap(), "a");
    assert_eq!(entries[1].get("timestamp").unwrap(), "b");
}

#[tokio::test]
async fn empty_lines_in_file_skipped() {
    let ws = TempDir::new().unwrap();
    make_audit_file(
        &ws,
        "llm_calls.jsonl",
        "\n{\"a\":1}\n\n\n{\"b\":2}\n\n",
    );
    let entries = read_audit_jsonl_inner(&ws_root(&ws), "llm")
        .await
        .unwrap();
    assert_eq!(entries.len(), 2);
}

#[tokio::test]
async fn all_seven_kinds_resolve_to_canonical_filenames() {
    let ws = TempDir::new().unwrap();
    for (kind, filename) in AUDIT_KIND_TO_FILENAME {
        make_audit_file(&ws, filename, &format!("{{\"k\":\"{kind}\"}}\n"));
        let entries = read_audit_jsonl_inner(&ws_root(&ws), kind)
            .await
            .unwrap_or_else(|e| panic!("kind {kind} must resolve: {e}"));
        assert_eq!(entries.len(), 1, "kind {kind} returned wrong count");
        assert_eq!(entries[0].get("k").unwrap(), kind);
    }
}

#[tokio::test]
#[cfg(unix)]
async fn symlink_pointing_outside_rejected() {
    use std::os::unix::fs::symlink;

    let ws = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let secret = outside.path().join("secret.jsonl");
    fs::write(&secret, "{\"leaked\":1}\n").unwrap();

    let codebus = ws.path().join(".codebus");
    fs::create_dir_all(&codebus).unwrap();
    let link = codebus.join("llm_calls.jsonl");
    symlink(&secret, &link).unwrap();

    let err = read_audit_jsonl_inner(&ws_root(&ws), "llm")
        .await
        .expect_err("symlink to outside ws must be rejected");
    assert_eq!(err, "E_DENIED");
}

#[tokio::test]
#[cfg(windows)]
async fn unc_path_normalized() {
    let ws = TempDir::new().unwrap();
    make_audit_file(&ws, "llm_calls.jsonl", "{\"x\":1}\n");

    let plain = ws.path().to_string_lossy().to_string();
    let unc = format!(r"\\?\{}", plain);

    let plain_ok = validate_audit_path(&plain, "llm").unwrap();
    let unc_ok = validate_audit_path(&unc, "llm").unwrap();
    assert_eq!(plain_ok, unc_ok);
}

#[tokio::test]
async fn directory_in_place_of_audit_file_rejected() {
    let ws = TempDir::new().unwrap();
    let codebus = ws.path().join(".codebus");
    fs::create_dir_all(&codebus).unwrap();
    // Make `.codebus/llm_calls.jsonl` itself a directory.
    fs::create_dir_all(codebus.join("llm_calls.jsonl")).unwrap();

    let err = read_audit_jsonl_inner(&ws_root(&ws), "llm")
        .await
        .expect_err("non-regular file must be rejected");
    assert_eq!(err, "E_NOT_REGULAR_FILE");
}
