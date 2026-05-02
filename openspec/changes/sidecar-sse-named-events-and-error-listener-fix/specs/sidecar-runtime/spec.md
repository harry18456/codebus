## MODIFIED Requirements

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
