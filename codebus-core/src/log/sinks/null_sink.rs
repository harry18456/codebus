//! `NullSink` — opt-out implementation. `write_run` returns `Ok(())` without
//! touching the filesystem. Selected via `~/.codebus/config.yaml`:
//!
//! ```yaml
//! log:
//!   sink: "null"
//! ```
//!
//! (The `"null"` MUST be quoted in YAML to avoid the bare null literal —
//! same foot-gun avoidance pattern as `pii.scanner: none`.)

use crate::log::sink::{LogError, LogSink, RunLog};

pub struct NullSink;

impl NullSink {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NullSink {
    fn default() -> Self {
        Self::new()
    }
}

impl LogSink for NullSink {
    fn name(&self) -> &str {
        "null"
    }

    fn write_run(&mut self, _entry: &RunLog) -> Result<(), LogError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::sink::TokenUsage;

    fn dummy_run() -> RunLog {
        RunLog {
            goal: "x".into(),
            mode: "goal".into(),
            model: None,
            effort: None,
            started_at: "2026-05-10T00:00:00Z".into(),
            finished_at: "2026-05-10T00:00:01Z".into(),
            tokens: TokenUsage::default(),
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
            outcome: "succeeded".into(),
            session_id: None,
            sandbox_denial_count: 0,
            interrupt_reason: None,
        }
    }

    #[test]
    fn name_is_stable_string_null() {
        assert_eq!(NullSink::new().name(), "null");
    }

    #[test]
    fn write_run_returns_ok_without_io() {
        let mut sink = NullSink::new();
        let result = sink.write_run(&dummy_run());
        assert!(result.is_ok());
    }
}
