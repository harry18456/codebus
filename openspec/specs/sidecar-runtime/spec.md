# sidecar-runtime Specification

## Purpose

TBD - created by archiving change 'm1-power-on'. Update Purpose after archive.

## Requirements

### Requirement: FastAPI sidecar binds ephemeral loopback port

The sidecar process SHALL start a FastAPI application bound exclusively to `127.0.0.1` on a port assigned by the operating system (ephemeral), per design decision D-local-1.

#### Scenario: Random port chosen at startup

- **WHEN** the sidecar process is launched twice in succession
- **THEN** each run MUST bind to a different ephemeral port number

#### Scenario: Not reachable from non-loopback interfaces

- **WHEN** a client on a non-loopback interface (any address other than `127.0.0.1` or `::1`) attempts to open a TCP connection to the sidecar port
- **THEN** the connection MUST fail to establish


<!-- @trace
source: m1-power-on
updated: 2026-04-19
-->

---
### Requirement: Bearer token authentication

All sidecar HTTP endpoints SHALL require a valid bearer token matching the startup-generated token, per design decision D-local-2 and `docs/sidecar-api.md §一`. The bearer token MUST be presented via the `Authorization: Bearer <token>` header. As a single, narrowly-scoped exception, the SSE events endpoint (path matching the regex `^/tasks/[^/]+/events$`) MAY also accept the bearer token via the `?bearer=<token>` query parameter, because browser-native `EventSource` cannot set custom request headers (this aligns the sidecar with the `frontend-shell` Requirement "useSseTask consumes bearer through useSidecar"). The query-parameter transport SHALL NOT be accepted on any other endpoint, ensuring the bearer never lands in access logs, browser history, or `Referer` headers for non-SSE traffic.

The token comparison SHALL use `secrets.compare_digest` regardless of which transport (header or query parameter) carried it, so timing-attack mitigation applies symmetrically.

#### Scenario: Missing bearer rejected

- **WHEN** a request arrives without an `Authorization` header AND (if the request targets a non-SSE path) without a `?bearer=` query parameter
- **THEN** the sidecar MUST respond with HTTP 401

#### Scenario: Wrong bearer rejected

- **WHEN** a request arrives with an `Authorization: Bearer` value that does not equal the startup token
- **THEN** the sidecar MUST respond with HTTP 401

#### Scenario: Correct bearer accepted

- **WHEN** a request arrives with the matching bearer token in the `Authorization` header
- **THEN** the sidecar MUST process the request and respond according to the endpoint's contract

#### Scenario: SSE events endpoint accepts bearer via query parameter

- **WHEN** a `GET /tasks/<task_id>/events` request arrives without an `Authorization` header but with `?bearer=<token>` matching the startup token
- **THEN** the sidecar MUST accept the request and proceed to stream the SSE event channel
- **AND** when both `Authorization` header and `?bearer=` query parameter are present and both match the startup token, the request MUST be accepted (header takes precedence; either valid transport satisfies the requirement)

#### Scenario: Non-SSE endpoints reject query-parameter bearer

- **WHEN** any HTTP request to a path NOT matching the regex `^/tasks/[^/]+/events$` arrives without an `Authorization` header but with `?bearer=<token>` matching the startup token (for example, `POST /scan?bearer=...`)
- **THEN** the sidecar MUST respond with HTTP 401
- **AND** the response MUST be indistinguishable from a request that omitted the bearer entirely (same status code, no leakage of which transport was attempted)

#### Scenario: Wrong bearer in query parameter rejected

- **WHEN** a `GET /tasks/<task_id>/events` request arrives with `?bearer=<token>` whose value does not equal the startup token
- **THEN** the sidecar MUST respond with HTTP 401


<!-- @trace
source: sidecar-sse-bearer-query-param-fallback
updated: 2026-05-03
code:
  - sidecar/src/codebus_agent/auth/__init__.py
  - web/dist
  - sidecar/src/codebus_agent/api/main.py
  - CLAUDE.md
tests:
  - sidecar/tests/auth/test_bearer_query_param.py
  - sidecar/tests/auth/test_access_log_invariant.py
-->

---
### Requirement: Health endpoint

The sidecar SHALL expose `GET /healthz` returning a JSON payload reflecting its readiness state. The response body SHALL include a top-level `status` field (`"ok"` or `"degraded"`) and a top-level `dependency` field carrying per-lane readiness (`llm_chat` / `llm_embed` / `pii` plus the existing infrastructure dependencies such as `qdrant`). Each lane MUST report one of `"ready"` / `"not-configured"` / `"unreachable"`.

