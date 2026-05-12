//! Verb orchestration library.
//!
//! Three `pub fn run_*` orchestration functions — `goal::run_goal`,
//! `query::run_query`, `fix::run_fix` — extracted from the CLI binary so
//! GUI callers (`v3-app-workspace-goal`) can reuse the same full verb
//! flow without re-implementing drift detection, agent spawn, fix loop,
//! and auto-commit.
//!
//! Each `run_*` function:
//! - Accepts `repo: &Path` + verb-specific `*Options` struct
//! - Emits all observable progress through a caller-supplied
//!   `on_event: impl FnMut(VerbEvent)` closure (no direct stdout / stderr writes)
//! - Honors a `cancel: Option<Arc<AtomicBool>>` cancellation flag
//!   (polled between stream events; flip true → kill child + return
//!   `Err(VerbError::Cancelled)`)
//! - Returns `Ok(*Report)` on success or `Err(VerbError)` on failure
//!
//! The CLI binary (`codebus-cli/src/commands/{goal,query,fix}.rs`) acts as
//! a thin wrapper: clap parse → construct `VerbEvent` dispatch closure
//! (Banner → `print_banner`, Stream → `print_event`, Lifecycle → no-op)
//! → call `run_*` → match `VerbError` for exit code → write RunLog. CLI
//! output (stdout, stderr, exit code) is byte-equivalent to the
//! pre-extraction implementation.

pub mod error;
pub mod event;

pub mod goal;
pub mod query;
pub mod fix;

pub use error::VerbError;
pub use event::{VerbBanner, VerbEvent, VerbLifecycleEvent};

/// Re-exported from `config::Verb` for verb-library consumers — kept
/// alongside the rest of the verb surface so downstream code can write
/// `use codebus_core::verb::Verb;` without reaching into `config`.
pub use crate::config::Verb;
