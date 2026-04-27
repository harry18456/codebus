## MODIFIED Requirements

### Requirement: KB dependency injection hook

The sidecar SHALL expose a `wire_kb_dependencies(app, *, openai_api_key, qdrant_url)` function that populates **all twelve** workspace-scoped `app.state` slots from resolved runtime inputs: `app.state.kb_backend`, `app.state.kb_provider`, `app.state.kb_query_provider`, `app.state.kb_usage_tracker`, `app.state.kb_embedding_dim`, `app.state.llm_reasoning_provider`, `app.state.llm_judge_provider`, `app.state.llm_chat_provider`, `app.state.llm_coverage_provider`, `app.state.llm_generate_provider`, `app.state.llm_qa_provider`, and `app.state.kb_growth_logger_factory`. The startup path (`main.py`) SHALL call this hook with values read from the `CODEBUS_OPENAI_API_KEY` and `CODEBUS_QDRANT_URL` environment variables (with existing resolver fallback for `CODEBUS_QDRANT_URL`). Missing values SHALL result in the corresponding slot being left as `None` rather than raising at startup, so the sidecar stays degraded-but-alive — `POST /kb/build`, `POST /kb/query`, `POST /explore`, `POST /generate`, and `POST /qa` all return their respective `503 *_NOT_CONFIGURED` errors.

The chat-ish slots follow the same factory-of-`TrackedProvider` pattern as the embedding slots: each slot is `Callable[[Path], TrackedProvider]`, the factory builds a workspace-scoped TrackedProvider wrapping `OpenAIChatProvider` with role-appropriate `default_module` (`"reasoning"` / `"judge"` / `"chat"` / `"coverage"` / `"generate"` / `"qa_agent"`) and per-role temperature defaults (`reasoning`: 0.1, `judge` / `coverage`: 0.0, `chat` / `qa_agent`: 0.2, `generate`: 0.4). All chat-ish slots default to model `"gpt-4o-mini"`. The `kb_growth_logger_factory` slot is `Callable[[Path], KBGrowthLogger]` returning a `KBGrowthLogger` whose `path` resolves under `<workspace_root>/.codebus/kb_growth.jsonl`; it lands with the `module-8-qa-p0` Q&A pipeline and is required by the `/qa` endpoint.

#### Scenario: Both env vars present wire all twelve slots

- **WHEN** the sidecar is started with `CODEBUS_OPENAI_API_KEY` set and Qdrant reachable at the resolved URL
- **THEN** all twelve of `app.state.kb_backend`, `app.state.kb_provider`, `app.state.kb_query_provider`, `app.state.kb_usage_tracker`, `app.state.kb_embedding_dim`, `app.state.llm_reasoning_provider`, `app.state.llm_judge_provider`, `app.state.llm_chat_provider`, `app.state.llm_coverage_provider`, `app.state.llm_generate_provider`, `app.state.llm_qa_provider`, and `app.state.kb_growth_logger_factory` MUST be non-`None` after `create_app` returns

#### Scenario: Missing OpenAI API key leaves provider slot as None

- **WHEN** the sidecar is started without `CODEBUS_OPENAI_API_KEY` set
- **THEN** all OpenAI-dependent slots MUST be `None` (`kb_provider`, `kb_query_provider`, `kb_embedding_dim`, `llm_reasoning_provider`, `llm_judge_provider`, `llm_chat_provider`, `llm_coverage_provider`, `llm_generate_provider`, `llm_qa_provider`, `kb_growth_logger_factory`); the sidecar MUST still start successfully (stdout handshake line emitted, `/healthz` reachable), and `app.state.qdrant_client` MUST still be constructed when `CODEBUS_QDRANT_URL` is present

#### Scenario: UsageTracker slot is a factory, not a prebuilt instance

- **WHEN** `app.state.kb_usage_tracker` is read by the `POST /kb/build` endpoint
- **THEN** the slot MUST be callable with signature `(workspace_root: Path) -> UsageTracker` and MUST return a `UsageTracker` whose `path` resolves under the given `workspace_root` (per the workspace-scoped path convention in the `usage-tracking` capability)

#### Scenario: Provider slot is also a factory returning a TrackedProvider

