## MODIFIED Requirements

### Requirement: task_id format

Task identifiers SHALL follow the format `{kind}_{rand}` where `kind` is one of the lowercase strings `"scan"`, `"kb"`, `"explore"`, or `"generate"` (extensible by future capabilities) and `rand` is exactly eight lowercase hexadecimal characters generated from a cryptographic random source (e.g. `secrets.token_hex(4)`). Identifiers MUST match the regular expression `^(scan|kb|explore|generate)_[0-9a-f]{8}$` for tasks created within scope of the `sse-progress-skeleton` change AND the `agent-sse-wiring` change that introduces the `explore` kind AND the `module-5-generator-p0` change that introduces the `generate` kind.

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

---

### Requirement: Background task error containment

Background tasks spawned by the sidecar to serve `POST /scan?stream=true`, `POST /kb/build`, `POST /explore`, and `POST /generate` SHALL run inside a wrapper that catches all exceptions, emits a single `error` event of the form `{"type": "error", "code": "<safe_code>", "message": "<safe_message>"}` to all subscribers, transitions the task `status` to `"error"`, and then closes subscriber queues. The wrapper MUST NOT include exception class names, file paths, or stack traces in the emitted `code` or `message` fields. The full exception SHALL be written to the sidecar's standard logger only. Subscribers MUST always receive either a `done` event or an `error` event before the stream closes; an open subscriber stream MUST NOT be left in a state where neither terminal event has been delivered.

The error code table SHALL be predefined (not derived from exception classes) and MUST include at minimum `"SCAN_FAILED"` (for `/scan?stream=true` failures), `"KB_BUILD_FAILED"` (for `/kb/build` failures), `"EXPLORE_FAILED"` (for `/explore` failures), and `"GENERATE_FAILED"` (for `/generate` failures). Future task kinds MUST extend the table when they are added; bare uncategorised exception text MUST NOT leak through the SSE channel.

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
