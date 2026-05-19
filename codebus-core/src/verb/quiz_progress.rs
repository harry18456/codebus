//! `verb::quiz_progress` — per-attempt progress sidecar (design D1/D2).
//!
//! The generated attempt markdown (`<vault>/.codebus/quiz/<slug>/<quiz_id>.md`)
//! stays immutable. This module owns the additive sibling
//! `<quiz_id>.progress.json` sidecar that records ONLY the non-derivable
//! answering state: `schema_version`, `answers`, `status`, `started_at`,
//! `completed_at`. Derived quantities (total / answered / correct / score /
//! pass-fail) are recomputed by callers from `answers` + the markdown, never
//! stored (single source of truth — design D1).
//!
//! Read tolerance mirrors the `config::quiz` loader: a missing file yields
//! the not-started state (not an error); a malformed file is treated as
//! not-started rather than panicking; unknown JSON keys are ignored; a
//! `schema_version` newer than known still best-effort reads known fields.
//! Writes are atomic (temp file in the same dir + rename over the target).

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Current sidecar schema version written by this build. A sidecar with a
/// higher `schema_version` is still best-effort read (known fields only).
pub const QUIZ_PROGRESS_SCHEMA_VERSION: u32 = 1;

/// The user's chosen answer for a question. A semantic four-variant enum
/// rather than a bare `String` so a caller cannot accidentally pass a free
/// string or swap it with `q` (audit Confused-Developer lens). Serializes
/// as the exact spec letters `"A"`/`"B"`/`"C"`/`"D"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Choice {
    A,
    B,
    C,
    D,
}

/// Answering lifecycle. The sidecar on disk only ever stores `InProgress`
/// or `Completed` (an absent sidecar means not-started — see [`read_progress`]);
/// `NotStarted` is the in-memory result returned for a missing/malformed
/// file so callers get one total return type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuizStatus {
    NotStarted,
    /// Default for a present-but-status-less sidecar: it has answers but is
    /// not yet finished.
    #[default]
    InProgress,
    Completed,
}

/// One answered question. `q` is the 1-based question number; `selected` is
/// the user's choice; `correct` is the client-side grade against the
/// markdown `Answer` field (the agent never sees user answers — design D3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuizAnswer {
    pub q: u32,
    pub selected: Choice,
    pub correct: bool,
}

/// The question the user is currently viewing and whether it was
/// already submitted (design D3 final — precise-cursor resume). Written
/// on every submission (`revealed: true`) and every Next
/// (`revealed: false`) so reopening restores the exact position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuizCursor {
    /// 1-based question number.
    pub q: u32,
    /// Whether question `q` has been submitted (its answer revealed).
    pub revealed: bool,
}

/// The non-derivable per-attempt answering state (design D1). Total /
/// answered / correct / score / pass-fail are NOT stored here — callers
/// recompute them from `answers` + the attempt markdown so the sidecar
/// cannot hold self-contradictory fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuizProgress {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub answers: Vec<QuizAnswer>,
    #[serde(default)]
    pub status: QuizStatus,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    /// Precise resume position (design D3 final). OPTIONAL and
    /// `#[serde(default)]` so a legacy / prior-build sidecar without it
    /// stays valid (`None`) — callers fall back to "last answered,
    /// revealed". No `schema_version` bump is needed: serde field-default
    /// + the unknown-key tolerance already cover absent/extra fields.
    #[serde(default)]
    pub cursor: Option<QuizCursor>,
}

fn default_schema_version() -> u32 {
    QUIZ_PROGRESS_SCHEMA_VERSION
}

impl QuizProgress {
    /// The not-started state: no answers, `NotStarted` status, no
    /// timestamps. Returned for an absent or unreadable sidecar.
    pub fn not_started() -> Self {
        Self {
            schema_version: QUIZ_PROGRESS_SCHEMA_VERSION,
            answers: Vec::new(),
            status: QuizStatus::NotStarted,
            started_at: None,
            completed_at: None,
            cursor: None,
        }
    }
}

/// Read the progress sidecar at `path`. Tolerant by contract (design D2,
/// mirrors the `config::quiz` loader): a missing file yields the
/// not-started state (not an error); any read/parse failure is logged and
/// also degrades to not-started rather than panicking; unknown JSON keys
/// are ignored and a `schema_version` newer than known still best-effort
/// reads the known fields (serde without `deny_unknown_fields`).
pub fn read_progress(path: &Path) -> QuizProgress {
    let body = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return QuizProgress::not_started();
        }
        Err(err) => {
            eprintln!(
                "warning: quiz progress sidecar read failed (treating as not-started): {err}"
            );
            return QuizProgress::not_started();
        }
    };
    match serde_json::from_str::<QuizProgress>(&body) {
        Ok(p) => p,
        Err(err) => {
            eprintln!(
                "warning: quiz progress sidecar is malformed (treating as not-started): {err}"
            );
            QuizProgress::not_started()
        }
    }
}