- **WHEN** `app.state.kb_provider` is read by the `POST /kb/build` endpoint
- **THEN** the slot MUST be callable with signature `(workspace_root: Path) -> LLMProvider`, and the returned provider MUST be a `TrackedProvider` with role `ProviderRole.EMBED` whose inner audit components (`UsageTracker`, `LLMCallLogger`, `SanitizerAuditLogger`) all resolve under the given `workspace_root`. The factory is needed because `TrackedProvider` binds workspace-scoped audit paths at construction time, and the sidecar does not know the workspace at startup.

#### Scenario: Chat-ish provider slots are factories returning TrackedProviders with role-appropriate default_module

- **WHEN** `app.state.llm_reasoning_provider`, `app.state.llm_judge_provider`, `app.state.llm_chat_provider`, `app.state.llm_coverage_provider`, `app.state.llm_generate_provider`, or `app.state.llm_qa_provider` is invoked with a workspace path
- **THEN** the returned provider MUST be a `TrackedProvider` wrapping an `OpenAIChatProvider` with role-appropriate `default_module` (`"reasoning"` / `"judge"` / `"chat"` / `"coverage"` / `"generate"` / `"qa_agent"` respectively) and matching `ProviderRole` (`REASONING` / `JUDGE` / `CHAT` / `JUDGE` / `CHAT` / `CHAT`); each slot MUST produce distinct TrackedProvider instances per call (no shared state across workspaces)

#### Scenario: KB growth logger factory targets the workspace .codebus subdirectory

- **WHEN** `app.state.kb_growth_logger_factory` is invoked with a workspace path by the `POST /qa` endpoint
- **THEN** the slot MUST be callable with signature `(workspace_root: Path) -> KBGrowthLogger`, and the returned logger MUST resolve its `path` to `<workspace_root>/.codebus/kb_growth.jsonl` (the seventh workspace-level audit JSONL per the `kb-growth` capability single-source contract)

#### Scenario: Healthz smoke probe bypasses TrackedProvider

- **WHEN** the sidecar's startup smoke embed runs to populate `/healthz` `openai_embedding.status`
- **THEN** the probe SHALL invoke a raw `OpenAIEmbeddingProvider.embed(["ping"])` directly, NOT through a `TrackedProvider`, so the probe result does not pollute any workspace audit trail (`token_usage.jsonl` / `llm_calls.jsonl` / `sanitize_audit.jsonl`). This bypass is permitted because the probe is an operational check, not user-facing production traffic.

#### Scenario: Healthz reflects OpenAI embedding configuration state

- **WHEN** `GET /healthz` is called
- **THEN** the response `dependencies` map MUST contain an `openai_embedding` key whose `status` is one of `"ok"` (API key set and smoke embed call succeeded at startup), `"degraded"` (API key set but smoke call failed), or `"not-configured"` (API key absent)

#### Scenario: Healthz reflects OpenAI chat configuration state

- **WHEN** `GET /healthz` is called after `chat-provider-wiring` lands
- **THEN** the response `dependencies` map MUST also contain an `openai_chat` key whose `status` is one of `"ok"` (API key set and a startup smoke chat completion against `gpt-4o-mini` succeeded), `"degraded"` (API key set but smoke call failed), or `"not-configured"` (API key absent). The `openai_chat` probe SHALL invoke a raw `OpenAIChatProvider`, NOT through a `TrackedProvider`, mirroring the embedding probe's bypass rule (operational check MUST NOT pollute audit trail). One probe covers all chat-ish roles since they share the same OpenAI API + key.

---

### Requirement: Q&A task spawn endpoint

The sidecar SHALL expose `POST /qa` whose request body is a Pydantic model with three fields: `workspace_root: str` (absolute path), `question: str`, and `originating_station_id: str | None`. The endpoint SHALL validate the request as follows before spawning any background task: `workspace_root` MUST resolve to an existing directory and MUST pass `ensure_in_workspace`-style validation (the same path-safety primitive used by `/scan` and `/explore`); `question` MUST be a non-empty string of at most 4000 characters after stripping leading and trailing whitespace; `originating_station_id`, when provided, MUST match `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$`. Validation failures MUST surface as `422 Unprocessable Entity` with a structured error body that names the offending field; they MUST NOT spawn a task, MUST NOT consume a `TaskRegistry` slot, and MUST NOT touch any audit log.