A lane reports `not-configured` when the corresponding `llm.bindings.<role>.provider_id` resolves to a provider that has no API key in `app.state.provider_keys` (the in-memory dict populated by `POST /internal/startup-config`). A lane reports `unreachable` when the API key is present but a smoke check (e.g., embedding model availability for `llm_embed`, model list for `llm_chat`) fails. A lane reports `ready` only when both checks pass.

#### Scenario: Healthy state with all lanes ready

- **WHEN** `GET /healthz` is called with a valid bearer and every lane's smoke check succeeds
- **THEN** the response status MUST be 200
- **AND** the body MUST contain `{"status": "ok", "dependency": { "llm_chat": "ready", "llm_embed": "ready", "pii": "ready", ... }}`

#### Scenario: Degraded state with unreachable infrastructure dependency

- **WHEN** `GET /healthz` is called with a valid bearer and Qdrant is unreachable
- **THEN** the response status MUST be 200
- **AND** the body MUST contain `"status": "degraded"`
- **AND** the body MUST contain `dependency.qdrant: "unreachable"` and the rest of the lane keys per their actual state

#### Scenario: not-configured lane after fresh install

- **WHEN** `GET /healthz` is called with a valid bearer immediately after a sidecar boot where no `POST /internal/startup-config` was made
- **THEN** the response status MUST be 200
- **AND** the body MUST contain `dependency.llm_chat: "not-configured"`
- **AND** the body MUST contain `dependency.llm_embed: "not-configured"`

---
### Requirement: Handshake via stdout first line

At startup the sidecar SHALL emit a single-line JSON handshake to stdout so the parent Tauri process can discover the port and bearer token, per design decision D-local-1.

#### Scenario: Handshake line format

- **WHEN** the sidecar process starts
- **THEN** the first line written to stdout MUST be valid JSON containing the keys `port` (integer) and `bearer` (string of at least 32 characters)

#### Scenario: Parent reads handshake and succeeds ping

- **WHEN** the parent Tauri process reads the handshake line and issues `GET /healthz` with the supplied bearer against the supplied port
- **THEN** the response MUST be HTTP 200


<!-- @trace
source: m1-power-on
updated: 2026-04-19
-->

---
### Requirement: Parent-process watchdog

The sidecar SHALL self-terminate when its parent process disappears, so that orphaned sidecars do not keep loopback ports bound, per design decision D-local-2.

#### Scenario: Parent exits unexpectedly

- **WHEN** the parent process identified by `--parent-pid` exits
- **THEN** the sidecar MUST exit within five seconds and MUST release the bound port

<!-- @trace
source: m1-power-on
updated: 2026-04-19
-->

---
### Requirement: Sidecar startup remains available when Qdrant is unreachable

The sidecar entry point SHALL complete its startup sequence (bind ephemeral loopback port, emit stdout handshake, serve `GET /healthz`) even when the Qdrant URL it has been configured with is unreachable. A missing or unresponsive Qdrant MUST NOT cause the process to exit non-zero, block handshake emission, or prevent the bearer-authenticated HTTP server from accepting requests. This aligns with design decision D-009 (local-first) and D-027 (user-managed Qdrant binary).

#### Scenario: Sidecar starts with no Qdrant listener

- **WHEN** the sidecar is launched while no process is listening on the configured Qdrant URL
- **THEN** the handshake JSON line MUST still be emitted to stdout within the existing startup budget
- **AND** `GET /healthz` with a valid bearer MUST respond with HTTP 200 and body `{"status": "degraded", "dependencies": {"qdrant": {"ok": false, ...}}}`

#### Scenario: Sidecar startup not delayed waiting for Qdrant

- **WHEN** the sidecar is launched while no Qdrant listener exists
- **THEN** the time between process spawn and handshake emission MUST NOT be measurably increased by Qdrant-related probes (probe timeout MUST be bounded by one second and MUST NOT run during handshake emission)


<!-- @trace
source: qdrant-lifecycle-bootstrap
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/main.py
  - sidecar/src/codebus_agent/healthz.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/__init__.py
tests:
  - sidecar/tests/test_e2e_handshake.py
  - sidecar/tests/test_healthz_cli.py
  - sidecar/tests/kb/test_qdrant_client.py
  - sidecar/tests/kb/__init__.py
  - sidecar/tests/test_create_app.py
  - sidecar/tests/kb/test_no_direct_sdk_import.py
  - sidecar/tests/test_healthz.py
  - sidecar/tests/test_main_run.py
-->

---
### Requirement: Sidecar entry point wires Qdrant URL into app factory

The CLI entry point `codebus_agent.api.main:run` SHALL resolve the Qdrant base URL via `codebus_agent.kb.qdrant_client.resolve_url()` and pass the result to `codebus_agent.api.create_app` as the `qdrant_url` keyword argument, so that runtime `/healthz` reflects live Qdrant connectivity. When the CLI is invoked with `--healthz`, the same resolver MUST be used to pick the URL for the self-check.

