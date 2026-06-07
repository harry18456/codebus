## MODIFIED Requirements

### Requirement: Goal Verb Library Function

The system SHALL provide `codebus_core::verb::goal::run_goal` as a public function with the signature:

```
pub fn run_goal(
    repo: &Path,
    options: GoalOptions,
    on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
    timeout: Option<std::time::Duration>,
    run_started_at: Option<String>,
) -> Result<GoalReport, VerbError>
```

The `run_started_at` argument SHALL control the run's identity timestamp. When `Some(s)`, the function SHALL use the RFC 3339 UTC string `s` verbatim as the single source for BOTH the `events-*.jsonl` filename slug AND the persisted `RunLog.started_at` value, instead of sampling its own clock. When `None`, the function SHALL sample `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)` internally and use that for the same two derivations (this preserves the existing CLI invocation behavior, which has no cross-layer id join and passes `None`). The `timeout` argument SHALL bound the spawned agent run's wall-clock duration; `None` means no limit.

The function SHALL execute the full goal-verb orchestration in this order: vault precondition (auto-init when `<repo>/.codebus/` is missing), claude-code and lint-fix and pii config loading, source-signal drift detection, conditional raw mirror re-sync with PII scanner dispatch, `agent::invoke` spawn with `GOAL_TOOLSET` and the resolved verb config, optional fix loop invocation (skipped when `options.no_fix` is true), wiki-changed detection against the nested git repo HEAD, conditional `auto_commit` of the wiki on success, and RunLog field accumulation. The function SHALL NOT touch stdout or stderr directly; all renderable progress SHALL flow through `on_event`. The function SHALL NOT call `auto_commit` when the result is `Err(VerbError::Cancelled)`.

The function SHALL fan out the `on_event` callback so that each `VerbEvent` emitted is **also** persisted to the run's events.jsonl file via an internally-constructed `EventsSink` (per the `events-log` capability). The fan-out SHALL preserve the caller's view of the event stream (the caller closure SHALL be invoked exactly as before) AND additionally write one `EventEnvelope { ts, event }` to the events sink per emission. When `EventsSink::write_event` returns `Err`, the function SHALL emit a stderr warning prefixed with `warning: events-log` AND continue normal execution (per `events-log` `events.jsonl Write Failure Is Non-Fatal`).

The function SHALL write the final `RunLog` entry (per the `run-log` capability) with `outcome` set to `"succeeded"` on the normal success path, `"failed"` when the agent terminated non-zero and the verb propagates that failure, OR `"cancelled"` on the cancel path (cancel-path write happens before the `Err(VerbError::Cancelled)` return per `Cancellation Signal Polling`).

`GoalOptions` SHALL contain the fields `text: String`, `force_resync: bool`, `no_fix: bool`, and `no_obsidian_register: bool`. `GoalReport` SHALL contain the fields `accumulated_tokens: TokenUsage`, `wiki_changed: bool`, `lint_error_count: usize`, `lint_warn_count: usize`, `started_at: String` (RFC 3339 UTC), `finished_at: String` (RFC 3339 UTC), `agent_exit_code: Option<i32>`, AND `fix_post_lint_issues_remain: bool`. `GoalReport.started_at` SHALL equal the caller-provided `run_started_at` when `Some`, otherwise the internally sampled value. The `agent_exit_code` field carries the spawned goal agent's exit code so the CLI thin wrapper can apply the existing exit-code-precedence rule (agent failure preempts fix-phase outcome). The `fix_post_lint_issues_remain` field is `true` only when the post-spawn fix-and-lint phase terminated with `TerminationReason::PostLintIssuesRemain` AND `!report.clean`; the CLI thin wrapper uses it to emit the `✗ fix: N error(s), M warning(s) remain after agent terminated` stderr line at byte-equivalent timing.

#### Scenario: run_goal uses caller-provided run_started_at when Some

- **WHEN** `run_goal` is invoked with `run_started_at = Some("2026-05-13T14:56:21.123Z")` AND the run reaches a terminal outcome
- **THEN** the persisted events file SHALL be named `events-2026-05-13T14-56-21.123Z.jsonl` AND the appended `RunLog.started_at` SHALL equal `"2026-05-13T14:56:21.123Z"` AND `GoalReport.started_at` SHALL equal `"2026-05-13T14:56:21.123Z"`

#### Scenario: run_goal samples run_started_at internally when None

- **WHEN** `run_goal` is invoked with `run_started_at = None`
- **THEN** the function SHALL derive the run's started_at via `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)` AND use that single value for BOTH the events filename slug AND `RunLog.started_at`

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