The endpoint SHALL require all of the following `app.state` slots populated before spawning: `kb_provider`, `kb_query_provider`, `kb_growth_logger_factory`, `llm_chat_provider`, `llm_judge_provider`. When any required slot is `None` or missing, the endpoint MUST respond with `503` and a body containing `code="QA_NOT_CONFIGURED"` whose `detail` field enumerates the missing slot names so operators can diagnose configuration gaps without reading sidecar logs. Returning `503` MUST NOT consume a `TaskRegistry` slot.

When validation passes and dependencies are populated, the endpoint SHALL allocate a `task_id` matching `^qa_[0-9a-f]{8}$`, register the task with the single-slot `TaskRegistry` (returning `409 TASK_IN_FLIGHT` when another task of any kind is currently in flight), spawn the Q&A coroutine via the same `_run_background_task` wrapper used by `/explore` and `/generate`, and respond with HTTP `202 Accepted` and body `{"task_id": "<qa_xxxxxxxx>"}`. The 202 status code MUST match the convention used by all other task-spawning endpoints (`/scan?stream=true` / `/kb/build` / `/explore` / `/generate`) so clients can apply uniform `if status === 202: subscribe to SSE` logic.

The Q&A coroutine SHALL drive `codebus_agent.agent.qa.run_qa(...)` to completion, route SSE events through the registered task subscriber channel, and surface failures via the `error` event with `code="QA_FAILED"` per the error containment Requirement.

#### Scenario: Empty question rejected

- **WHEN** `POST /qa` is called with body `{"workspace_root": "<valid>", "question": "", "originating_station_id": null}`
- **THEN** the response MUST be `422 Unprocessable Entity` referencing the `question` field
- **AND** no `task_id` MUST be allocated
- **AND** no `TaskRegistry` slot MUST be consumed

#### Scenario: Oversize question rejected

- **WHEN** `POST /qa` is called with body containing a `question` of 4001 characters
- **THEN** the response MUST be `422 Unprocessable Entity` referencing the maximum-length constraint

#### Scenario: Invalid originating_station_id rejected

- **WHEN** `POST /qa` is called with `originating_station_id="bad"`
- **THEN** the response MUST be `422 Unprocessable Entity` referencing the regex constraint

#### Scenario: Missing dependency yields 503 with detail listing missing slots

- **WHEN** `POST /qa` is called against an app where `kb_growth_logger_factory` is `None`
- **THEN** the response MUST be `503` with body `{"code": "QA_NOT_CONFIGURED", "detail": "...kb_growth_logger_factory..."}`
- **AND** the response detail MUST enumerate every missing slot, not just the first one encountered
- **AND** no `TaskRegistry` slot MUST be consumed

#### Scenario: Successful spawn returns task_id

- **WHEN** `POST /qa` is called with valid body and all dependencies present
- **THEN** the response MUST be `202` with body `{"task_id": "<qa_xxxxxxxx>"}` matching `^qa_[0-9a-f]{8}$`
- **AND** a single `TaskRegistry` slot MUST be occupied
- **AND** the background coroutine MUST begin emitting `rag_hits` followed by either `qa_answer` (success) or `error` (failure) on the task's SSE stream

#### Scenario: TaskRegistry single-slot blocks concurrent qa task

- **WHEN** a `qa` task is in flight
- **AND** a second `POST /qa` is issued
- **THEN** the second response MUST be `409 TASK_IN_FLIGHT`
- **AND** no second background coroutine MUST be spawned

#### Scenario: Question text never echoed in error path

- **WHEN** the Q&A coroutine raises an exception whose message contains the literal `question` string
- **THEN** the emitted `error` event's `message` field MUST NOT contain any substring of `question`
- **AND** the full exception MUST be visible in the sidecar standard logger only

---

### Requirement: Background task error containment

Background tasks spawned by the sidecar to serve `POST /scan?stream=true`, `POST /kb/build`, `POST /explore`, `POST /generate`, and `POST /qa` SHALL run inside a wrapper that catches all exceptions, emits a single `error` event of the form `{"type": "error", "code": "<safe_code>", "message": "<safe_message>"}` to all subscribers, transitions the task `status` to `"error"`, and then closes subscriber queues. The wrapper MUST NOT include exception class names, file paths, or stack traces in the emitted `code` or `message` fields. The full exception SHALL be written to the sidecar's standard logger only. Subscribers MUST always receive either a `done` event or an `error` event before the stream closes; an open subscriber stream MUST NOT be left in a state where neither terminal event has been delivered.

