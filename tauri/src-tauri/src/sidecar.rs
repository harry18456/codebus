//! Sidecar spawn + handshake + health-check.  Backs SHALL clauses in
//! openspec/changes/m1-power-on/specs/tauri-shell/spec.md
//!   Requirement: Tauri spawns sidecar and completes handshake
//!   Requirement: sidecar_ping command returns /healthz result

use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct Handshake {
    pub port: u16,
    pub bearer: String,
}

#[derive(Debug, thiserror::Error)]
pub enum HandshakeError {
    #[error("invalid handshake JSON: {0}")]
    InvalidJson(String),
    #[error("handshake missing required field: {0}")]
    MissingField(&'static str),
    #[error("port must be in 1..=65535, got {got}")]
    InvalidPort { got: i64 },
    #[error("bearer must be ≥32 chars, got {got}")]
    BearerTooShort { got: usize },
}

const MIN_BEARER_LEN: usize = 32;

pub fn parse_handshake(line: &str) -> Result<Handshake, HandshakeError> {
    let raw: serde_json::Value = serde_json::from_str(line.trim())
        .map_err(|e| HandshakeError::InvalidJson(e.to_string()))?;

    let port_num = raw
        .get("port")
        .and_then(|v| v.as_i64())
        .ok_or(HandshakeError::MissingField("port"))?;
    if !(1..=65535).contains(&port_num) {
        return Err(HandshakeError::InvalidPort { got: port_num });
    }
    let port = port_num as u16;

    let bearer = raw
        .get("bearer")
        .and_then(|v| v.as_str())
        .ok_or(HandshakeError::MissingField("bearer"))?
        .to_string();
    if bearer.len() < MIN_BEARER_LEN {
        return Err(HandshakeError::BearerTooShort { got: bearer.len() });
    }

    Ok(Handshake { port, bearer })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResult {
    pub status: String,
    pub port: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum PingError {
    #[error("failed to spawn sidecar: {0}")]
    Spawn(String),
    #[error("sidecar stdout closed before handshake")]
    HandshakeClosed,
    #[error(transparent)]
    Handshake(#[from] HandshakeError),
    #[error("healthz request failed: {0}")]
    HealthRequest(String),
    #[error("healthz returned non-2xx: {0}")]
    HealthStatus(u16),
}

/// Spawn the sidecar binary, read the first stdout line as a handshake,
/// then call `GET /healthz` with the bearer and return the parsed result.
///
/// `sidecar_path` must point at the packaged binary (Phase 8 artifact).
/// In M1 dev runs it is the `codebus-sidecar` shim on PATH.
pub async fn sidecar_ping(sidecar_path: &str) -> Result<PingResult, PingError> {
    use std::io::{BufRead, BufReader};

    let parent_pid = std::process::id().to_string();
    let mut child = std::process::Command::new(sidecar_path)
        .arg("--parent-pid")
        .arg(&parent_pid)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| PingError::Spawn(e.to_string()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| PingError::Spawn("stdout not piped".into()))?;
    let mut reader = BufReader::new(stdout);
    let mut first_line = String::new();
    let n = reader
        .read_line(&mut first_line)
        .map_err(|e| PingError::Spawn(e.to_string()))?;
    if n == 0 {
        return Err(PingError::HandshakeClosed);
    }
    let hs = parse_handshake(&first_line)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| PingError::HealthRequest(e.to_string()))?;
    let resp = client
        .get(format!("http://127.0.0.1:{}/healthz", hs.port))
        .bearer_auth(&hs.bearer)
        .send()
        .await
        .map_err(|e| PingError::HealthRequest(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(PingError::HealthStatus(status.as_u16()));
    }
    #[derive(Deserialize)]
    struct HealthBody {
        status: String,
    }
    let body: HealthBody = resp
        .json()
        .await
        .map_err(|e| PingError::HealthRequest(e.to_string()))?;

    Ok(PingResult {
        status: body.status,
        port: hs.port,
    })
}
