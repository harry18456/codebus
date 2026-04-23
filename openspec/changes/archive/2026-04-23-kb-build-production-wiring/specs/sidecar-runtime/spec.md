## ADDED Requirements

### Requirement: KB dependency injection hook

The sidecar SHALL expose a `wire_kb_dependencies(app, *, openai_api_key, qdrant_url)` function that populates `app.state.kb_backend`, `app.state.kb_provider`, `app.state.kb_usage_tracker`, and `app.state.kb_embedding_dim` from resolved runtime inputs. The startup path (`main.py`) SHALL call this hook with values read from the `CODEBUS_OPENAI_API_KEY` and `CODEBUS_QDRANT_URL` environment variables (with existing resolver fallback for `CODEBUS_QDRANT_URL`). Missing values SHALL result in the corresponding `app.state.kb_*` slot being left as `None` rather than raising at startup, so the sidecar stays degraded-but-alive and `POST /kb/build` can return `503 KB_NOT_CONFIGURED` per the knowledge-base capability.

#### Scenario: Both env vars present wire all four slots

- **WHEN** the sidecar is started with `CODEBUS_OPENAI_API_KEY` set and Qdrant reachable at the resolved URL
- **THEN** `app.state.kb_backend`, `app.state.kb_provider`, `app.state.kb_usage_tracker`, and `app.state.kb_embedding_dim` MUST all be non-`None` after `create_app` returns

#### Scenario: Missing OpenAI API key leaves provider slot as None

- **WHEN** the sidecar is started without `CODEBUS_OPENAI_API_KEY` set
- **THEN** `app.state.kb_provider` and `app.state.kb_embedding_dim` MUST be `None`, the sidecar MUST still start successfully (stdout handshake line emitted, `/healthz` reachable), and `app.state.qdrant_client` MUST still be constructed when `CODEBUS_QDRANT_URL` is present

#### Scenario: UsageTracker slot is a factory, not a prebuilt instance

- **WHEN** `app.state.kb_usage_tracker` is read by the `POST /kb/build` endpoint
- **THEN** the slot MUST be callable with signature `(workspace_root: Path) -> UsageTracker` and MUST return a `UsageTracker` whose `path` resolves under the given `workspace_root` (per the workspace-scoped path convention in the `usage-tracking` capability)

#### Scenario: Provider slot is also a factory returning a TrackedProvider

- **WHEN** `app.state.kb_provider` is read by the `POST /kb/build` endpoint
- **THEN** the slot MUST be callable with signature `(workspace_root: Path) -> LLMProvider`, and the returned provider MUST be a `TrackedProvider` with role `ProviderRole.EMBED` whose inner audit components (`UsageTracker`, `LLMCallLogger`, `SanitizerAuditLogger`) all resolve under the given `workspace_root`. The factory is needed because `TrackedProvider` binds workspace-scoped audit paths at construction time, and the sidecar does not know the workspace at startup.

#### Scenario: Healthz smoke probe bypasses TrackedProvider

- **WHEN** the sidecar's startup smoke embed runs to populate `/healthz` `openai_embedding.status`
- **THEN** the probe SHALL invoke a raw `OpenAIEmbeddingProvider.embed(["ping"])` directly, NOT through a `TrackedProvider`, so the probe result does not pollute any workspace audit trail (`token_usage.jsonl` / `llm_calls.jsonl` / `sanitize_audit.jsonl`). This bypass is permitted because the probe is an operational check, not user-facing production traffic.

#### Scenario: Healthz reflects OpenAI embedding configuration state

- **WHEN** `GET /healthz` is called
- **THEN** the response `dependencies` map MUST contain an `openai_embedding` key whose `status` is one of `"ok"` (API key set and smoke embed call succeeded at startup), `"degraded"` (API key set but smoke call failed), or `"not-configured"` (API key absent)
