## ADDED Requirements

### Requirement: POST /kb/build async endpoint

The sidecar SHALL expose `POST /kb/build` that accepts a JSON request body of the shape `{"workspace_root": "<absolute path>", "scan_result": <ScanResult JSON>}`. The endpoint MUST require the bearer token via the existing authentication middleware. On a successful request the endpoint SHALL create a `kind="kb"` task in the sidecar task registry, spawn a background coroutine that invokes `KnowledgeBase.build(scan_result, on_progress=<adapter>)`, return HTTP `200` with body `{"task_id": "kb_<hex8>"}` immediately, and SHALL NOT block until the build completes. There SHALL NOT be a synchronous variant of `POST /kb/build` in this change. When the background build completes successfully the task handle's `result` MUST be set to the `KBStats` JSON returned by `build` and a `done` event MUST be emitted; when it raises, the error containment path defined by `sidecar-runtime` MUST apply.

#### Scenario: Successful request returns task_id immediately

- **WHEN** a client calls `POST /kb/build` with a valid bearer token and body `{"workspace_root": "<path>", "scan_result": {...}}` while no other task is in flight
- **THEN** the response MUST return within a small bounded latency (not blocked by KB build) with body matching `{"task_id": "kb_<hex8>"}`

#### Scenario: Concurrent task in flight rejected with 409

- **WHEN** a client calls `POST /kb/build` while another task is currently `running` in the registry
- **THEN** the response MUST be HTTP `409` with body `{"code": "TASK_IN_FLIGHT", "running_task_id": "<id>"}` and no new background task MUST be started

#### Scenario: Done event makes KBStats reachable via result endpoint

- **WHEN** a client subscribes to `GET /tasks/{kb_task_id}/events` and the stream emits `done`
- **THEN** an immediately following `GET /tasks/{kb_task_id}/result` MUST return HTTP `200` with body equal to the `KBStats` JSON produced by the build

### Requirement: KB progress phase translation to wire schema

The `POST /kb/build` background task SHALL adapt every `KBProgressEvent` produced by `KnowledgeBase.build` into a wire event matching `docs/sidecar-api.md §四` `progress` schema with the field `phase` set to the literal string `"embedding"` regardless of the source event's internal phase (`chunking`, `embedding`, `upserting`, `done`). The adapter SHALL guarantee that subscribers observe at least one `progress` event whose `current == 0` near the start of the build (corresponding to the chunking transition) and at least one `progress` event whose `current == total` near the end of the build (corresponding to the upserting transition), so the wire stream forms a monotonic 0 → total progression even when the underlying KB build phases are not equal-sized. The adapter SHALL NOT emit a wire `progress` event for the source `done` phase; the terminal transition MUST be emitted as the SSE `done` event by the task wrapper.

#### Scenario: Source done phase becomes wire done event

- **WHEN** `KnowledgeBase.build` emits a `KBProgressEvent` whose internal phase is `done`
- **THEN** the adapter MUST NOT translate it into a `progress` wire event; the task wrapper MUST emit the SSE `done` event after the build coroutine returns

#### Scenario: All non-done source phases collapse to embedding

- **WHEN** `KnowledgeBase.build` emits source events with internal phases `chunking`, `embedding`, and `upserting` during a single build
- **THEN** every wire `progress` event delivered to subscribers MUST have `phase == "embedding"`

#### Scenario: Wire stream is monotonic and reaches total

- **WHEN** a build produces N total chunks across a sequence of source events
- **THEN** the sequence of wire `progress` events delivered to a subscriber MUST contain at least one event with `current == 0` and at least one event with `current == N`, and the `current` values MUST be monotonically non-decreasing
