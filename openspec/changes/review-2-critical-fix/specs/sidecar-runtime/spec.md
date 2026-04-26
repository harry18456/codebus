## MODIFIED Requirements

### Requirement: Background task error containment

Background tasks spawned by the sidecar to serve `POST /scan?stream=true`, `POST /kb/build`, `POST /explore`, `POST /generate`, and `POST /qa` SHALL run inside a wrapper that catches all exceptions, emits a single `error` event of the form `{"type": "error", "code": "<safe_code>", "message": "<safe_message>"}` to all subscribers, transitions the task `status` to `"error"`, and then closes subscriber queues. The wrapper MUST NOT include exception class names, file paths, or stack traces in the emitted `code` or `message` fields. The full exception SHALL be written to the sidecar's standard logger only. Subscribers MUST always receive either a `done` event or an `error` event before the stream closes; an open subscriber stream MUST NOT be left in a state where neither terminal event has been delivered.

The error code table SHALL be predefined (not derived from exception classes) and MUST include the full closed set of ten codes: `"SCAN_FAILED"` (for `/scan?stream=true` failures), `"KB_BUILD_FAILED"` (for `/kb/build` failures), `"EXPLORE_FAILED"` (for `/explore` failures), `"GENERATE_FAILED"` (for `/generate` failures), `"QA_FAILED"` (for `/qa` failures), `"OPENAI_AUTH_FAILED"`, `"OPENAI_RATE_LIMITED"`, `"OPENAI_CONTEXT_EXCEEDED"`, `"KB_DIM_MISMATCH"`, and `"INTERNAL_ERROR"` (catch-all for unmapped exceptions). The code list is closed at the spec layer — the production frozenset `ERROR_CODES` in `sidecar/src/codebus_agent/api/tasks.py` MUST contain exactly these ten string literals, no more and no fewer. Any code path emitting a non-listed code is an invariant violation. Future task kinds MUST extend the table by Spectra change before any code emits the new code; bare uncategorised exception text MUST NOT leak through the SSE channel.

The historical alias `"KB_EMBED_FAILED"` (used during M2 development before this Requirement was tightened) MUST NOT appear in production code or tests. `review-2-critical-fix` (2026-04-26) renames all callsites to `"KB_BUILD_FAILED"` so the production frozenset matches this spec literally.

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
