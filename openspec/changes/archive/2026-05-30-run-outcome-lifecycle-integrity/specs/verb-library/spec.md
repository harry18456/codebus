## ADDED Requirements

### Requirement: Run Wall-Clock Timeout Safety Net

The `agent::invoke` function SHALL accept an additional `timeout: Option<Duration>` parameter, positioned as a sibling runtime control to the existing `cancel` parameter (both are caller-injected; neither is read from config by the library). When `timeout` is `None`, `agent::invoke` SHALL behave exactly as before this change — no wall-clock timer is started and the spawn / stream / reap path is byte-equivalent to the pre-change behavior.

When `timeout` is `Some(limit)`, `agent::invoke` SHALL capture a monotonic start instant immediately before spawning the child and SHALL extend the existing background watcher thread with a third check (in addition to the existing `done` and `cancel` checks): when the elapsed time since the start instant exceeds `limit`, the watcher SHALL terminate the child's entire process tree by calling the existing `KillHandle::terminate_tree()` (the same tree-kill mechanism the cancel path uses — this requirement SHALL NOT introduce a second kill mechanism). The resulting stdout EOF SHALL unblock the main read loop exactly as the cancel path does. The watcher SHALL still be joined before `agent::invoke` returns so it cannot outlive the child's PID slot.

`InvokeReport` SHALL carry a `timed_out: bool` field that is `true` if and only if the timeout watcher branch fired during the run. A timeout-induced kill SHALL NOT be distinguishable from any other kill by exit status alone, so `timed_out` is the authoritative signal callers SHALL use.

Each `run_*` function in `codebus_core::verb` SHALL accept a `timeout: Option<Duration>` parameter and forward it to `agent::invoke`. After `agent::invoke` returns, each `run_*` SHALL derive the run outcome in this precedence order: (1) when the `cancel` flag was observed `true`, the existing cancel path applies (`outcome == "cancelled"`, `interrupt_reason == Some(UserCancel)`); (2) otherwise, when `InvokeReport.timed_out == true`, the function SHALL write a `RunLog` with `outcome == "failed"` AND `interrupt_reason == Some(InterruptReason::Timeout)` (per the `run-log` capability) AND SHALL skip `git::auto_commit` per the existing `Auto-Commit Skipped On Cancel Or Error` requirement; (3) otherwise the existing exit-code-based derivation applies unchanged.

The per-run timeout limit SHALL originate from configuration under a top-level `lifecycle` section in `~/.codebus/config.yaml` with a single field `run_timeout_secs` (an unsigned integer number of seconds). The loader SHALL resolve a missing file, a missing `lifecycle` section, OR a missing `run_timeout_secs` field to `None` (no limit — preserving current behavior). A structurally invalid or wrong-typed value SHALL surface as `ConfigLoadError::YamlParse`, and the caller SHALL fall back to `None` after emitting a stderr warning, never silently shortening or lengthening a run. A `run_timeout_secs` value of `0` SHALL also resolve to `None` (no limit), NOT to a zero-duration timeout — a literal `0` would terminate the run instantly, so it is normalized to "unbounded" to avoid that foot-gun. The caller (CLI command / app IPC handler) SHALL convert the resolved `Option<u64>` seconds into an `Option<Duration>` and inject it into the `run_*` call; the verb library SHALL NOT read this config itself.

#### Scenario: Timeout fires terminate_tree and invoke returns before natural child exit

- **WHEN** `agent::invoke` is invoked with `timeout: Some(limit)` against a backend whose child runs far longer than `limit` and never exits on its own
- **THEN** the watcher SHALL call `KillHandle::terminate_tree()` after the elapsed time exceeds `limit` AND `agent::invoke` SHALL return well before the child's natural completion AND the returned `InvokeReport.exit.success()` SHALL be `false` AND `InvokeReport.timed_out` SHALL be `true`

#### Scenario: None timeout leaves invoke behavior unchanged

- **WHEN** `agent::invoke` is invoked with `timeout: None` against a finite child that emits three lines and exits
- **THEN** no wall-clock timer SHALL be started AND `agent::invoke` SHALL return after the child exits AND `InvokeReport.timed_out` SHALL be `false` AND `InvokeReport.exit.success()` SHALL be `true`

#### Scenario: Verb derives failed outcome and Timeout interrupt_reason on timeout

- **WHEN** `run_goal` is invoked with a `timeout` that elapses during the agent spawn AND the cancel flag was never flipped AND `agent::invoke` returns with `timed_out == true`
- **THEN** `run_goal` SHALL invoke `LogSink::write_run` exactly once with a `RunLog` whose `outcome == "failed"` AND `interrupt_reason == Some(InterruptReason::Timeout)` AND SHALL NOT invoke `git::auto_commit`

#### Scenario: Cancel takes precedence over timeout

- **WHEN** `run_goal` is invoked with both a cancel flag flipped to `true` AND an elapsed timeout such that `agent::invoke` returns `timed_out == true`
- **THEN** the written `RunLog.outcome` SHALL equal `"cancelled"` AND `RunLog.interrupt_reason` SHALL equal `Some(InterruptReason::UserCancel)` (cancel intent wins over the wall-clock timeout)

#### Scenario: lifecycle config absent resolves to no limit

