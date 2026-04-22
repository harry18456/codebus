## ADDED Requirements

### Requirement: Scanner progress callback hook

The scanner service `scan(...)` function SHALL accept an optional `on_progress` keyword argument typed as `ScannerProgressCallback`, defined as `Callable[[ScannerProgressEvent], Awaitable[None]] | None`. When `on_progress is None` the scanner MUST behave identically to the existing synchronous contract and MUST NOT introduce any await points beyond those already present. When `on_progress` is provided the scanner MUST emit at least one progress event during the directory walk phase and at least one progress event during the sanitizer Pass 1 phase. Each emitted `ScannerProgressEvent` MUST carry the fields `phase: Literal["walking", "sanitizing"]`, `current: int` (non-negative count of files processed so far in the phase), `total: int | None` (total expected count when known, `None` while still discovering), and `current_file: str | None` (path of the most recently processed file when applicable). The scanner MUST NOT emit progress events with negative counts or with `current > total` when `total` is not `None`.

#### Scenario: Synchronous call without callback preserves existing contract

- **WHEN** `scan(...)` is invoked without `on_progress`
- **THEN** the call MUST return a `ScanResult` synchronously and MUST NOT raise due to a missing callback

#### Scenario: Callback receives at least one event per phase

- **WHEN** `scan(...)` is invoked with an `on_progress` callback against a workspace containing at least three files
- **THEN** the callback MUST be awaited at least once with `phase == "walking"` and at least once with `phase == "sanitizing"` before `scan` returns

#### Scenario: Callback exception does not corrupt scan result

- **WHEN** the supplied `on_progress` callback raises during one of its invocations
- **THEN** the scanner MUST surface the exception to the caller without silently swallowing it, and the partially-built scan state MUST NOT leak into a returned `ScanResult`

### Requirement: POST /scan opt-in async streaming mode

The `POST /scan` endpoint SHALL preserve its existing synchronous contract when invoked without query parameters: it MUST return the full `ScanResult` JSON in a single response. When the request URL includes the query parameter `stream=true`, the endpoint SHALL instead create a `kind="scan"` task in the sidecar task registry, spawn a background coroutine that invokes `scan(..., on_progress=handle.emit)`, return HTTP `200` with body `{"task_id": "scan_<hex8>"}` immediately, and SHALL NOT block until the scan completes. The `?stream=true` path MUST translate every `ScannerProgressEvent` it receives from the callback into a wire event matching `docs/sidecar-api.md §四` `progress` schema with `phase: "scanning"` (collapsing the scanner's internal `walking`/`sanitizing` distinction). When the background scan completes successfully the task handle's `result` MUST be set to the full `ScanResult` JSON and a `done` event MUST be emitted; when it raises, the error containment path defined by `sidecar-runtime` MUST apply.

#### Scenario: Sync mode unchanged when stream query absent

- **WHEN** a client calls `POST /scan` without query parameters and a valid bearer token, body `{"workspace_type": "folder", "workspace_root": "<path>"}`
- **THEN** the response MUST be HTTP `200` containing the full `ScanResult` JSON, with no `task_id` field present

#### Scenario: Stream mode returns task_id and starts background work

- **WHEN** a client calls `POST /scan?stream=true` against the same workspace
- **THEN** the response MUST return immediately with body `{"task_id": "scan_<hex8>"}` and a subsequent `GET /tasks/<task_id>/events` subscription MUST eventually receive at least one `progress` event with `phase: "scanning"` followed by a `done` event

#### Scenario: Stream done event triggers result endpoint readiness

- **WHEN** a client subscribes to a stream-mode scan task and the stream emits `done`
- **THEN** an immediately following `GET /tasks/<task_id>/result` MUST return HTTP `200` with the full `ScanResult` JSON
