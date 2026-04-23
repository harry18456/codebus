## MODIFIED Requirements

### Requirement: KB dependency injection hook

The sidecar SHALL expose a `wire_kb_dependencies(app, *, openai_api_key, qdrant_url)` function that populates `app.state.kb_backend`, `app.state.kb_provider`, `app.state.kb_usage_tracker`, `app.state.kb_embedding_dim`, `app.state.kb_query_provider`, `app.state.llm_reasoning_provider`, `app.state.llm_judge_provider`, and `app.state.llm_chat_provider` from resolved runtime inputs. The startup path (`main.py`) SHALL call this hook with values read from the `CODEBUS_OPENAI_API_KEY` and `CODEBUS_QDRANT_URL` environment variables (with existing resolver fallback for `CODEBUS_QDRANT_URL`). Missing values SHALL result in the corresponding slot being left as `None` rather than raising at startup, so the sidecar stays degraded-but-alive — `POST /kb/build`, `POST /kb/query`, and any chat-ish caller (e.g., Module 4 Explorer) all return their respective `503 *_NOT_CONFIGURED` errors.

The chat-ish slots (`llm_reasoning_provider` / `llm_judge_provider` / `llm_chat_provider`) added by `chat-provider-wiring` follow the same factory-of-`TrackedProvider` pattern as the embedding slots: each slot is `Callable[[Path], TrackedProvider]`, the factory builds a workspace-scoped TrackedProvider wrapping `OpenAIChatProvider` with role-appropriate `default_module` (`"reasoning"`, `"judge"`, `"chat"`) and per-role temperature defaults (`reasoning`: 0.1, `judge`: 0.0, `chat`: 0.2). All three default to model `"gpt-4o-mini"`.

#### Scenario: Both env vars present wire all eight slots

- **WHEN** the sidecar is started with `CODEBUS_OPENAI_API_KEY` set and Qdrant reachable at the resolved URL
- **THEN** all of `app.state.kb_backend`, `app.state.kb_provider`, `app.state.kb_query_provider`, `app.state.kb_usage_tracker`, `app.state.kb_embedding_dim`, `app.state.llm_reasoning_provider`, `app.state.llm_judge_provider`, and `app.state.llm_chat_provider` MUST be non-`None` after `create_app` returns

#### Scenario: Missing OpenAI API key leaves provider slot as None

- **WHEN** the sidecar is started without `CODEBUS_OPENAI_API_KEY` set
- **THEN** all OpenAI-dependent slots MUST be `None` (`kb_provider`, `kb_query_provider`, `kb_embedding_dim`, `llm_reasoning_provider`, `llm_judge_provider`, `llm_chat_provider`); the sidecar MUST still start successfully (stdout handshake line emitted, `/healthz` reachable), and `app.state.qdrant_client` MUST still be constructed when `CODEBUS_QDRANT_URL` is present

#### Scenario: UsageTracker slot is a factory, not a prebuilt instance

- **WHEN** `app.state.kb_usage_tracker` is read by the `POST /kb/build` endpoint
- **THEN** the slot MUST be callable with signature `(workspace_root: Path) -> UsageTracker` and MUST return a `UsageTracker` whose `path` resolves under the given `workspace_root` (per the workspace-scoped path convention in the `usage-tracking` capability)

#### Scenario: Provider slot is also a factory returning a TrackedProvider

- **WHEN** `app.state.kb_provider` is read by the `POST /kb/build` endpoint
- **THEN** the slot MUST be callable with signature `(workspace_root: Path) -> LLMProvider`, and the returned provider MUST be a `TrackedProvider` with role `ProviderRole.EMBED` whose inner audit components (`UsageTracker`, `LLMCallLogger`, `SanitizerAuditLogger`) all resolve under the given `workspace_root`. The factory is needed because `TrackedProvider` binds workspace-scoped audit paths at construction time, and the sidecar does not know the workspace at startup.

#### Scenario: Chat-ish provider slots are factories returning TrackedProviders with role-appropriate default_module

- **WHEN** `app.state.llm_reasoning_provider`, `app.state.llm_judge_provider`, or `app.state.llm_chat_provider` is invoked with a workspace path
- **THEN** the returned provider MUST be a `TrackedProvider` wrapping an `OpenAIChatProvider` with role-appropriate `default_module` (`"reasoning"`, `"judge"`, `"chat"` respectively) and matching `ProviderRole` (`REASONING`, `JUDGE`, `CHAT`); each slot MUST produce distinct TrackedProvider instances per call (no shared state across workspaces)

#### Scenario: Healthz smoke probe bypasses TrackedProvider

- **WHEN** the sidecar's startup smoke embed runs to populate `/healthz` `openai_embedding.status`
- **THEN** the probe SHALL invoke a raw `OpenAIEmbeddingProvider.embed(["ping"])` directly, NOT through a `TrackedProvider`, so the probe result does not pollute any workspace audit trail (`token_usage.jsonl` / `llm_calls.jsonl` / `sanitize_audit.jsonl`). This bypass is permitted because the probe is an operational check, not user-facing production traffic.

#### Scenario: Healthz reflects OpenAI embedding configuration state

- **WHEN** `GET /healthz` is called
- **THEN** the response `dependencies` map MUST contain an `openai_embedding` key whose `status` is one of `"ok"` (API key set and smoke embed call succeeded at startup), `"degraded"` (API key set but smoke call failed), or `"not-configured"` (API key absent)

#### Scenario: Healthz reflects OpenAI chat configuration state

- **WHEN** `GET /healthz` is called after `chat-provider-wiring` lands
- **THEN** the response `dependencies` map MUST also contain an `openai_chat` key whose `status` is one of `"ok"` (API key set and a startup smoke chat completion against `gpt-4o-mini` succeeded), `"degraded"` (API key set but smoke call failed), or `"not-configured"` (API key absent). The `openai_chat` probe SHALL invoke a raw `OpenAIChatProvider`, NOT through a `TrackedProvider`, mirroring the embedding probe's bypass rule (operational check MUST NOT pollute audit trail). One probe covers all three chat-ish roles since they share the same OpenAI API + key.
