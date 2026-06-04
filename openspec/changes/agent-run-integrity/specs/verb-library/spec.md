## MODIFIED Requirements

### Requirement: Sandbox Denial Signal Observability

The system SHALL surface, as a best-effort observability signal, the case where an agent provider exits zero at the top level even though an inner tool / shell command was blocked by the OS sandbox (notably codex `exec`, whose top-level process exit code is `0` even when an inner `command_execution` item reports `exit_code != 0` and `status: "failed"`, and whose denial message frequently surfaces only on the child process's stderr stream rather than as a stdout tool-result). The system SHALL accumulate this signal WITHOUT changing the run `outcome`: a detected denial SHALL NOT, in this change, flip `outcome` to `"failed"`.

The system SHALL provide a pure detector `stream::sandbox_signal::is_sandbox_denial(output: &str) -> bool` that returns `true` when the supplied text contains any marker from a curated set of locale-independent permission / sandbox denial markers (case-insensitive substring match). The marker set SHALL be chosen for high precision (markers that essentially never appear in benign command failures) over recall, and SHALL be validated against the captured codex sandbox PoC fixture so that a real OS-localized denial whose human-readable message is NOT English (observed: a Traditional-Chinese "access denied" message) is still detected via co-occurring locale-independent markers such as a .NET / shell error category token. The detector SHALL NOT treat an arbitrary non-zero inner exit as a denial.

`agent::invoke` SHALL apply the detector to BOTH of the following sources and SHALL accumulate a single `sandbox_denial_count` returned on `InvokeReport.sandbox_denial_count`: (a) `StreamEvent::ToolResult { output, is_error }` events whose `is_error == true` (incremented once per matching result, as before); AND (b) each line of the child process's stderr stream (incremented once per matching line). The stderr classification SHALL run regardless of the `CODEBUS_FORWARD_AGENT_STDERR` forwarding toggle — denial markers SHALL be counted whether or not stderr lines are forwarded to the parent terminal, and a stderr line that does NOT match SHALL retain its existing disposition (forwarded when the toggle is set, otherwise discarded). The two sources SHALL be summed; the system SHALL NOT attempt to de-duplicate a denial that surfaces on both stdout and stderr (the count is a best-effort signal, and an over-count caused by the same denial appearing on both sources SHALL NOT be treated as incorrect). Each `run_*` function SHALL copy `InvokeReport.sandbox_denial_count` into the written `RunLog.sandbox_denial_count` (per the `run-log` capability) AND, when that count is greater than `0`, SHALL emit exactly one stderr line prefixed with `warning: sandbox-denial` describing the count. The claude provider's distinct permission-denial concept (`permission_denials`) is explicitly OUT OF SCOPE for this requirement.

#### Scenario: Sandbox-denied inner command is counted

- **WHEN** `agent::invoke` processes a codex `ToolResult` whose `is_error == true` AND whose `output` contains a curated locale-independent denial marker (e.g., `PermissionDenied`)
- **THEN** `InvokeReport.sandbox_denial_count` SHALL be incremented by one for that result AND the written `RunLog.sandbox_denial_count` SHALL reflect the accumulated total AND a stderr line prefixed `warning: sandbox-denial` SHALL be emitted

#### Scenario: Stderr-only sandbox denial is counted

- **WHEN** `agent::invoke` processes a child whose stdout stream carries NO denial marker (no `ToolResult` with `is_error == true` matching a marker) but whose stderr stream contains a line carrying a curated locale-independent denial marker (the codex Windows top-level-exit-0 case)
- **THEN** `InvokeReport.sandbox_denial_count` SHALL be incremented by one for that stderr line AND a stderr line prefixed `warning: sandbox-denial` SHALL be emitted AND the run `outcome` SHALL remain unchanged by the denial

#### Scenario: Stderr denial is counted even when stderr forwarding is disabled

- **WHEN** `agent::invoke` runs with `CODEBUS_FORWARD_AGENT_STDERR` unset (child stderr is NOT forwarded to the parent terminal) AND a child stderr line contains a curated denial marker
- **THEN** `InvokeReport.sandbox_denial_count` SHALL still be incremented for that line (classification is independent of the forwarding toggle)

#### Scenario: Ordinary grep-no-match failure is NOT counted (false-positive guard)

- **WHEN** `agent::invoke` processes a `ToolResult` whose `is_error == true` (e.g., a `grep` that matched nothing and exited `1`) AND whose `output` contains NO curated denial marker
- **THEN** `InvokeReport.sandbox_denial_count` SHALL NOT be incremented for that result AND, absent any other denial on either source, the written `RunLog.sandbox_denial_count` SHALL equal `0` AND no `warning: sandbox-denial` stderr line SHALL be emitted

#### Scenario: Successful tool result is never scanned

- **WHEN** `agent::invoke` processes a `ToolResult` whose `is_error == false` even though its `output` coincidentally contains the substring `Access is denied`
- **THEN** `InvokeReport.sandbox_denial_count` SHALL NOT be incremented for that result (only `is_error == true` tool results are stdout-source candidates)

#### Scenario: Denial count does not change outcome

- **WHEN** a codex run finishes with top-level exit code `0` AND one inner command was sandbox-denied (so `sandbox_denial_count == 1`, from either the stdout or the stderr source)
- **THEN** the written `RunLog.outcome` SHALL remain `"succeeded"` (derived from the top-level zero exit) AND `RunLog.sandbox_denial_count` SHALL equal `1` (the denial is observable but orthogonal to outcome)

#### Scenario: Localized denial is still detected via locale-independent marker

- **WHEN** `is_sandbox_denial` is given the captured PoC output whose human-readable line is a non-English (Traditional Chinese) "access denied" message but which also contains the tokens `PermissionDenied` and `UnauthorizedAccessError`
- **THEN** the detector SHALL return `true` (it keys on the locale-independent tokens, not the localized phrasing)
