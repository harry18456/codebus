//! Defensive parity test: the Rust `AUDIT_KIND_TO_FILENAME` constant
//! and the Python `_<NAME>_FILENAME` constants in
//! `sidecar/src/codebus_agent/_audit_paths.py` MUST agree on the seven
//! canonical filenames. Any drift fails this test with a message
//! naming both sides — preventing silent renames that would break the
//! audit IPC at runtime.
//!
//! Backs `llm-call-inspector` capability:
//!   - Requirement: Audit kind filename mapping defensive parity

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use codebus_lib::audit_files::AUDIT_KIND_TO_FILENAME;

/// Locate `sidecar/src/codebus_agent/_audit_paths.py` from the repo root.
/// Tauri tests run with CWD at `tauri/src-tauri/`, so go up two levels.
fn audit_paths_py() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR"); // tauri/src-tauri
    PathBuf::from(manifest_dir)
        .join("..")
        .join("..")
        .join("sidecar")
        .join("src")
        .join("codebus_agent")
        .join("_audit_paths.py")
}

/// Grep `_<NAME>_FILENAME = "..."` declarations from the Python module.
fn parse_python_constants() -> BTreeMap<String, String> {
    let path = audit_paths_py();
    let content = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {} (test runs from tauri/src-tauri/, expected sibling sidecar/): {e}",
            path.display()
        )
    });
    let mut map = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with('_') || !line.contains("_FILENAME") {
            continue;
        }
        // Match `_FOO_FILENAME = "value"` (allow trailing comment).
        let Some((lhs, rhs)) = line.split_once('=') else {
            continue;
        };
        let lhs = lhs.trim();
        if !lhs.starts_with('_') || !lhs.ends_with("_FILENAME") {
            continue;
        }
        let rhs = rhs.trim();
        // Strip trailing inline comment if present.
        let rhs = rhs.split('#').next().unwrap_or(rhs).trim();
        // Expect a quoted string literal.
        let value = rhs
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or_else(|| {
                panic!("unexpected RHS shape on line {line:?} (expected double-quoted string)")
            });
        // Convert `_LLM_CALLS_FILENAME` → audit_kind enum key by stripping
        // the suffix and comparing canonical forms below.
        map.insert(lhs.to_string(), value.to_string());
    }
    map
}

/// Mapping from Python `_<NAME>_FILENAME` constant key → expected
/// audit_kind enum value used in the Rust `AUDIT_KIND_TO_FILENAME`
/// table. This is the only place where we explicitly translate
/// between the two naming schemes; both sides' values (the right-hand
/// strings) MUST match.
fn python_const_to_audit_kind(constant: &str) -> Option<&'static str> {
    match constant {
        "_SANITIZE_AUDIT_FILENAME" => Some("sanitize"),
        "_TOOL_AUDIT_FILENAME" => Some("tool"),
        "_REASONING_LOG_FILENAME" => Some("reasoning"),
        "_TOKEN_USAGE_FILENAME" => Some("token"),
        "_LLM_CALLS_FILENAME" => Some("llm"),
        "_KB_GROWTH_FILENAME" => Some("kb_growth"),
        "_GENERATOR_LOG_FILENAME" => Some("generator"),
        _ => None,
    }
}

#[test]
fn rust_table_has_seven_entries_in_canonical_order() {
    assert_eq!(AUDIT_KIND_TO_FILENAME.len(), 7);
    let kinds: Vec<&str> = AUDIT_KIND_TO_FILENAME.iter().map(|(k, _)| *k).collect();
    assert_eq!(
        kinds,
        vec![
            "sanitize",
            "tool",
            "reasoning",
            "token",
            "llm",
            "kb_growth",
            "generator"
        ]
    );
}

#[test]
fn rust_table_filenames_match_python_constants() {
    let py = parse_python_constants();
    assert!(
        py.len() >= 7,
        "expected ≥ 7 _<NAME>_FILENAME constants in Python, got {} ({:?})",
        py.len(),
        py.keys().collect::<Vec<_>>()
    );

    // For every Python constant we recognise, assert the Rust table
    // has the same audit_kind → filename pair.
    let rust_map: BTreeMap<&str, &str> = AUDIT_KIND_TO_FILENAME.iter().copied().collect();

    for (py_const, py_filename) in &py {
        let Some(audit_kind) = python_const_to_audit_kind(py_const) else {
            continue;
        };
        let rust_filename = rust_map.get(audit_kind).unwrap_or_else(|| {
            panic!(
                "Rust AUDIT_KIND_TO_FILENAME missing key {audit_kind:?} \
                 (Python side declares {py_const} = {py_filename:?}). \
                 Update tauri/src-tauri/src/audit_files.rs::AUDIT_KIND_TO_FILENAME."
            )
        });
        assert_eq!(
            rust_filename, py_filename,
            "DRIFT: Rust audit_kind {audit_kind:?} → {rust_filename:?} but \
             Python {py_const} = {py_filename:?}. \
             Reconcile tauri/src-tauri/src/audit_files.rs::AUDIT_KIND_TO_FILENAME \
             with sidecar/src/codebus_agent/_audit_paths.py."
        );
    }

    // Also verify each Rust kind is backed by a Python constant.
    let py_filenames: std::collections::HashSet<&str> =
        py.values().map(|s| s.as_str()).collect();
    for (kind, rust_filename) in AUDIT_KIND_TO_FILENAME {
        assert!(
            py_filenames.contains(rust_filename),
            "Rust kind {kind:?} → {rust_filename:?} has no matching Python \
             _<NAME>_FILENAME constant. Add it to \
             sidecar/src/codebus_agent/_audit_paths.py or remove from Rust."
        );
    }
}
