## ADDED Requirements

### Requirement: KB build production dependency wiring

The sidecar SHALL expose a `POST /kb/build` endpoint that, when all four KB dependencies (`kb_backend`, `kb_provider`, `kb_usage_tracker` factory, `kb_embedding_dim`) are populated on `app.state`, executes a full chunk → embed → upsert pipeline and makes the resulting `KBStats` retrievable via `GET /tasks/{id}/result`. When any dependency is absent or misconfigured, the endpoint SHALL respond with a specific, documented error code so the caller can recover without restarting the sidecar.

#### Scenario: Happy path returns KBStats via result endpoint

- **WHEN** `CODEBUS_OPENAI_API_KEY` is set, Qdrant is reachable, and the caller posts a valid `{workspace_root, scan_result}` body to `POST /kb/build`
- **THEN** the endpoint MUST return `200 {"task_id": "kb_<hex8>"}` within 2 seconds, emit `progress` and `done` events over the SSE stream, and make a `KBStats` object with non-zero `chunks_emitted` and `points_upserted` reachable through `GET /tasks/{task_id}/result`

#### Scenario: Missing OpenAI API key returns 503 KB_NOT_CONFIGURED

- **WHEN** the sidecar starts without `CODEBUS_OPENAI_API_KEY` and the caller posts to `POST /kb/build`
- **THEN** the endpoint MUST return `503` with body `{"code": "KB_NOT_CONFIGURED", "missing": ["embedding_provider"]}` and MUST NOT create a task handle, MUST NOT emit any SSE events, and MUST NOT call the Qdrant backend

#### Scenario: Existing collection with wrong vector dimension returns 409 KB_DIM_MISMATCH

- **WHEN** the Qdrant collection named by the workspace already exists with a vector dimension different from the dimension declared by the configured embedding provider, and the caller posts to `POST /kb/build`
- **THEN** the background task MUST emit an SSE `error` event with `{"code": "KB_DIM_MISMATCH", "expected_dim": <provider-dim>, "actual_dim": <collection-dim>, "suggestion": "delete collection and rebuild"}` before any embedding calls are made, and MUST NOT upsert any points

#### Scenario: OpenAI rate limit surfaces as sanitized error event

- **WHEN** the OpenAI embedding provider exhausts its internal retry budget during a `POST /kb/build` task
- **THEN** the background task MUST emit an SSE `error` event with `code: "OPENAI_RATE_LIMITED"` (or `OPENAI_AUTH_FAILED` for 401 responses), MUST NOT leak the provider's stack trace in the wire event, and the full traceback MUST be written only to the sidecar logger

#### Scenario: UsageTracker records embedding call for the requesting workspace

- **WHEN** a `POST /kb/build` task completes successfully against `workspace_root=/abs/example`
- **THEN** at least one line with `operation="embed"` and `module="kb_build"` MUST be appended to `/abs/example/token_usage.jsonl` (or the workspace-scoped path defined by the existing `UsageTracker writes token_usage.jsonl` Requirement), with `input_tokens > 0` and a non-null `cost_usd`