The error code table SHALL be predefined (not derived from exception classes) and MUST include the full closed set of ten codes: `"SCAN_FAILED"` (for `/scan?stream=true` failures), `"KB_BUILD_FAILED"` (for `/kb/build` failures), `"EXPLORE_FAILED"` (for `/explore` failures), `"GENERATE_FAILED"` (for `/generate` failures), `"QA_FAILED"` (for `/qa` failures), `"OPENAI_AUTH_FAILED"`, `"OPENAI_RATE_LIMITED"`, `"OPENAI_CONTEXT_EXCEEDED"`, `"KB_DIM_MISMATCH"`, and `"INTERNAL_ERROR"` (catch-all for unmapped exceptions). The code list is closed at the spec layer — the production frozenset `ERROR_CODES` in `sidecar/src/codebus_agent/api/tasks.py` MUST contain exactly these ten string literals, no more and no fewer. Any code path emitting a non-listed code is an invariant violation. Future task kinds MUST extend the table by Spectra change before any code emits the new code; bare uncategorised exception text MUST NOT leak through the SSE channel.

The historical alias `"KB_EMBED_FAILED"` (used during M2 development before this Requirement was tightened) MUST NOT appear in production code or tests. `review-2-critical-fix` (2026-04-26) renames all callsites to `"KB_BUILD_FAILED"` so the production frozenset matches this spec literally.

The wire `error` event payload MAY carry a narrow, code-specific set of typed extras alongside the mandatory `code` + `message` fields. The extras whitelist is defined per error code below; any other code MUST emit only `code` + `message` with no additional fields. Extras MUST be derived exclusively from typed attributes on the exception (e.g., `getattr(exc, "expected_dim", None)`) — never from `repr(exc)` / `str(exc)` / stack frames — so the operational invariant "no raw exception text on the wire" stays intact.

Extras whitelist:

- `"KB_DIM_MISMATCH"`: `expected_dim: int` (the embedding dimension recorded in the existing Qdrant collection), `actual_dim: int` (the dimension produced by the currently-configured embedding model), and `suggestion: str` (a fixed remediation hint, currently `"delete collection and rebuild"`). The two integer fields MUST appear only when the underlying exception exposes the corresponding typed attribute (`isinstance(value, int)` check); the `suggestion` field MUST always be present when the error code is `"KB_DIM_MISMATCH"` so the UI can always render a human-friendly remediation step.
- All other nine codes (`"SCAN_FAILED"` / `"KB_BUILD_FAILED"` / `"EXPLORE_FAILED"` / `"GENERATE_FAILED"` / `"QA_FAILED"` / `"OPENAI_AUTH_FAILED"` / `"OPENAI_RATE_LIMITED"` / `"OPENAI_CONTEXT_EXCEEDED"` / `"INTERNAL_ERROR"`): no extras MUST be added; the wire payload MUST contain exactly `{"type": "error", "code": ..., "message": ...}` and nothing else.

#### Scenario: Background task exception surfaces as safe error event

- **WHEN** a background scan task raises an exception while running
- **THEN** every active subscriber MUST receive an `error` event with `code` chosen from the predefined error code table (e.g. `"SCAN_FAILED"`) and a human-readable `message` that does not include `repr(exc)`, and the task `status` MUST become `"error"`

#### Scenario: Subscriber connecting after error still observes terminal event

- **WHEN** a subscriber connects to a task that has already transitioned to `"error"`
- **THEN** the stream MUST emit the previously stored `error` event and close, rather than hanging indefinitely

#### Scenario: Explore task exception surfaces as safe error event

- **WHEN** a background explore task (created via `POST /explore`) raises an exception while running
- **THEN** every active subscriber MUST receive an `error` event with `code="EXPLORE_FAILED"` and a human-readable `message` that does not include `repr(exc)`, and the task `status` MUST become `"error"`
- **AND** the wrapper MUST NOT differentiate Explorer-specific exceptions (token budget exhausted, sandbox violation, provider error) at the SSE-channel surface — the `code` MUST stay `"EXPLORE_FAILED"` and finer-grained classification stays in the sidecar standard logger

#### Scenario: Generate task exception surfaces as safe error event

