use codebus_core::llm::provider::{InvokeOptions, LlmMode, LlmProvider};
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
}

/// Read-only LLM round-trip. Spawns the provider in [`LlmMode::Query`]
/// (no Write/Edit), feeds it the schema + index.md + the user query,
/// and yields each emitted [`StreamEvent`] back via the callback so the
/// CLI can render to stdout. No mutation, no commit.
pub async fn run_query(
    opts: RunQueryOptions<'_>,
    mut on_event: impl FnMut(&StreamEvent),
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
    };

    let mut stream = opts
        .provider
        .invoke(invoke)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    while let Some(event) = stream.next().await {
        on_event(&event);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codebus_core::llm::provider::ProviderError;
    use std::fs;
    use std::sync::{Arc, Mutex};

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
    async fn run_query_streams_events_to_callback() {
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
        let collected: Arc<Mutex<Vec<StreamEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let cl = collected.clone();
        run_query(
            RunQueryOptions {
                repo_root: &repo,
                query: "what is X?",
                provider: &provider,
            },
            move |e| cl.lock().unwrap().push(e.clone()),
        )
        .await
        .unwrap();

        let got = collected.lock().unwrap();
        assert_eq!(got.len(), 2);
        assert!(matches!(got[0], StreamEvent::Thought { .. }));
        let _ = fs::remove_dir_all(&repo);
    }
}
