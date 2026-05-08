use codebus_core::llm::provider::{InvokeOptions, LlmMode, LlmProvider};
use codebus_core::log::{LogSink, RunLog, TokenUsage, accumulate_token_usage};
use codebus_core::render::EventRenderer;
use codebus_core::schema::CODEBUS_SCHEMA;
use codebus_core::stream::StreamEvent;
use codebus_core::vault::layout::vault_paths;
use futures_util::StreamExt;
use std::io;
use std::path::Path;

pub struct RunQueryOptions<'a> {
    pub repo_root: &'a Path,
    pub query: &'a str,
    pub provider: &'a dyn LlmProvider,
    /// Optional `--model` override forwarded to the LLM invocation.
    pub model: Option<&'a str>,
    /// Optional `--effort` override forwarded to the LLM invocation.
    pub effort: Option<&'a str>,
}

/// Read-only LLM round-trip. Spawns the provider in [`LlmMode::Query`]
/// (no Write/Edit), feeds it the schema + index.md + the user query,
/// and yields each emitted [`StreamEvent`] back via `renderer` so the
/// CLI can render to stdout. No mutation, no commit.
pub async fn run_query(
    opts: RunQueryOptions<'_>,
    renderer: &mut dyn EventRenderer,
    log_sink: &mut dyn LogSink,
) -> io::Result<()> {
    let p = vault_paths(opts.repo_root);
    if !p.root.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No codebus vault at {} — run init first", p.root.display()),
        ));
    }

    let index = if p.wiki_index.exists() {
        std::fs::read_to_string(&p.wiki_index)?
    } else {
        "(empty)".into()
    };

    let system_prompt = format!(
        "{CODEBUS_SCHEMA}\n\n# Current wiki index\n\n{index}\n\n# Query mode\n\nThe user is asking a question about the wiki. Use Read/Glob/Grep on existing pages and answer with citations. Do NOT write any files."
    );

    let invoke = InvokeOptions {
        system_prompt,
        user_message: opts.query.to_string(),
        mode: LlmMode::Query,
        cwd: p.root.clone(),
        vault_root: p.root.clone(),
        model: opts.model.map(str::to_string),
        effort: opts.effort.map(str::to_string),
    };

    let started_at = chrono::Utc::now().to_rfc3339();
    let mut accumulated_tokens = TokenUsage::default();

    let stream_result = opts.provider.invoke(invoke).await;
    let invoke_outcome: io::Result<()> = match stream_result {
        Ok(mut stream) => {
            while let Some(event) = stream.next().await {
                if let StreamEvent::Usage(u) = &event {
                    accumulate_token_usage(&mut accumulated_tokens, u);
                }
                renderer.render(&event);
            }
            Ok(())
        }
        Err(e) => Err(io::Error::other(e.to_string())),
    };

    // Build and write the RunLog whether or not the invoke succeeded —
    // partial token counts are still useful for cost tracking.
    let finished_at = chrono::Utc::now().to_rfc3339();
    let run_log = RunLog {
        goal: opts.query.to_string(),
        mode: "query".into(),
        model: opts.model.map(str::to_string),
        effort: opts.effort.map(str::to_string),
        started_at,
        finished_at,
        tokens: accumulated_tokens,
        wiki_changed: false,
        lint_error_count: 0,
        lint_warn_count: 0,
    };
    if let Err(e) = log_sink.write_run(&run_log) {
        eprintln!("warning: failed to write run log: {e}");
    }

    invoke_outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use codebus_core::llm::provider::ProviderError;
    use codebus_core::log::{LogError, sinks::null_sink::NullSink};
    use codebus_core::render::Banner;
    use codebus_core::wiki::types::LintResult;
    use std::fs;

    struct CollectingRenderer {
        events: Vec<StreamEvent>,
    }

    impl EventRenderer for CollectingRenderer {
        fn render(&mut self, e: &StreamEvent) {
            self.events.push(e.clone());
        }
        fn render_banner(&mut self, _: &Banner<'_>) {}
        fn render_lint_report(&mut self, _: &LintResult) {}
        fn render_lint_summary(&mut self, _: &LintResult) {}
    }

    /// Test sink that captures every RunLog into a Vec for assertions.
    /// Lets the integration test verify token accumulation + RunLog
    /// shape without touching the filesystem.
    #[derive(Default)]
    struct CapturingSink {
        captured: Vec<RunLog>,
    }

    impl LogSink for CapturingSink {
        fn name(&self) -> &str {
            "capturing"
        }
        fn write_run(&mut self, entry: &RunLog) -> Result<(), LogError> {
            self.captured.push(entry.clone());
            Ok(())
        }
    }

    struct ScriptedProvider {
        events: Vec<StreamEvent>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for ScriptedProvider {
        async fn invoke(
            &self,
            _opts: InvokeOptions,
        ) -> Result<codebus_core::llm::provider::EventStream, ProviderError> {
            Ok(Box::pin(futures_util::stream::iter(self.events.clone())))
        }
        fn cancel(&self) {}
    }

    fn nanos() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }

    #[tokio::test]
    async fn run_query_streams_events_to_renderer() {
        let repo =
            std::env::temp_dir().join(format!("codebus-q-{}-{}", std::process::id(), nanos()));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join(".codebus/wiki")).unwrap();
        fs::write(repo.join(".codebus/wiki/index.md"), "# index").unwrap();

        let provider = ScriptedProvider {
            events: vec![
                StreamEvent::Thought {
                    text: "thinking".into(),
                },
                StreamEvent::Done,
            ],
        };
        let mut renderer = CollectingRenderer { events: Vec::new() };
        let mut sink = NullSink::new();
        run_query(
            RunQueryOptions {
                repo_root: &repo,
                query: "what is X?",
                provider: &provider,
                model: None,
                effort: None,
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .unwrap();

        assert_eq!(renderer.events.len(), 2);
        assert!(matches!(renderer.events[0], StreamEvent::Thought { .. }));
        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn run_query_writes_run_log_with_accumulated_tokens() {
        // Spec scenario: "Query flow writes a single RunLog after success"
        // — mock provider emits Thought + Usage + Done; sink captures the
        // RunLog; assert mode/model/effort/tokens populated correctly.
        let repo = std::env::temp_dir()
            .join(format!("codebus-q-runlog-{}-{}", std::process::id(), nanos()));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join(".codebus/wiki")).unwrap();
        fs::write(repo.join(".codebus/wiki/index.md"), "# index").unwrap();

        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: Some(20),
            cache_write_tokens: None,
            reasoning_tokens: None,
            extras: serde_json::json!({"raw": "object"}),
        };
        let provider = ScriptedProvider {
            events: vec![
                StreamEvent::Thought {
                    text: "thinking".into(),
                },
                StreamEvent::Usage(usage.clone()),
                StreamEvent::Done,
            ],
        };
        let mut renderer = CollectingRenderer { events: Vec::new() };
        let mut sink = CapturingSink::default();
        run_query(
            RunQueryOptions {
                repo_root: &repo,
                query: "what is X?",
                provider: &provider,
                model: Some("haiku"),
                effort: Some("low"),
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .unwrap();

        assert_eq!(sink.captured.len(), 1, "exactly one RunLog written");
        let log = &sink.captured[0];
        assert_eq!(log.mode, "query");
        assert_eq!(log.model.as_deref(), Some("haiku"));
        assert_eq!(log.effort.as_deref(), Some("low"));
        assert_eq!(log.tokens.input_tokens, 100);
        assert_eq!(log.tokens.output_tokens, 50);
        assert_eq!(log.tokens.cache_read_tokens, Some(20));
        assert_eq!(log.tokens.cache_write_tokens, None);
        assert!(!log.wiki_changed, "query never mutates");
        assert_eq!(log.lint_error_count, 0);
        // extras carried verbatim through accumulation.
        assert_eq!(
            log.tokens.extras.get("raw").and_then(|v| v.as_str()),
            Some("object")
        );

        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn run_query_default_null_sink_silently_discards_run_log() {
        // Spec scenario: "Default null sink discards the RunLog write
        // silently" — NullSink's write_run is a no-op; query still
        // succeeds.
        let repo = std::env::temp_dir().join(format!(
            "codebus-q-nullsink-{}-{}",
            std::process::id(),
            nanos()
        ));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join(".codebus/wiki")).unwrap();
        fs::write(repo.join(".codebus/wiki/index.md"), "# index").unwrap();

        let provider = ScriptedProvider {
            events: vec![
                StreamEvent::Usage(TokenUsage {
                    input_tokens: 5,
                    output_tokens: 3,
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                    reasoning_tokens: None,
                    extras: serde_json::Value::Null,
                }),
                StreamEvent::Done,
            ],
        };
        let mut renderer = CollectingRenderer { events: Vec::new() };
        let mut sink = NullSink::new();
        run_query(
            RunQueryOptions {
                repo_root: &repo,
                query: "noop",
                provider: &provider,
                model: None,
                effort: None,
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .expect("run_query succeeds even with NullSink");

        let _ = fs::remove_dir_all(&repo);
    }
}