/// Atomically persist `progress` to `path`: serialize into a sibling temp
/// file in the *same* directory, then `fs::rename` over the target. An
/// interrupted write therefore cannot corrupt an existing sidecar, and on
/// Windows `fs::rename` replaces an existing file (design Risks note).
pub fn write_progress(path: &Path, progress: &QuizProgress) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(progress)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "progress".to_string());
    let tmp = path.with_file_name(format!(".{file_name}.tmp"));
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample() -> QuizProgress {
        QuizProgress {
            schema_version: QUIZ_PROGRESS_SCHEMA_VERSION,
            answers: vec![
                QuizAnswer {
                    q: 1,
                    selected: Choice::A,
                    correct: true,
                },
                QuizAnswer {
                    q: 2,
                    selected: Choice::C,
                    correct: false,
                },
            ],
            status: QuizStatus::InProgress,
            started_at: Some("2026-05-18T10:00:00Z".into()),
            completed_at: None,
            cursor: None,
        }
    }

    /// 1.1(a): a missing sidecar reads as the not-started state, not an error.
    #[test]
    fn missing_file_reads_not_started() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("absent.progress.json");
        let got = read_progress(&p);
        assert_eq!(got.status, QuizStatus::NotStarted);
        assert!(got.answers.is_empty());
        assert!(got.completed_at.is_none());
    }

    /// 1.1(b): a malformed/garbage file is treated as not-started, no panic.
    #[test]
    fn malformed_file_reads_not_started() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("bad.progress.json");
        std::fs::write(&p, "}{ this is not json at all \0\u{1f}").unwrap();
        let got = read_progress(&p);
        assert_eq!(got.status, QuizStatus::NotStarted);
        assert!(got.answers.is_empty());
    }

    /// 1.1(c): write then read round-trips the persisted fields exactly.
    #[test]
    fn round_trips_through_disk() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a.progress.json");
        let original = sample();
        write_progress(&p, &original).unwrap();
        let got = read_progress(&p);
        assert_eq!(got, original);
    }

    /// 1.1(d): unknown JSON keys are ignored AND a newer `schema_version`
    /// still best-effort reads the known fields (forward-compatible).
    #[test]
    fn unknown_keys_ignored_and_future_schema_version_read() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("future.progress.json");
        std::fs::write(
            &p,
            r#"{
                "schema_version": 999,
                "answers": [{"q": 1, "selected": "B", "correct": true, "extra_answer_key": 7}],
                "status": "completed",
                "started_at": "2026-05-18T10:00:00Z",
                "completed_at": "2026-05-18T10:05:00Z",
                "future_top_level_key": {"nested": "ignored"}
            }"#,
        )
        .unwrap();
        let got = read_progress(&p);
        assert_eq!(got.schema_version, 999);
        assert_eq!(got.status, QuizStatus::Completed);
        assert_eq!(got.answers.len(), 1);
        assert_eq!(got.answers[0].q, 1);
        assert_eq!(got.answers[0].selected, Choice::B);
        assert!(got.answers[0].correct);
        assert_eq!(got.completed_at.as_deref(), Some("2026-05-18T10:05:00Z"));
    }

    /// 1.1(e): an atomic write over an existing sidecar leaves only the
    /// second write's content AND no `.tmp` residue in the same directory.
    #[test]
    fn atomic_write_overwrites_and_leaves_no_tmp() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("o.progress.json");

        let mut first = sample();
        first.answers.truncate(1);
        write_progress(&p, &first).unwrap();

        let mut second = sample();
        second.status = QuizStatus::Completed;
        second.completed_at = Some("2026-05-18T10:09:00Z".into());
        write_progress(&p, &second).unwrap();

        let got = read_progress(&p);
        assert_eq!(got, second, "final content must be the second write");

        let residue: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x == "tmp")
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            residue.is_empty(),
            "no .tmp file may remain after an atomic write: {residue:?}"
        );
    }

    // --- task 11.1 (design D3 final): optional `cursor` ---

    /// 11.1(a): a sidecar carrying a `cursor` round-trips intact.
    #[test]
    fn cursor_round_trips_through_disk() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("c.progress.json");
        let mut original = sample();
        original.cursor = Some(QuizCursor {
            q: 4,
            revealed: false,
        });
        write_progress(&p, &original).unwrap();
        let got = read_progress(&p);
        assert_eq!(got.cursor, Some(QuizCursor { q: 4, revealed: false }));
        assert_eq!(got, original);
    }

    /// 11.1(b): a sidecar JSON omitting `cursor` parses with `cursor: None`
    /// (backward compatible — legacy / prior-build sidecars stay valid).
    #[test]
    fn absent_cursor_key_parses_as_none() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("legacy.progress.json");
        std::fs::write(
            &p,
            r#"{
                "schema_version": 1,
                "answers": [{"q": 1, "selected": "A", "correct": true}],
                "status": "in_progress",
                "started_at": "2026-05-18T10:00:00Z",
                "completed_at": null
            }"#,
        )
        .unwrap();
        let got = read_progress(&p);
        assert_eq!(got.cursor, None);
        assert_eq!(got.answers.len(), 1);
    }
}