#### Scenario: Environment variable propagates to runtime healthz

- **WHEN** the sidecar is launched with `CODEBUS_QDRANT_URL=http://custom.invalid:7000`
- **THEN** `GET /healthz` responses MUST include a `dependencies.qdrant` entry whose `detail` field reports `http://custom.invalid:7000`

#### Scenario: Default URL used when environment unset

- **WHEN** the sidecar is launched with `CODEBUS_QDRANT_URL` unset
- **THEN** `GET /healthz` responses MUST include a `dependencies.qdrant` entry whose `detail` field reports `http://127.0.0.1:6333`

#### Scenario: --healthz CLI shares the same resolver

- **WHEN** the sidecar is invoked with `--healthz` and `CODEBUS_QDRANT_URL=http://custom.invalid:7000`
- **THEN** the printed JSON line's `dependencies.qdrant.detail` MUST reference `http://custom.invalid:7000`

<!-- @trace
source: qdrant-lifecycle-bootstrap
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/main.py
  - sidecar/src/codebus_agent/healthz.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/__init__.py
tests:
  - sidecar/tests/test_e2e_handshake.py
  - sidecar/tests/test_healthz_cli.py
  - sidecar/tests/kb/test_qdrant_client.py
  - sidecar/tests/kb/__init__.py
  - sidecar/tests/test_create_app.py
  - sidecar/tests/kb/test_no_direct_sdk_import.py
  - sidecar/tests/test_healthz.py
  - sidecar/tests/test_main_run.py
-->

---
### Requirement: Workspace scan endpoint registration

The FastAPI sidecar SHALL register a `POST /scan` route that implements the `folder-scanner` capability. The route MUST be mounted under the same bearer-protected middleware as `/healthz`; it MUST NOT introduce a new authentication path, a new bind address, or bypass the ephemeral loopback constraint established by the bind-port requirement. The route's presence MUST NOT change the synchronous shape of `/healthz` or the stdout handshake.

#### Scenario: Scan route requires bearer token

- **WHEN** a client sends `POST /scan` without the `Authorization: Bearer <token>` header
- **THEN** the sidecar returns HTTP 401 and the scanner code path is not invoked.

#### Scenario: Scan route shares the loopback bind

- **WHEN** the sidecar starts and the stdout handshake prints `{"port": N, "bearer": "..."}`
- **THEN** both `/healthz` and `/scan` are reachable on `127.0.0.1:N` with the same bearer token, and neither endpoint is reachable on any non-loopback interface.

#### Scenario: Existing endpoints unchanged

- **WHEN** a client calls `GET /healthz` with a valid bearer token after `/scan` is registered
- **THEN** the response shape matches the existing Health endpoint contract (dependency statuses and overall status unchanged) and the response does not reference the scan endpoint.

---
### Requirement: Single-slot in-memory task registry

The sidecar SHALL maintain a single-slot in-memory task registry exposed on `app.state` that tracks at most one in-flight background task at a time, per `docs/sidecar-api.md §七` ("single FIFO queue"). The registry SHALL hold a `TaskHandle` whose fields include `id` (string), `kind` (one of `"scan"`, `"kb"`), `status` (one of `"running"`, `"done"`, `"error"`), an `asyncio.Queue` event channel per subscriber, and an optional terminal `result` payload. When any endpoint that creates a background task is invoked while the registry's current handle has `status == "running"`, the endpoint MUST reject the new request with HTTP `409 Conflict` and a JSON body `{"code": "TASK_IN_FLIGHT", "running_task_id": "<id>"}` and MUST NOT spawn a new background task. After a task transitions to `done` or `error`, its handle and result SHALL remain reachable via the registry until a subsequent successful task creation overwrites the slot.

#### Scenario: Second concurrent task rejected with 409

- **WHEN** a client successfully starts task A by calling `POST /scan?stream=true` and receives `{"task_id": "scan_..."}` while task A is still running, then immediately issues `POST /kb/build` against the same sidecar
- **THEN** the second request MUST return HTTP `409` with body `{"code": "TASK_IN_FLIGHT", "running_task_id": "scan_..."}` and MUST NOT have started a new background task

#### Scenario: Terminal handle survives until next task overwrites

- **WHEN** task A has emitted `done` and a client subsequently issues `GET /tasks/<task_a_id>/result` before any new task is created
- **THEN** the registry MUST still contain task A's handle and the endpoint MUST return its terminal payload


