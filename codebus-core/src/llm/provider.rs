use crate::stream::StreamEvent;
use std::pin::Pin;

/// LLM invocation mode.
///
/// `Ingest` allows Write/Edit (used by `goal` to produce wiki pages);
/// `Query` is hard read-only (used by `query` and `check`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmMode {
    Ingest,
    Query,
}

#[derive(Debug, Clone)]
pub struct InvokeOptions {
    pub system_prompt: String,
    pub user_message: String,
    pub mode: LlmMode,
    /// Working directory for the subprocess. The agent's filesystem reach is
    /// scoped to this directory under `acceptEdits` mode (system-level
    /// isolation per spike E).
    pub cwd: std::path::PathBuf,
    /// Vault root, kept separately for callers that resolve schema / index
    /// paths independently of `cwd`.
    pub vault_root: std::path::PathBuf,
}

/// Stream of [`StreamEvent`]s yielded by the provider until the turn ends.
/// Pinned + boxed for trait-object compatibility (so [`LlmProvider`] can be
/// stored in `Box<dyn LlmProvider>` and passed across crate boundaries).
pub type EventStream = Pin<Box<dyn futures_core::Stream<Item = StreamEvent> + Send>>;

/// Abstract LLM provider. Phase 1 ships a single implementation
/// (`ClaudeCliProvider`); Phase 2 adds direct API providers (Anthropic,
/// OpenAI, local models) without touching this trait or any caller in
/// `commands/` / `core/` / `ui/`.
///
/// Errors are surfaced via the stream itself (e.g. provider can yield a
/// final event then the stream ends). Setup-level failures (e.g. binary
/// not found, OAuth missing) are reported via `invoke`'s Result return
/// before the stream begins.
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Begin a turn. Yields zero or more [`StreamEvent`]s ending with
    /// [`StreamEvent::Done`] under normal completion.
    async fn invoke(&self, opts: InvokeOptions) -> Result<EventStream, ProviderError>;

    /// Cancel an in-flight invocation. Idempotent: safe to call when no
    /// invocation is active. Implementations should send SIGTERM (or the
    /// platform equivalent) to any spawned subprocess.
    fn cancel(&self);
}

#[derive(Debug)]
pub enum ProviderError {
    /// The CLI binary or API endpoint could not be reached / authenticated.
    /// User-facing message in `message` (TS-parity: includes the OAuth hint
    /// for ClaudeCliProvider).
    Setup { message: String },
    /// An internal error (process spawn failure, malformed config, etc.).
    Internal(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Setup { message } => write!(f, "{message}"),
            ProviderError::Internal(msg) => write!(f, "internal provider error: {msg}"),
        }
    }
}

impl std::error::Error for ProviderError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::StreamEvent;
    use futures_util::StreamExt;

    /// In-memory test-only provider that yields a fixed sequence of events.
    /// Used by Phase C command tests in lieu of spawning a real `claude` CLI.
    struct MockProvider {
        events: Vec<StreamEvent>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn invoke(&self, _opts: InvokeOptions) -> Result<EventStream, ProviderError> {
            let evts = self.events.clone();
            Ok(Box::pin(futures_util::stream::iter(evts)))
        }

        fn cancel(&self) {}
    }

    #[tokio::test]
    async fn trait_is_object_safe_and_streams_through_box_dyn() {
        let provider: Box<dyn LlmProvider> = Box::new(MockProvider {
            events: vec![
                StreamEvent::Thought { text: "hi".into() },
                StreamEvent::Done,
            ],
        });
        let mut stream = provider
            .invoke(InvokeOptions {
                system_prompt: String::new(),
                user_message: String::new(),
                mode: LlmMode::Query,
                cwd: std::path::PathBuf::from("."),
                vault_root: std::path::PathBuf::from("."),
            })
            .await
            .unwrap();

        let mut collected = Vec::new();
        while let Some(e) = stream.next().await {
            collected.push(e);
        }
        assert_eq!(collected.len(), 2);
        assert!(matches!(collected[0], StreamEvent::Thought { .. }));
        assert!(matches!(collected[1], StreamEvent::Done));
    }

    #[test]
    fn provider_error_display_includes_message() {
        let e = ProviderError::Setup { message: "OAuth needed".into() };
        assert_eq!(format!("{e}"), "OAuth needed");
    }
}
