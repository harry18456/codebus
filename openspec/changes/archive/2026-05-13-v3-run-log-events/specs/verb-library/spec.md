## MODIFIED Requirements

### Requirement: Cancellation Signal Polling

The `agent::invoke` function SHALL accept a `cancel: Option<Arc<AtomicBool>>` parameter. When `cancel` is `Some(flag)`, the function SHALL read the flag with `Ordering::Relaxed` after processing each line read from the child's stdout. When the flag is observed as `true`, the function SHALL invoke `child.kill()` on the spawned child process, SHALL drain any remaining bytes from the child's stdout pipe on a best-effort basis (no further `on_event` invocations), SHALL `child.wait()` to reap the child, and SHALL return `Ok(InvokeReport { exit: <kill-state>, .. })` with `started_at` and `finished_at` populated as for a normal return. The function SHALL NOT panic if `child.kill()` fails (the child may have already exited between the poll and the kill call) — best-effort termination is the contract.

Each `run_*` function in `codebus_core::verb` SHALL, after `agent::invoke` returns, read the same `cancel` flag (if `Some`) and translate an observed-true value into `Err(VerbError::Cancelled)`. Before returning that `Err`, the verb function SHALL write one `RunLog` entry with `outcome: "cancelled"` to the configured log sink (per the `run-log` capability), reflecting the accumulated partial token usage and `wiki_changed` status as observed at the cancel point. The verb function SHALL skip `git::auto_commit` on this path per the existing `Auto-Commit Skipped On Cancel Or Error` requirement.

When `cancel` is `None`, the function SHALL behave exactly as before this change (no polling overhead beyond a single `None` discriminant check per event loop iteration).

#### Scenario: Cancel flag flipped mid-stream halts loop

- **WHEN** `agent::invoke` is invoked with `cancel: Some(flag)` AND the caller flips `flag` to `true` after the second stream line has been processed
- **THEN** the function SHALL invoke `child.kill()` no later than after processing the third line (one-line poll cadence) AND SHALL return `Ok(InvokeReport)` whose `exit.success()` SHALL be `false`

#### Scenario: Cancel propagates to VerbError::Cancelled and writes RunLog first

- **WHEN** `run_goal` is invoked with a cancel flag that is flipped during the agent spawn AND `agent::invoke` returns with a non-success exit due to the kill
- **THEN** `run_goal` SHALL invoke `LogSink::write_run` exactly once with a `RunLog` whose `outcome == "cancelled"` BEFORE returning `Err(VerbError::Cancelled)` AND SHALL NOT invoke `git::auto_commit`

#### Scenario: Cancel-path RunLog reflects partial state

- **WHEN** `run_goal` is cancelled after the agent emitted 3 `Thought` events AND 2 `ToolUse` events AND no `Usage` event yet
- **THEN** the written `RunLog.tokens.input_tokens` SHALL equal 0 (no Usage emitted) AND `RunLog.lint_error_count` SHALL equal 0 (no lint phase ran) AND `RunLog.wiki_changed` SHALL reflect the actual `git diff HEAD~1 wiki/` result at cancel time AND `RunLog.outcome` SHALL equal `"cancelled"`

---
### Requirement: Goal Verb Library Function

The system SHALL provide `codebus_core::verb::goal::run_goal` as a public function with the signature:

```
pub fn run_goal(
    repo: &Path,
    options: GoalOptions,
    on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<GoalReport, VerbError>
```

The function SHALL execute the full goal-verb orchestration in this order: vault precondition (auto-init when `<repo>/.codebus/` is missing), claude-code and lint-fix and pii config loading, source-signal drift detection, conditional raw mirror re-sync with PII scanner dispatch, `agent::invoke` spawn with `GOAL_TOOLSET` and the resolved verb config, optional fix loop invocation (skipped when `options.no_fix` is true), wiki-changed detection against the nested git repo HEAD, conditional `auto_commit` of the wiki on success, and RunLog field accumulation. The function SHALL NOT touch stdout or stderr directly; all renderable progress SHALL flow through `on_event`. The function SHALL NOT call `auto_commit` when the result is `Err(VerbError::Cancelled)`.