<!-- @trace
source: sse-progress-skeleton
updated: 2026-04-22
code:
  - sidecar/src/codebus_agent/scanner/models.py
  - CLAUDE.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/scanner/service.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/api/scan.py
  - docs/module-1-scanner.md
  - docs/module-2-kb-builder.md
  - docs/sidecar-api.md
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/api/__init__.py
  - sidecar/tests/scanner/test_fixtures_integration.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/api/test_task_error_containment.py
  - sidecar/tests/api/test_task_registry.py
  - sidecar/tests/api/test_task_result.py
  - sidecar/tests/scanner/test_service.py
  - sidecar/tests/api/test_tasks_sse.py
  - sidecar/tests/scanner/test_progress_callback.py
-->

---
### Requirement: SSE event stream endpoint

The sidecar SHALL expose `GET /tasks/{id}/events` returning a `text/event-stream` response per `docs/sidecar-api.md §四`. The endpoint MUST require the bearer token via the existing authentication middleware and MUST NOT be exempt from loopback binding. The response stream MUST emit only events whose `type` is one of `"progress"`, `"done"`, or `"error"` for changes scoped to this capability; other event types defined in the spec are reserved for follow-on changes and SHALL NOT be emitted by Module 1 or Module 2 task code paths in this change. Each event payload MUST be a single line of JSON terminated by the standard SSE `\n\n` separator. When a subscriber connects, the registry SHALL append a fresh `asyncio.Queue` to the handle's subscriber list and stream every subsequent emit to that queue; when the connection closes the queue MUST be removed from the list.

Each emitted event on the wire MUST include both an `event:` line and a `data:` line. The `event:` line value MUST equal the inner JSON's `type` field (e.g., `event: progress`, `event: done`, `event: error`). This enables HTML EventSource named listeners (`addEventListener("done", ...)`, etc.) on the client side to fire reliably; emitting only `data:` collapses every event into the browser's default `message` channel and silently breaks consumers that dispatch on `EventSource.addEventListener(<type>, ...)`.

#### Scenario: Stream emits progress, done, and final close

- **WHEN** a client subscribes to `GET /tasks/{id}/events` for a task that emits one progress event then completes
- **THEN** the stream MUST deliver the `progress` event followed by the `done` event in order, and the connection MUST close cleanly after the `done` event

#### Scenario: Stream rejects without bearer token

- **WHEN** a client connects to `GET /tasks/{id}/events` without a valid bearer token
- **THEN** the response MUST be HTTP `401` with no event-stream body produced

#### Scenario: Multiple subscribers receive identical event sequences

- **WHEN** two clients subscribe to the same task simultaneously and the task emits a sequence of three progress events followed by `done`
- **THEN** each client's stream MUST contain all four events in the same order, and one subscriber's disconnect MUST NOT affect the other

#### Scenario: Wire format includes both event and data lines per emission

- **WHEN** any task emits an event with `type` field set to `"progress"`, `"done"`, or `"error"` and a subscriber reads the raw HTTP response stream
- **THEN** the wire bytes for that event MUST contain both an `event: <type>` line and a `data: <json>` line, separated by `\r\n`, followed by the standard `\r\n\r\n` event terminator
- **AND** the `<type>` value on the `event:` line MUST exactly equal the `type` field inside the JSON on the `data:` line
- **AND** an event whose dict lacks an explicit `type` key MUST default the `event:` line value to `message`


<!-- @trace
source: sidecar-sse-named-events-and-error-listener-fix
updated: 2026-05-03
code:
  - web/app/components/workspace-onramp/FolderPickerButton.vue
  - web/app/components/workspace-onramp/OnrampProgress.vue
  - web/app/components/workspace-onramp/WorkspaceOnrampCard.vue
  - web/app/composables/useWorkspaceOnramp.ts
  - web/dist
  - web/app/utils/workspace-id.ts
  - web/app/components/AppShell.vue
  - tauri/src-tauri/src/lib.rs
  - tauri/src-tauri/Cargo.toml
  - web/package.json
  - tauri/src-tauri/capabilities/default.json
  - web/app/pages/index.vue
  - sidecar/src/codebus_agent/api/tasks.py
  - web/app/composables/useSseTask.ts
tests:
  - web/tests/utils/workspace-id.spec.ts
  - sidecar/tests/auth/test_workspace_id_parity.py
  - web/tests/onramp/WorkspaceOnrampCard.spec.ts
  - tauri/src-tauri/tests/dialog_plugin_smoke.rs
  - web/tests/onramp/FolderPickerButton.spec.ts
  - web/tests/onramp/useWorkspaceOnramp.spec.ts
  - web/tests/onboarding/index-page-redirect.spec.ts
  - web/tests/setup.ts
  - sidecar/tests/api/test_tasks_sse_wire_format.py
  - web/tests/onramp/OnrampProgress.spec.ts
  - web/tests/composables/useSseTask.connection-error.spec.ts
-->

---
### Requirement: Task result lookup endpoint

