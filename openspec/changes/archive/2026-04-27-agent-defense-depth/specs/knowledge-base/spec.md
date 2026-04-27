## MODIFIED Requirements

### Requirement: POST /kb/build async endpoint

The sidecar SHALL expose `POST /kb/build` that accepts a JSON request body of the shape `{"workspace_root": "<absolute path>", "scan_result": <ScanResult JSON>}`. The endpoint MUST require the bearer token via the existing authentication middleware. On a successful request the endpoint SHALL create a `kind="kb"` task in the sidecar task registry, spawn a background coroutine that invokes `KnowledgeBase.build(scan_result, on_progress=<adapter>)`, return HTTP `202 Accepted` with body `{"task_id": "kb_<hex8>"}` immediately, and SHALL NOT block until the build completes. The 202 status code MUST match the convention used by all other task-spawning endpoints (`POST /scan` with stream=true / `POST /explore` / `POST /generate` / `POST /qa`) so clients can apply uniform `if status === 202: subscribe to SSE` logic. There SHALL NOT be a synchronous variant of `POST /kb/build` in this change. When the background build completes successfully the task handle's `result` MUST be set to the `KBStats` JSON returned by `build` and a `done` event MUST be emitted; when it raises, the error containment path defined by `sidecar-runtime` MUST apply.

#### Scenario: Successful request returns 202 with task_id immediately

- **WHEN** a client calls `POST /kb/build` with a valid bearer token and body `{"workspace_root": "<path>", "scan_result": {...}}` while no other task is in flight
- **THEN** the HTTP response status code MUST equal `202` (Accepted)
- **AND** the response body MUST match `{"task_id": "kb_<hex8>"}`
- **AND** the response MUST return within a small bounded latency (not blocked by KB build)

#### Scenario: Status code aligned with sibling task endpoints

- **WHEN** the sidecar test suite asserts the status code returned by each task-spawning endpoint (`POST /scan?stream=true`, `POST /kb/build`, `POST /explore`, `POST /generate`, `POST /qa`) on the success path
- **THEN** every endpoint in that set MUST return HTTP `202` (no endpoint MUST return `200` on the success path)

#### Scenario: Concurrent task in flight rejected with 409

- **WHEN** a client calls `POST /kb/build` while another task is currently `running` in the registry
- **THEN** the response MUST be HTTP `409` with body `{"code": "TASK_IN_FLIGHT", "running_task_id": "<id>"}` and no new background task MUST be started

#### Scenario: Done event makes KBStats reachable via result endpoint

- **WHEN** a client subscribes to `GET /tasks/{kb_task_id}/events` and the stream emits `done`
- **THEN** an immediately following `GET /tasks/{kb_task_id}/result` MUST return HTTP `200` with body equal to the `KBStats` JSON produced by the build