The function SHALL fan out the `on_event` callback so that each `VerbEvent` emitted is **also** persisted to the run's events.jsonl file via an internally-constructed `EventsSink` (per the `events-log` capability). The fan-out SHALL preserve the caller's view of the event stream (the caller closure SHALL be invoked exactly as before) AND additionally write one `EventEnvelope { ts, event }` to the events sink per emission. When `EventsSink::write_event` returns `Err`, the function SHALL emit a stderr warning prefixed with `warning: events-log` AND continue normal execution (per `events-log` `events.jsonl Write Failure Is Non-Fatal`).

The function SHALL write the final `RunLog` entry (per the `run-log` capability) with `outcome` set to `"succeeded"` on the normal success path, `"failed"` when the agent terminated non-zero and the verb propagates that failure, OR `"cancelled"` on the cancel path (cancel-path write happens before the `Err(VerbError::Cancelled)` return per `Cancellation Signal Polling`).

`GoalOptions` SHALL contain the fields `text: String`, `force_resync: bool`, `no_fix: bool`, and `no_obsidian_register: bool`. `GoalReport` SHALL contain the fields `accumulated_tokens: TokenUsage`, `wiki_changed: bool`, `lint_error_count: usize`, `lint_warn_count: usize`, `started_at: String` (RFC 3339 UTC), `finished_at: String` (RFC 3339 UTC), `agent_exit_code: Option<i32>`, AND `fix_post_lint_issues_remain: bool`. The `agent_exit_code` field carries the spawned goal agent's exit code so the CLI thin wrapper can apply the existing exit-code-precedence rule (agent failure preempts fix-phase outcome). The `fix_post_lint_issues_remain` field is `true` only when the post-spawn fix-and-lint phase terminated with `TerminationReason::PostLintIssuesRemain` AND `!report.clean`; the CLI thin wrapper uses it to emit the `✗ fix: N error(s), M warning(s) remain after agent terminated` stderr line at byte-equivalent timing.

#### Scenario: run_goal returns GoalReport on successful run

- **WHEN** `run_goal` is invoked with a valid vault AND the agent exits zero AND the fix loop reports zero errors
- **THEN** the function SHALL return `Ok(GoalReport)` AND `GoalReport.wiki_changed` SHALL be true when the nested git repo's `wiki/` tree differs from HEAD AND `auto_commit` SHALL have been invoked once with a `wiki: <goal-text>` message AND the appended `RunLog.outcome` SHALL equal `"succeeded"`

#### Scenario: run_goal auto-inits missing vault

- **WHEN** `run_goal` is invoked AND `<repo>/.codebus/` does not exist
- **THEN** the function SHALL invoke `vault::init::run_init` to create the vault AND proceed with the goal flow. The auto-init invocation SHALL pass a no-op `InitEvent` closure (`|_| {}`) — the library does NOT translate `InitEvent` variants into `VerbEvent` emissions in this change. CLI's direct `codebus init` invocation continues to emit init banners via `commands::init::run`; auto-init triggered inside `run_goal` runs silently. A future enhancement MAY add an InitEvent-to-VerbEvent adapter.

#### Scenario: run_goal short-circuits when fix is disabled

- **WHEN** `run_goal` is invoked with `GoalOptions { no_fix: true, .. }`
- **THEN** the function SHALL skip the fix loop step AND the returned `GoalReport.lint_error_count` and `GoalReport.lint_warn_count` SHALL reflect the pre-fix lint counts (or zero when no lint was run)

#### Scenario: run_goal cancel path writes cancelled RunLog and persists events.jsonl

- **WHEN** `run_goal` is invoked with a cancel flag AND the flag flips mid-stream
- **THEN** the function SHALL invoke `EventsSink::write_event` at least once before the cancel observation (the events.jsonl file SHALL contain the envelopes accumulated up to the cancel point) AND SHALL invoke `LogSink::write_run` exactly once with `RunLog.outcome == "cancelled"` AND THEN SHALL return `Err(VerbError::Cancelled)` AND `git::auto_commit` SHALL NOT have been invoked

---
### Requirement: Query Verb Library Function

The system SHALL provide `codebus_core::verb::query::run_query` as a public function with the signature:

```
pub fn run_query(
    repo: &Path,
    options: QueryOptions,
    on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<QueryReport, VerbError>
```