The sidecar SHALL expose `GET /tasks/{id}/result` that returns the terminal payload of a completed task. When the task's `status == "done"` the endpoint MUST return HTTP `200` with the task's stored result JSON. When the task's `status == "running"` the endpoint MUST return HTTP `409` with body `{"code": "TASK_NOT_DONE", "task_id": "<id>", "status": "running"}`. When no task with the given id exists in the registry the endpoint MUST return HTTP `404`. The endpoint MUST require the bearer token via the existing authentication middleware.

#### Scenario: Done task returns terminal payload

- **WHEN** a client calls `GET /tasks/{id}/result` for a task whose `status == "done"`
- **THEN** the response MUST be HTTP `200` with body equal to the payload that was stored when the task transitioned to `done`

#### Scenario: Running task rejected with 409

- **WHEN** a client calls `GET /tasks/{id}/result` for a task whose `status == "running"`
- **THEN** the response MUST be HTTP `409` with body containing `"code": "TASK_NOT_DONE"`

#### Scenario: Unknown task returns 404

- **WHEN** a client calls `GET /tasks/{id}/result` for a task id that has never been created
- **THEN** the response MUST be HTTP `404` and no result payload MUST be returned


<!-- @trace
source: sse-progress-skeleton
updated: 2026-04-22
code:
  - sidecar/src/codebus_agent/scanner/models.py
  - CLAUDE.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/scanner/service.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/api/scan.py
  - docs/module-1-scanner.md
  - docs/module-2-kb-builder.md
  - docs/sidecar-api.md
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/api/__init__.py
  - sidecar/tests/scanner/test_fixtures_integration.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/api/test_task_error_containment.py
  - sidecar/tests/api/test_task_registry.py
  - sidecar/tests/api/test_task_result.py
  - sidecar/tests/scanner/test_service.py
  - sidecar/tests/api/test_tasks_sse.py
  - sidecar/tests/scanner/test_progress_callback.py
-->

---
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


<!-- @trace
source: module-8-qa-p0
updated: 2026-04-26
code:
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/types.py
  - docs/sidecar-api.md
  - docs/decisions.md
  - sidecar/src/codebus_agent/agent/qa.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/agent/tools/qa_tools.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/__init__.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/prompts/qa.py
tests:
  - sidecar/tests/agent/tools/test_kb_search.py
  - sidecar/tests/kb/test_upsert_chunk.py
  - sidecar/tests/api/test_qa_sse_events.py
  - sidecar/tests/agent/test_qa_types.py
  - sidecar/tests/api/test_task_id_qa_kind.py
  - sidecar/tests/agent/tools/test_qa_tools.py
  - sidecar/tests/integration/__init__.py
  - sidecar/tests/kb/test_query_filter_stations.py
  - sidecar/tests/agent/test_qa_prompts.py
  - sidecar/tests/agent/test_hits_confident.py
  - sidecar/tests/agent/test_run_qa.py
  - sidecar/tests/api/test_audit_paths_kb_growth.py
  - sidecar/tests/kb/test_growth_logger.py
  - sidecar/tests/api/test_qa_endpoint.py
  - sidecar/tests/integration/test_qa_end_to_end.py
  - sidecar/tests/agent/test_qa_budget_constants.py
  - sidecar/tests/agent/tools/test_add_to_kb.py
  - sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py
-->

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


<!-- @trace
source: spec-cleanup-stage-5-batch-b
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - docs/sidecar-api.md
  - CLAUDE.md
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
-->

---
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


<!-- @trace
source: spec-cleanup-stage-5-batch-b
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - docs/sidecar-api.md
  - CLAUDE.md
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
-->

---
### Requirement: KB query endpoint registration

The sidecar's `create_app` SHALL register the `POST /kb/query` route on the same `kb_router` already used for `POST /kb/build`, behind the same bearer-token middleware. The endpoint MUST resolve its dependencies from `app.state` exactly like `POST /kb/build` does, but SHALL read `app.state.kb_query_provider` (the query-flavored TrackedProvider factory tagged with `default_module="kb_query"`) instead of `app.state.kb_provider` (the build-flavored factory tagged with `default_module="kb_build"`). This separation lets `token_usage.jsonl` distinguish embedding cost spent on building the KB versus querying it, without per-call `module=` plumbing in the endpoint.

#### Scenario: Both KB build and KB query slots present after wiring

- **WHEN** `create_app(...)` returns with `CODEBUS_OPENAI_API_KEY` set
- **THEN** `app.state.kb_provider` MUST be a callable factory and `app.state.kb_query_provider` MUST be a separate callable factory; invoking each with the same `workspace_root` MUST return distinct `TrackedProvider` instances whose `_default_module` values are `"kb_build"` and `"kb_query"` respectively

#### Scenario: Missing OpenAI API key leaves both provider slots None