- **WHEN** `~/.codebus/config.yaml` does not exist OR contains no `lifecycle` section OR the `lifecycle` section omits `run_timeout_secs`
- **THEN** the loader SHALL resolve `run_timeout_secs` to `None` AND callers SHALL inject `timeout: None` (preserving current unbounded behavior)

#### Scenario: lifecycle config honors an explicit timeout

- **WHEN** `~/.codebus/config.yaml` contains `lifecycle:\n  run_timeout_secs: 1800\n`
- **THEN** the loader SHALL resolve `run_timeout_secs` to `Some(1800)` AND the caller SHALL inject `timeout: Some(Duration::from_secs(1800))` into the `run_*` call

#### Scenario: Wrong-typed lifecycle timeout falls back to no limit

- **WHEN** `~/.codebus/config.yaml` contains `lifecycle:\n  run_timeout_secs: not-a-number\n`
- **THEN** the loader SHALL return `Err(ConfigLoadError::YamlParse(_))` AND the caller SHALL fall back to `timeout: None` after emitting a stderr warning (never silently applying a fabricated limit)

#### Scenario: Zero lifecycle timeout normalizes to no limit

- **WHEN** `~/.codebus/config.yaml` contains `lifecycle:\n  run_timeout_secs: 0\n`
- **THEN** the loader SHALL resolve `run_timeout_secs` to `None` (treated as no limit, not a zero-duration instant-kill timeout) AND the caller SHALL inject `timeout: None`

### Requirement: Sandbox Denial Signal Observability

The system SHALL surface, as a best-effort observability signal, the case where an agent provider exits zero at the top level even though an inner tool / shell command was blocked by the OS sandbox (notably codex `exec`, whose top-level process exit code is `0` even when an inner `command_execution` item reports `exit_code != 0` and `status: "failed"`). The system SHALL accumulate this signal WITHOUT changing the run `outcome`: a detected denial SHALL NOT, in this change, flip `outcome` to `"failed"`.

The system SHALL provide a pure detector `stream::sandbox_signal::is_sandbox_denial(output: &str) -> bool` that returns `true` when the supplied tool-result output contains any marker from a curated set of locale-independent permission / sandbox denial markers (case-insensitive substring match). The marker set SHALL be chosen for high precision (markers that essentially never appear in benign command failures) over recall, and SHALL be validated against the captured codex sandbox PoC fixture so that a real OS-localized denial whose human-readable message is NOT English (observed: a Traditional-Chinese "access denied" message) is still detected via co-occurring locale-independent markers such as a .NET / shell error category token. The detector SHALL NOT treat an arbitrary non-zero inner exit as a denial.

`agent::invoke` SHALL apply the detector ONLY to `StreamEvent::ToolResult { output, is_error }` events whose `is_error == true`, and SHALL accumulate a `sandbox_denial_count` (incremented once per matching result) that it returns on `InvokeReport.sandbox_denial_count`. Each `run_*` function SHALL copy `InvokeReport.sandbox_denial_count` into the written `RunLog.sandbox_denial_count` (per the `run-log` capability) AND, when that count is greater than `0`, SHALL emit exactly one stderr line prefixed with `warning: sandbox-denial` describing the count. The claude provider's distinct permission-denial concept (`permission_denials`) is explicitly OUT OF SCOPE for this requirement.

#### Scenario: Sandbox-denied inner command is counted

- **WHEN** `agent::invoke` processes a codex `ToolResult` whose `is_error == true` AND whose `output` contains a curated locale-independent denial marker (e.g., `PermissionDenied`)
- **THEN** `InvokeReport.sandbox_denial_count` SHALL be incremented by one for that result AND the written `RunLog.sandbox_denial_count` SHALL reflect the accumulated total AND a stderr line prefixed `warning: sandbox-denial` SHALL be emitted

#### Scenario: Ordinary grep-no-match failure is NOT counted (false-positive guard)

- **WHEN** `agent::invoke` processes a `ToolResult` whose `is_error == true` (e.g., a `grep` that matched nothing and exited `1`) AND whose `output` contains NO curated denial marker
- **THEN** `InvokeReport.sandbox_denial_count` SHALL NOT be incremented for that result AND, absent any other denial, the written `RunLog.sandbox_denial_count` SHALL equal `0` AND no `warning: sandbox-denial` stderr line SHALL be emitted

#### Scenario: Successful tool result is never scanned

- **WHEN** `agent::invoke` processes a `ToolResult` whose `is_error == false` even though its `output` coincidentally contains the substring `Access is denied`
- **THEN** `InvokeReport.sandbox_denial_count` SHALL NOT be incremented for that result (only `is_error == true` results are candidates)

#### Scenario: Denial count does not change outcome

- **WHEN** a codex run finishes with top-level exit code `0` AND one inner command was sandbox-denied (so `sandbox_denial_count == 1`)
- **THEN** the written `RunLog.outcome` SHALL remain `"succeeded"` (derived from the top-level zero exit) AND `RunLog.sandbox_denial_count` SHALL equal `1` (the denial is observable but orthogonal to outcome)

#### Scenario: Localized denial is still detected via locale-independent marker

- **WHEN** `is_sandbox_denial` is given the captured PoC output whose human-readable line is a non-English (Traditional Chinese) "access denied" message but which also contains the tokens `PermissionDenied` and `UnauthorizedAccessError`
- **THEN** the detector SHALL return `true` (it keys on the locale-independent tokens, not the localized phrasing)
