use codebus_core::llm::provider::{InvokeOptions, LlmMode, LlmProvider};
use codebus_core::log::LogSink;
use codebus_core::render::EventRenderer;
use codebus_core::schema::CODEBUS_SCHEMA;
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
    // Plumbing only: per the change Non-Goal "不啟用 LogSink 寫檔", the sink
    // is accepted but not wired in this change.
    let _ = log_sink;
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

    let mut stream = opts
        .provider
        .invoke(invoke)
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;

    while let Some(event) = stream.next().await {
        renderer.render(&event);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codebus_core::llm::provider::ProviderError;
    use codebus_core::log::sinks::null_sink::NullSink;
    use codebus_core::render::Banner;
    use codebus_core::stream::StreamEvent;
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
}