- **WHEN** the sidecar starts without `CODEBUS_OPENAI_API_KEY`
- **THEN** both `app.state.kb_provider` and `app.state.kb_query_provider` MUST be `None`, and the sidecar MUST start successfully (the existing graceful-degrade contract)

#### Scenario: Bearer middleware blocks unauthenticated KB query

- **WHEN** a `POST /kb/query` request arrives without an `Authorization` header
- **THEN** the bearer middleware MUST short-circuit with `401` before the endpoint handler runs, mirroring the behavior verified for `POST /kb/build`

<!-- @trace
source: kb-query-endpoint
updated: 2026-04-23
code:
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/api/__init__.py
  - docs/sidecar-api.md
  - CLAUDE.md
  - docs/module-2-kb-builder.md
tests:
  - sidecar/tests/test_wire_kb_dependencies.py
  - sidecar/tests/api/test_kb_query.py
-->

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


<!-- @trace
source: spec-cleanup-stage-5-batch-b
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - docs/sidecar-api.md
  - CLAUDE.md
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
-->

---
### Requirement: Authorization endpoints registration

The sidecar SHALL register four synchronous HTTP endpoints under the `/auth/` route prefix from a dedicated router module `sidecar/src/codebus_agent/api/auth.py`. The router SHALL be included in the FastAPI app inside `create_app` after the bearer authentication middleware is installed, mirroring the pattern used by `/scan`, `/kb/*`, `/explore`, `/generate`, and `/qa` routers. The four endpoints (`POST /auth/grant`, `POST /auth/deny`, `POST /auth/revoke`, `GET /auth/status`) MUST be subject to the same bearer enforcement as all other sidecar endpoints; no auth endpoint MAY bypass bearer middleware.

The auth router SHALL NOT use the `TaskRegistry`, SHALL NOT spawn background tasks, and SHALL NOT extend the `task_id` regex. All auth endpoints return their result synchronously in the HTTP response body. Error responses SHALL use HTTP 4xx status codes with body shape `{"detail": {"code": "AUTH_*", "message": "<safe>"}}` where the `code` value comes from `sidecar/src/codebus_agent/auth/errors.py` module-level string constants. The auth error code constants MUST NOT be added to the SSE-channel `ERROR_CODES` frozenset in `api/tasks.py`; the two error code spaces are intentionally disjoint.

The app factory `create_app` SHALL accept a new optional `auth_audit_logger_factory: Callable[[], AuthorizationAuditLogger] | None = None` parameter and SHALL store it on `app.state.auth_audit_logger_factory`. The startup path (`main.py`) SHALL pass a default factory that constructs an `AuthorizationAuditLogger` pointing at the App-level audit log path (`~/.codebus/authorization_audit.jsonl`). When the factory is `None`, the four `/auth/*` endpoints MUST return HTTP 503 with `{"detail": {"code": "AUTH_NOT_CONFIGURED", "message": "..."}}` (the constant `AUTH_NOT_CONFIGURED` is added to `auth/errors.py` alongside the four primary codes); this stays parallel to the `*_NOT_CONFIGURED` pattern used by the KB/Explorer/Generator/QA endpoints.

#### Scenario: Auth router included in app factory

- **WHEN** `create_app(auth_audit_logger_factory=lambda: AuthorizationAuditLogger(...))` is called
- **THEN** the returned FastAPI app MUST have a route registered for each of the four paths: `POST /auth/grant`, `POST /auth/deny`, `POST /auth/revoke`, `GET /auth/status`
- **AND** each route MUST be subject to the bearer authentication dependency
- **AND** `app.state.auth_audit_logger_factory` MUST equal the factory passed in

#### Scenario: Auth endpoints return 503 when factory is None

- **WHEN** `create_app(auth_audit_logger_factory=None)` is called and `POST /auth/grant` is invoked with a valid bearer
- **THEN** the response MUST be HTTP 503 with body `{"detail": {"code": "AUTH_NOT_CONFIGURED", "message": "<safe>"}}`
- **AND** no audit log MUST be written
- **AND** no in-memory session MUST be created

#### Scenario: Auth endpoints reject missing bearer

- **WHEN** `POST /auth/grant` is called without an `Authorization` header
- **THEN** the response MUST be HTTP 401 (matching the bearer middleware behavior for all other endpoints)
- **AND** the response body MUST NOT include any auth-specific code (the response is the generic bearer-missing response, not an auth-flow-specific error)

#### Scenario: task_id regex unchanged

- **WHEN** the sidecar codebase is grepped for the regex literal in `tasks.py` defining the task_id format
- **THEN** the pattern MUST remain `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$` (no `auth` prefix added)
- **AND** any test asserting the regex MUST NOT be modified by this change

