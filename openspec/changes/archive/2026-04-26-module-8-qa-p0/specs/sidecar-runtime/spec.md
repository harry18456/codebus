## MODIFIED Requirements

### Requirement: task_id format

Task identifiers SHALL follow the format `{kind}_{rand}` where `kind` is one of the lowercase strings `"scan"`, `"kb"`, `"explore"`, `"generate"`, or `"qa"` (extensible by future capabilities) and `rand` is exactly eight lowercase hexadecimal characters generated from a cryptographic random source (e.g. `secrets.token_hex(4)`). Identifiers MUST match the regular expression `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$` for tasks created within scope of the `sse-progress-skeleton` change AND the `agent-sse-wiring` change that introduces the `explore` kind AND the `module-5-generator-p0` change that introduces the `generate` kind AND the `module-8-qa-p0` change that introduces the `qa` kind.

#### Scenario: Generated id matches required regex

- **WHEN** the sidecar creates a scan task identifier
- **THEN** the resulting `task_id` MUST match `^scan_[0-9a-f]{8}$`

#### Scenario: Explore kind follows same shape

- **WHEN** the sidecar creates an explore task identifier
- **THEN** the resulting `task_id` MUST match `^explore_[0-9a-f]{8}$`
- **AND** the `TaskRegistry` single-slot enforcement MUST apply equally — an in-flight `explore` task MUST block subsequent `scan` / `kb` / `explore` creations with `409 TASK_IN_FLIGHT`

#### Scenario: Generate kind follows same shape

- **WHEN** the sidecar creates a generate task identifier (via `POST /generate`)
- **THEN** the resulting `task_id` MUST match `^generate_[0-9a-f]{8}$`
- **AND** the `TaskRegistry` single-slot enforcement MUST apply equally — an in-flight `generate` task MUST block subsequent `scan` / `kb` / `explore` / `generate` creations with `409 TASK_IN_FLIGHT`, and an in-flight task of any other kind MUST block new `generate` task creation symmetrically

#### Scenario: QA kind follows same shape

- **WHEN** the sidecar creates a Q&A task identifier (via `POST /qa`)
- **THEN** the resulting `task_id` MUST match `^qa_[0-9a-f]{8}$`
- **AND** the `TaskRegistry` single-slot enforcement MUST apply equally — an in-flight `qa` task MUST block subsequent `scan` / `kb` / `explore` / `generate` / `qa` creations with `409 TASK_IN_FLIGHT`, and an in-flight task of any other kind MUST block new `qa` task creation symmetrically
- **AND** the `TaskKind` Literal enum used at the API surface MUST include `"qa"` so type checking blocks accidental drift

### Requirement: Background task error containment

Background tasks spawned by the sidecar to serve `POST /scan?stream=true`, `POST /kb/build`, `POST /explore`, `POST /generate`, and `POST /qa` SHALL run inside a wrapper that catches all exceptions, emits a single `error` event of the form `{"type": "error", "code": "<safe_code>", "message": "<safe_message>"}` to all subscribers, transitions the task `status` to `"error"`, and then closes subscriber queues. The wrapper MUST NOT include exception class names, file paths, or stack traces in the emitted `code` or `message` fields. The full exception SHALL be written to the sidecar's standard logger only. Subscribers MUST always receive either a `done` event or an `error` event before the stream closes; an open subscriber stream MUST NOT be left in a state where neither terminal event has been delivered.

The error code table SHALL be predefined (not derived from exception classes) and MUST include at minimum `"SCAN_FAILED"` (for `/scan?stream=true` failures), `"KB_BUILD_FAILED"` (for `/kb/build` failures), `"EXPLORE_FAILED"` (for `/explore` failures), `"GENERATE_FAILED"` (for `/generate` failures), and `"QA_FAILED"` (for `/qa` failures). Future task kinds MUST extend the table when they are added; bare uncategorised exception text MUST NOT leak through the SSE channel.

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

## ADDED Requirements

### Requirement: Q&A task spawn endpoint

The sidecar SHALL expose `POST /qa` whose request body is a Pydantic model with three fields: `workspace_root: str` (absolute path), `question: str`, and `originating_station_id: str | None`. The endpoint SHALL validate the request as follows before spawning any background task: `workspace_root` MUST resolve to an existing directory and MUST pass `ensure_in_workspace`-style validation (the same path-safety primitive used by `/scan` and `/explore`); `question` MUST be a non-empty string of at most 4000 characters after stripping leading and trailing whitespace; `originating_station_id`, when provided, MUST match `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$`. Validation failures MUST surface as `422 Unprocessable Entity` with a structured error body that names the offending field; they MUST NOT spawn a task, MUST NOT consume a `TaskRegistry` slot, and MUST NOT touch any audit log.

The endpoint SHALL require all of the following `app.state` slots populated before spawning: `kb_provider`, `kb_query_provider`, `kb_growth_logger_factory`, `llm_chat_provider`, `llm_judge_provider`. When any required slot is `None` or missing, the endpoint MUST respond with `503` and a body containing `code="QA_NOT_CONFIGURED"` whose `detail` field enumerates the missing slot names so operators can diagnose configuration gaps without reading sidecar logs. Returning `503` MUST NOT consume a `TaskRegistry` slot.

When validation passes and dependencies are populated, the endpoint SHALL allocate a `task_id` matching `^qa_[0-9a-f]{8}$`, register the task with the single-slot `TaskRegistry` (returning `409 TASK_IN_FLIGHT` when another task of any kind is currently in flight), spawn the Q&A coroutine via the same `_run_background_task` wrapper used by `/explore` and `/generate`, and respond synchronously with `{"task_id": "<qa_xxxxxxxx>"}`.

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
- **THEN** the response MUST be `200` with body `{"task_id": "<qa_xxxxxxxx>"}` matching `^qa_[0-9a-f]{8}$`
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
