//! Backs SHALL clauses in
//! openspec/changes/m1-power-on/specs/tauri-shell/spec.md
//!   Requirement: Tauri spawns sidecar and completes handshake
//!
//! The `invoke('sidecar_ping')` command spawns the sidecar binary, reads
//! the first stdout line, and expects a JSON handshake of shape:
//!   {"port": <u16>, "bearer": "<string ≥ 32 chars>"}
//!
//! Parsing must reject malformed / short-bearer / non-u16-port variants so
//! a corrupt sidecar start cannot silently downgrade auth strength.

use codebus_lib::sidecar::{parse_handshake, HandshakeError};

#[test]
fn parses_valid_handshake() {
    let line = r#"{"port": 54321, "bearer": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#;
    let hs = parse_handshake(line).expect("valid handshake");
    assert_eq!(hs.port, 54321);
    assert_eq!(hs.bearer.len(), 32);
}

#[test]
fn rejects_invalid_json() {
    let err = parse_handshake("not json").unwrap_err();
    assert!(matches!(err, HandshakeError::InvalidJson(_)), "got: {err:?}");
}

#[test]
fn rejects_missing_port() {
    let line = r#"{"bearer": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#;
    let err = parse_handshake(line).unwrap_err();
    assert!(matches!(err, HandshakeError::MissingField(_)), "got: {err:?}");
}

#[test]
fn rejects_missing_bearer() {
    let line = r#"{"port": 54321}"#;
    let err = parse_handshake(line).unwrap_err();
    assert!(matches!(err, HandshakeError::MissingField(_)), "got: {err:?}");
}

#[test]
fn rejects_short_bearer() {
    let line = r#"{"port": 54321, "bearer": "short"}"#;
    let err = parse_handshake(line).unwrap_err();
    assert!(matches!(err, HandshakeError::BearerTooShort { .. }), "got: {err:?}");
}

#[test]
fn rejects_out_of_range_port() {
    let line = r#"{"port": 99999, "bearer": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#;
    let err = parse_handshake(line).unwrap_err();
    assert!(matches!(err, HandshakeError::InvalidPort { .. }), "got: {err:?}");
}

#[test]
fn rejects_zero_port() {
    let line = r#"{"port": 0, "bearer": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#;
    let err = parse_handshake(line).unwrap_err();
    assert!(matches!(err, HandshakeError::InvalidPort { .. }), "got: {err:?}");
}