#### Scenario: Auth error codes disjoint from SSE ERROR_CODES frozenset

- **WHEN** the test suite imports `auth.errors.AUTH_WORKSPACE_INVALID`, `auth.errors.AUTH_NO_ACTIVE_GRANT`, `auth.errors.AUTH_INVALID_REQUEST`, `auth.errors.AUTH_NOT_CONFIGURED`, and `api.tasks.ERROR_CODES`
- **THEN** the intersection of the auth code set and the SSE frozenset MUST be empty
- **AND** the SSE `ERROR_CODES` frozenset MUST remain exactly the closed set of ten codes defined by the `Background task error containment` Requirement

<!-- @trace
source: auth-flow
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/auth.py
  - sidecar/src/codebus_agent/auth/service.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/main.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/auth/errors.py
  - web/app/pages/workspace/grant.vue
  - sidecar/src/codebus_agent/auth/audit_logger.py
  - sidecar/src/codebus_agent/api/__init__.py
  - web/app/composables/useAuthorization.ts
  - web/app/composables/useSidecar.ts
  - docs/authorization.md
  - sidecar/src/codebus_agent/auth/paths.py
  - sidecar/src/codebus_agent/api/auth.py
  - sidecar/src/codebus_agent/auth/__init__.py
  - docs/sidecar-api.md
  - docs/implementation-plan.md
  - web/app/components/auth/AuthorizationModal.vue
tests:
  - sidecar/tests/auth/test_error_codes_disjoint.py
  - sidecar/tests/auth/__init__.py
  - sidecar/tests/auth/test_audit_logger.py
  - sidecar/tests/auth/test_paths.py
  - sidecar/tests/auth/test_service.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/api/test_auth_endpoints.py
-->

---
### Requirement: Sidecar accepts provider config mutation endpoints

The sidecar SHALL register an internal mutation router that exposes the following bearer-authenticated endpoints used by the settings page:

- `GET /settings/providers` returns `{ providers: [...], bindings: {...}, pii_mode: "rule" | "llm" }` reflecting the in-memory snapshot. API keys MUST NOT appear in the response.
- `POST /settings/providers` upserts an entry in `llm.providers[]` (body schema `{ id, type, model, base_url }` — no `api_key`). On success the sidecar MUST emit SSE event `provider_config_changed` and persist the config (without API keys) to disk.
- `DELETE /settings/providers/{id}` removes an entry. Returns HTTP 409 with `code: PROVIDER_BOUND_TO_ROLE` and the bound role names if the provider id is referenced in `llm.bindings.<role>.provider_id` for any role.
- `PUT /settings/bindings` updates `llm.bindings` (body schema `{ reasoning, judge, chat, embed: <provider_id> }`). On success the sidecar MUST swap the active `RegistryHolder` reference atomically and emit `provider_config_changed`.
- `PUT /settings/pii-mode` updates `llm.pii.mode` (body schema `{ mode: "rule" | "llm", provider_id?: string }`). When `mode == "llm"` the `provider_id` MUST be present and MUST resolve to a provider type in `TrackedProvider.PII_ALLOWED_INNER_TYPES`; otherwise HTTP 400 with `code: INVALID_PII_PROVIDER`.

These endpoints MUST be guarded by the existing bearer middleware. They MUST NOT appear in the public OpenAPI document path entry list (set `include_in_schema=False`).

#### Scenario: Settings provider GET excludes API keys

- **WHEN** `GET /settings/providers` is called with a valid bearer
- **THEN** the response body MUST contain `providers[*]` entries
- **AND** no entry MUST contain an `api_key` field

#### Scenario: Settings binding PUT triggers RegistryHolder swap

- **WHEN** `PUT /settings/bindings` is called with a valid body
- **THEN** the in-memory `RegistryHolder` MUST be swapped to a new immutable `ProviderRegistry` instance reflecting the new bindings
- **AND** the sidecar MUST emit SSE event `provider_config_changed` on the app-level channel

#### Scenario: PII mode llm without provider_id rejected

- **WHEN** `PUT /settings/pii-mode` is called with body `{ mode: "llm" }` and no `provider_id`
- **THEN** the response status MUST be 400
- **AND** the body MUST contain `{ "detail": { "code": "INVALID_PII_PROVIDER" } }`

---
### Requirement: RegistryHolder enables atomic registry hot-swap

The sidecar SHALL provide a `RegistryHolder` class wrapping a single immutable `ProviderRegistry` reference. Code paths that retrieve providers MUST go through `holder.current()` which returns the registry reference under an `asyncio.Lock`. The holder MUST expose `swap(new_registry: ProviderRegistry)` which atomically replaces the current reference; in-flight callers that already hold a registry reference MUST continue using their captured reference until they finish, while subsequent `holder.current()` calls receive the new registry.