The function SHALL execute the full query-verb orchestration in this order: strict vault precondition (return `Err(VerbError::VaultMissing { path })` when `<repo>/.codebus/` is missing — query SHALL NOT auto-init), claude-code config loading, `agent::invoke` spawn with `QUERY_TOOLSET` (read-only: `Read`, `Glob`, `Grep`) and the resolved verb config, and RunLog field accumulation. The function SHALL NOT call `auto_commit` under any circumstance (query is read-only). The function SHALL NOT run the fix loop. The function SHALL NOT touch stdout or stderr directly; all renderable progress SHALL flow through `on_event`.

The function SHALL fan out the `on_event` callback to additionally persist each `VerbEvent` to the run's events.jsonl file via an internally-constructed `EventsSink` (per the `events-log` capability), with the same write-failure-non-fatal semantics as `run_goal`.

The function SHALL write the final `RunLog` entry with `outcome` set to `"succeeded"` on the normal success path (regardless of agent exit code — `query_propagates_agent_exit_code` test asserts CLI propagates the child exit but the RunLog row is still marked succeeded), OR `"cancelled"` on the cancel path (cancel-path write happens before the `Err(VerbError::Cancelled)` return per `Cancellation Signal Polling`).

`QueryOptions` SHALL contain the field `text: String`. `QueryReport` SHALL contain `accumulated_tokens: TokenUsage`, `started_at: String`, `finished_at: String`, AND `agent_exit_code: Option<i32>`. `QueryReport` SHALL NOT contain `wiki_changed` or any lint count field. The `agent_exit_code` field carries the spawned claude child's exit code so the CLI thin wrapper can propagate it as its own exit code (preserving `query_propagates_agent_exit_code` and other golden tests); `None` represents signal termination on platforms where the child died without an integer exit code.

#### Scenario: run_query refuses missing vault

- **WHEN** `run_query` is invoked AND `<repo>/.codebus/` does not exist
- **THEN** the function SHALL return `Err(VerbError::VaultMissing { path })` where `path` is the `<repo>/.codebus/` path AND SHALL NOT spawn `agent::invoke` AND SHALL NOT auto-init the vault

#### Scenario: run_query never auto-commits

- **WHEN** `run_query` runs to successful agent completion AND the agent has written files into the vault working tree
- **THEN** the function SHALL return `Ok(QueryReport)` AND SHALL NOT have invoked `auto_commit` AND the working tree SHALL retain any uncommitted writes (caller decides how to handle them)

#### Scenario: run_query records outcome succeeded with non-zero agent exit

- **WHEN** `run_query` is invoked AND the agent exits with non-zero exit code (e.g., agent crash mid-stream)
- **THEN** the function SHALL return `Ok(QueryReport)` with `agent_exit_code == Some(<non-zero>)` AND the appended `RunLog.outcome` SHALL equal `"succeeded"` (the verb itself completed even though the child crashed; CLI propagates the child exit via `agent_exit_code` field)

---
### Requirement: Fix Verb Library Function

The system SHALL provide `codebus_core::verb::fix::run_fix` as a public function with the signature:

```
pub fn run_fix(
    repo: &Path,
    options: FixOptions,
    on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<FixReport, VerbError>
```

The function SHALL execute the full fix-verb orchestration in this order: strict vault precondition (return `Err(VerbError::VaultMissing { path })` when `<repo>/.codebus/` is missing — fix SHALL NOT auto-init), `no_fix` short-circuit (return immediately when `options.no_fix` is true), lint pre-check (return immediately with zero-issue `FixReport` when no lint errors AND no warnings exist), `agent::invoke` spawn with `FIX_TOOLSET` and the resolved verb config, fix loop run, final lint pass, conditional `auto_commit` on success, and RunLog field accumulation. The function SHALL NOT call `auto_commit` when the result is `Err(VerbError::Cancelled)`. The function SHALL NOT touch stdout or stderr directly; all renderable progress SHALL flow through `on_event`.

The function SHALL fan out the `on_event` callback to additionally persist each `VerbEvent` to the run's events.jsonl file via an internally-constructed `EventsSink` (per the `events-log` capability), with the same write-failure-non-fatal semantics as `run_goal`.

