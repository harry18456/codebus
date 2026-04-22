## ADDED Requirements

### Requirement: Single-slot in-memory task registry

The sidecar SHALL maintain a single-slot in-memory task registry exposed on `app.state` that tracks at most one in-flight background task at a time, per `docs/sidecar-api.md §七` ("single FIFO queue"). The registry SHALL hold a `TaskHandle` whose fields include `id` (string), `kind` (one of `"scan"`, `"kb"`), `status` (one of `"running"`, `"done"`, `"error"`), an `asyncio.Queue` event channel per subscriber, and an optional terminal `result` payload. When any endpoint that creates a background task is invoked while the registry's current handle has `status == "running"`, the endpoint MUST reject the new request with HTTP `409 Conflict` and a JSON body `{"code": "TASK_IN_FLIGHT", "running_task_id": "<id>"}` and MUST NOT spawn a new background task. After a task transitions to `done` or `error`, its handle and result SHALL remain reachable via the registry until a subsequent successful task creation overwrites the slot.

#### Scenario: Second concurrent task rejected with 409

- **WHEN** a client successfully starts task A by calling `POST /scan?stream=true` and receives `{"task_id": "scan_..."}` while task A is still running, then immediately issues `POST /kb/build` against the same sidecar
- **THEN** the second request MUST return HTTP `409` with body `{"code": "TASK_IN_FLIGHT", "running_task_id": "scan_..."}` and MUST NOT have started a new background task

#### Scenario: Terminal handle survives until next task overwrites

- **WHEN** task A has emitted `done` and a client subsequently issues `GET /tasks/<task_a_id>/result` before any new task is created
- **THEN** the registry MUST still contain task A's handle and the endpoint MUST return its terminal payload

### Requirement: SSE event stream endpoint

The sidecar SHALL expose `GET /tasks/{id}/events` returning a `text/event-stream` response per `docs/sidecar-api.md §四`. The endpoint MUST require the bearer token via the existing authentication middleware and MUST NOT be exempt from loopback binding. The response stream MUST emit only events whose `type` is one of `"progress"`, `"done"`, or `"error"` for changes scoped to this capability; other event types defined in the spec are reserved for follow-on changes and SHALL NOT be emitted by Module 1 or Module 2 task code paths in this change. Each event payload MUST be a single line of JSON terminated by the standard SSE `\n\n` separator. When a subscriber connects, the registry SHALL append a fresh `asyncio.Queue` to the handle's subscriber list and stream every subsequent emit to that queue; when the connection closes the queue MUST be removed from the list.

#### Scenario: Stream emits progress, done, and final close

- **WHEN** a client subscribes to `GET /tasks/{id}/events` for a task that emits one progress event then completes
- **THEN** the stream MUST deliver the `progress` event followed by the `done` event in order, and the connection MUST close cleanly after the `done` event

#### Scenario: Stream rejects without bearer token

- **WHEN** a client connects to `GET /tasks/{id}/events` without a valid bearer token
- **THEN** the response MUST be HTTP `401` with no event-stream body produced

#### Scenario: Multiple subscribers receive identical event sequences

- **WHEN** two clients subscribe to the same task simultaneously and the task emits a sequence of three progress events followed by `done`
- **THEN** each client's stream MUST contain all four events in the same order, and one subscriber's disconnect MUST NOT affect the other

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

### Requirement: task_id format

Task identifiers SHALL follow the format `{kind}_{rand}` where `kind` is one of the lowercase strings `"scan"` or `"kb"` (extensible by future capabilities) and `rand` is exactly eight lowercase hexadecimal characters generated from a cryptographic random source (e.g. `secrets.token_hex(4)`). Identifiers MUST match the regular expression `^(scan|kb)_[0-9a-f]{8}$` for tasks created within this change's scope.

#### Scenario: Generated id matches required regex

- **WHEN** the sidecar creates a scan task identifier
- **THEN** the resulting `task_id` MUST match `^scan_[0-9a-f]{8}$`

### Requirement: Background task error containment

Background tasks spawned by the sidecar to serve `POST /scan?stream=true` and `POST /kb/build` SHALL run inside a wrapper that catches all exceptions, emits a single `error` event of the form `{"type": "error", "code": "<safe_code>", "message": "<safe_message>"}` to all subscribers, transitions the task `status` to `"error"`, and then closes subscriber queues. The wrapper MUST NOT include exception class names, file paths, or stack traces in the emitted `code` or `message` fields. The full exception SHALL be written to the sidecar's standard logger only. Subscribers MUST always receive either a `done` event or an `error` event before the stream closes; an open subscriber stream MUST NOT be left in a state where neither terminal event has been delivered.

#### Scenario: Background task exception surfaces as safe error event

- **WHEN** a background scan task raises an exception while running
- **THEN** every active subscriber MUST receive an `error` event with `code` chosen from the predefined error code table (e.g. `"SCAN_FAILED"`) and a human-readable `message` that does not include `repr(exc)`, and the task `status` MUST become `"error"`

#### Scenario: Subscriber connecting after error still observes terminal event

- **WHEN** a subscriber connects to a task that has already transitioned to `"error"`
- **THEN** the stream MUST emit the previously stored `error` event and close, rather than hanging indefinitely