- **WHEN** a background generate task (created via `POST /generate`) raises an unrecoverable exception while running (e.g., the inner LLM provider raises a 5xx, the workspace becomes unwritable, or `wire_kb_dependencies` slots are missing)
- **THEN** every active subscriber MUST receive an `error` event with `code="GENERATE_FAILED"` and a human-readable `message` that does not include `repr(exc)`, and the task `status` MUST become `"error"`
- **AND** the wrapper MUST NOT differentiate per-station validator failures (which are handled internally by the degraded-fallback Requirement in `module-5-generator`) at this top-level SSE-channel surface — degraded stations are a normal completion path with `done` event, not an `error` event

#### Scenario: QA task exception surfaces as safe error event

- **WHEN** a background Q&A task (created via `POST /qa`) raises an unrecoverable exception while running (e.g., the inner LLM provider raises a 5xx, the KB query backend becomes unreachable mid-run, or a tool call raises a sandbox violation that is not caught by the tool body)
- **THEN** every active subscriber MUST receive an `error` event with `code="QA_FAILED"` and a human-readable `message` that does not include `repr(exc)`, the task `status` MUST become `"error"`, and the `message` MUST NOT echo any portion of the user `question` or any chunk text that may have been in flight
- **AND** the wrapper MUST NOT differentiate Q&A-specific exception kinds (budget exhausted, sandbox violation, dedup error) at this top-level SSE-channel surface — the `code` MUST stay `"QA_FAILED"` and finer-grained classification stays in the sidecar standard logger

#### Scenario: KB build task exception surfaces as safe error event

- **WHEN** a background KB build task (created via `POST /kb/build`) raises an exception while running
- **THEN** every active subscriber MUST receive an `error` event with `code="KB_BUILD_FAILED"` (NOT the historical alias `"KB_EMBED_FAILED"`) and a human-readable `message` that does not include `repr(exc)`, and the task `status` MUST become `"error"`
- **AND** the production `ERROR_CODES` frozenset MUST contain `"KB_BUILD_FAILED"` and MUST NOT contain `"KB_EMBED_FAILED"`

#### Scenario: Error code table is exhaustively enumerated

- **WHEN** any test reads `sidecar/src/codebus_agent/api/tasks.py::ERROR_CODES`
- **THEN** the frozenset MUST equal exactly `{"SCAN_FAILED", "KB_BUILD_FAILED", "EXPLORE_FAILED", "GENERATE_FAILED", "QA_FAILED", "OPENAI_AUTH_FAILED", "OPENAI_RATE_LIMITED", "OPENAI_CONTEXT_EXCEEDED", "KB_DIM_MISMATCH", "INTERNAL_ERROR"}` — ten elements, no more, no fewer
- **AND** the docs/sidecar-api.md `§三-bis` ERROR_CODES table MUST list all ten codes with a short description for each

#### Scenario: KB_DIM_MISMATCH error event carries expected_dim, actual_dim, and suggestion extras

- **WHEN** a background task raises an exception that the classifier maps to `code="KB_DIM_MISMATCH"` and the underlying exception exposes integer-typed `expected_dim` and `actual_dim` attributes
- **THEN** the emitted wire `error` event MUST be `{"type": "error", "code": "KB_DIM_MISMATCH", "message": "<safe>", "expected_dim": <int>, "actual_dim": <int>, "suggestion": "delete collection and rebuild"}`
- **AND** the extras MUST be derived from the exception's typed attributes via `getattr(exc, "expected_dim", None)` / `getattr(exc, "actual_dim", None)`, NEVER from `repr(exc)` or `str(exc)`
- **AND** the `suggestion` field MUST be present even when `expected_dim` / `actual_dim` are absent (so the UI always has a remediation hint to render)

#### Scenario: Other error codes carry no extras beyond code and message

- **WHEN** a background task raises an exception that the classifier maps to any code other than `"KB_DIM_MISMATCH"` (e.g., `"SCAN_FAILED"` / `"OPENAI_RATE_LIMITED"` / `"INTERNAL_ERROR"`)
- **THEN** the emitted wire `error` event MUST contain exactly the keys `{"type", "code", "message"}` with no additional fields
- **AND** even when the underlying exception has typed attributes the wire payload MUST NOT include them — the extras whitelist is closed per error code