The holder MUST NOT mutate the inner `ProviderRegistry`; each swap creates a new immutable registry from the updated config snapshot.

#### Scenario: holder.current returns same instance until swap

- **WHEN** code calls `await holder.current()` twice in succession with no intervening `swap()`
- **THEN** the two calls MUST return the same `ProviderRegistry` instance (identity comparison)

#### Scenario: In-flight task continues with captured reference after swap

- **WHEN** an explorer task captures `holder.current()` and then a separate caller invokes `holder.swap(new_registry)`
- **THEN** the in-flight task's subsequent calls on its captured reference MUST continue using the old registry's providers
- **AND** a new task started after the swap MUST receive the new registry from `holder.current()`

#### Scenario: swap is atomic across concurrent reads

- **WHEN** N concurrent `await holder.current()` calls and one `holder.swap(new_registry)` call interleave
- **THEN** every concurrent caller MUST receive either the old or new registry — never a partially-constructed state — and the swap MUST complete in finite time

---
### Requirement: provider_config_changed SSE event surface

The sidecar SHALL broadcast SSE event `provider_config_changed` with payload `{ changed_roles: string[], embed_changed: boolean, providers_pool_changed: boolean }` whenever the provider pool, role bindings, or PII mode is mutated through the settings endpoints. The event MUST be emitted on an app-level SSE channel (not bound to a specific `task_id`) — sidecar SHALL register `GET /events?channel=app` for clients to subscribe; this channel MUST require a valid bearer.

The event MUST NOT carry any API key value, sensitive provider metadata (e.g., raw `base_url` is acceptable since it is not a secret), or any audit lane content. Multiple events MUST be coalesced when several mutations happen within a 50 ms window (single event with the union of changes).

#### Scenario: Binding change emits event with role list

- **WHEN** `PUT /settings/bindings` changes the `reasoning` and `chat` roles
- **THEN** the SSE channel `app` MUST receive exactly one event with `type: "provider_config_changed"` and `data.changed_roles == ["reasoning", "chat"]` (order-insensitive)

#### Scenario: Embed change sets embed_changed flag

- **WHEN** `PUT /settings/bindings` changes the `embed` role
- **THEN** the SSE event MUST include `data.embed_changed == true`

#### Scenario: Event carries no secrets

- **WHEN** the test inspects every emitted `provider_config_changed` event payload during a flow that mutates provider pool + bindings
- **THEN** the payload MUST NOT contain any `api_key` value
- **AND** the payload MUST NOT contain any `~/.codebus/` filesystem path

---
### Requirement: Config schema supports provider pool with role bindings

The sidecar config loader SHALL accept the new schema shape with `[[llm.providers]]` array entries (each carrying `id` / `type` / `model` / `base_url`) and a separate `[llm.bindings]` table mapping `reasoning` / `judge` / `chat` / `embed` roles to provider ids. The loader MUST also accept the legacy `[llm.roles]` shape and convert it into the new in-memory representation for backward compatibility, emitting a single deprecation warning per process start.

The loader MUST validate: (1) every binding's `provider_id` exists in the providers array, (2) the `embed` binding's referenced provider has `type` matching the embedding-shaped allowlist (`openai_embedding` and any future embedding type added to `TrackedProvider.ALLOWED_INNER_TYPES`), (3) `pii.mode == "llm"` requires `pii.provider_id` to reference a provider in the PII allowlist. Validation failures MUST raise `INVALID_PROVIDER_BINDING` / `INVALID_PROVIDER_TYPE` / `INVALID_PII_PROVIDER` at startup with the offending field name.

#### Scenario: New schema accepted

- **WHEN** the config file contains `[[llm.providers]]` entries plus `[llm.bindings]` table and the loader runs
- **THEN** the in-memory provider pool MUST contain entries matching every `[[llm.providers]]` block
- **AND** the in-memory bindings MUST match the `[llm.bindings]` table

#### Scenario: Legacy schema converted with deprecation warning

- **WHEN** the config file contains the legacy `[llm.roles.reasoning]` / `[llm.roles.embed]` shape
- **THEN** the loader MUST convert it into the new in-memory representation
- **AND** exactly one deprecation warning MUST be emitted to logs

#### Scenario: Binding referencing unknown provider rejected

- **WHEN** the config file contains `llm.bindings.reasoning = "does-not-exist"` with no matching provider id
- **THEN** the loader MUST raise an error with code `INVALID_PROVIDER_BINDING` and the offending role name in the message

#### Scenario: Embed binding to chat-typed provider rejected

- **WHEN** the config file contains `llm.bindings.embed = "openai-default"` and that provider's `type == "openai_chat"`
- **THEN** the loader MUST raise an error with code `INVALID_PROVIDER_TYPE`
