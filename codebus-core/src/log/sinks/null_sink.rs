//! `null` sink — no-op `LogSink`. The 0.2.0 behavior-preserving default
//! used when `~/.codebus/config.yaml` doesn't pin a sink.

use crate::log::sink::{LogError, LogSink, RunLog, TokenUsage};

#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl NullSink {
    pub fn new() -> Self {
        Self
    }
}

impl LogSink for NullSink {
    fn name(&self) -> &str {
        "null"
    }

    fn write_run(&mut self, _entry: &RunLog) -> Result<(), LogError> {
        Ok(())
    }

    fn write_token_usage(&mut self, _usage: &TokenUsage) -> Result<(), LogError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_run() -> RunLog {
        RunLog {
            goal: "explore foo".into(),
            mode: "goal".into(),
            model: None,
            effort: None,
            started_at: "2026-05-06T10:00:00Z".into(),
            finished_at: "2026-05-06T10:00:30Z".into(),
            tokens: TokenUsage::default(),
            wiki_changed: true,
            lint_error_count: 0,
            lint_warn_count: 2,
        }
    }

    #[test]
    fn null_sink_write_run_is_noop() {
        let mut s = NullSink::new();
        assert!(s.write_run(&sample_run()).is_ok());
    }

    #[test]
    fn null_sink_write_token_usage_is_noop() {
        let mut s = NullSink::new();
        assert!(s.write_token_usage(&TokenUsage::default()).is_ok());
    }

    #[test]
    fn null_sink_name_is_stable() {
        assert_eq!(NullSink::new().name(), "null");
    }

    #[test]
    fn null_sink_is_object_safe() {
        let _: Box<dyn LogSink> = Box::new(NullSink::new());
    }

    #[test]
    fn null_sink_flush_is_noop() {
        let mut s = NullSink::new();
        assert!(s.flush().is_ok());
    }
}