The function SHALL write the final `RunLog` entry with `outcome` set to `"succeeded"` when termination is `PostLintClean` or `InitialClean`, `"failed"` when termination is `PostLintIssuesRemain`, OR `"cancelled"` on the cancel path. The `Skipped` status (no_fix flag or `lint.fix.enabled` false) SHALL NOT write a RunLog entry (no agent spawn occurred — there is nothing to record).

`FixOptions` SHALL contain the field `no_fix: bool`. `FixReport` SHALL contain `accumulated_tokens: TokenUsage`, `wiki_changed: bool`, `final_lint_error_count: usize`, `final_lint_warn_count: usize`, `fix_iterations: u8`, `started_at: Option<String>`, `finished_at: Option<String>`, AND `status: FixStatus`. The `started_at` / `finished_at` fields SHALL be `None` on the `Skipped` and `InitialClean` short-circuit paths (where no agent spawn occurred) and `Some(rfc3339)` on the post-agent paths.

`FixStatus` SHALL be a public enum with exactly four variants: `Skipped { reason: SkipReason }`, `InitialClean`, `PostLintClean`, `PostLintIssuesRemain`. `SkipReason` SHALL be a public enum with exactly two variants: `NoFixFlag` (the caller passed `FixOptions { no_fix: true }`) and `DisabledByConfig` (the loaded `lint.fix.enabled` was `false`). The CLI thin wrapper SHALL `match` exhaustively on `FixStatus` to emit the existing per-status stderr messages (`fix: disabled by --no-fix or lint.fix.enabled = false` for `Skipped`, `✗ fix: N error(s), M warning(s) remain after agent terminated` for `PostLintIssuesRemain`, no stderr for `InitialClean` and `PostLintClean`) and derive its own exit code (`Skipped` / `InitialClean` / `PostLintClean` → 0; `PostLintIssuesRemain` → 1).

#### Scenario: run_fix refuses missing vault

- **WHEN** `run_fix` is invoked AND `<repo>/.codebus/` does not exist
- **THEN** the function SHALL return `Err(VerbError::VaultMissing { path })` AND SHALL NOT spawn `agent::invoke`

#### Scenario: run_fix short-circuits on no_fix

- **WHEN** `run_fix` is invoked with `FixOptions { no_fix: true }`
- **THEN** the function SHALL return `Ok(FixReport)` with `fix_iterations == 0` AND `status == FixStatus::Skipped { reason: SkipReason::NoFixFlag }` AND `started_at == None` AND `finished_at == None` AND SHALL NOT spawn `agent::invoke` AND SHALL NOT run lint AND SHALL NOT write a RunLog entry

#### Scenario: run_fix short-circuits on clean lint pre-check

- **WHEN** `run_fix` is invoked AND the pre-check lint reports zero error count AND zero warn count
- **THEN** the function SHALL return `Ok(FixReport)` with `fix_iterations == 0`, `final_lint_error_count == 0`, `final_lint_warn_count == 0` AND `status == FixStatus::InitialClean` AND `started_at == None` AND `finished_at == None` AND SHALL NOT spawn `agent::invoke` AND SHALL NOT write a RunLog entry

#### Scenario: run_fix populates status PostLintClean on successful repair and writes RunLog succeeded

- **WHEN** `run_fix` runs to completion AND the post-spawn lint reports zero errors AND zero warnings
- **THEN** the function SHALL return `Ok(FixReport)` with `status == FixStatus::PostLintClean` AND `fix_iterations == 1` AND `started_at == Some(_)` AND `finished_at == Some(_)` AND the appended `RunLog.outcome` SHALL equal `"succeeded"`

#### Scenario: run_fix populates status PostLintIssuesRemain and writes RunLog failed

- **WHEN** `run_fix` runs to completion AND the post-spawn lint reports at least one error or warning
- **THEN** the function SHALL return `Ok(FixReport)` with `status == FixStatus::PostLintIssuesRemain` AND `fix_iterations == 1` AND the appended `RunLog.outcome` SHALL equal `"failed"`

#### Scenario: run_fix cancel path writes cancelled RunLog

- **WHEN** `run_fix` is invoked with a cancel flag AND the flag flips mid-stream
- **THEN** the function SHALL invoke `LogSink::write_run` exactly once with `RunLog.outcome == "cancelled"` BEFORE returning `Err(VerbError::Cancelled)` AND SHALL NOT invoke `git::auto_commit`
