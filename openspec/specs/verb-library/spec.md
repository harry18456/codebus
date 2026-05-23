# verb-library Specification

## Purpose

TBD - created by archiving change 'v3-goal-library'. Update Purpose after archive.

## Requirements

### Requirement: Verb Library Module Surface

The system SHALL expose a public module `codebus_core::verb` containing five sub-modules `goal`, `query`, `fix`, `chat`, and `quiz`. The `goal`, `query`, `fix`, and `chat` sub-modules SHALL each export exactly one public orchestration function (`run_goal`, `run_query`, `run_fix`, `run_chat_turn`) plus the verb-specific options and report structs. The `quiz` sub-module SHALL export exactly **two** public orchestration functions — `run_quiz_plan` and `run_quiz_generate` — plus its option/report structs; this is the documented exception to the one-function rule, required because the GUI confirm gate (`app-workspace` Quiz Tab Plan-Confirm-Generate Flow, design D1) demands the plan and generate spawns be separately invokable and a single connected call cannot pause mid-flight for an asynchronous confirmation. The `codebus_core::verb` parent module SHALL also export the cross-verb types `VerbEvent`, `VerbLifecycleEvent`, and `VerbError`. No other public surface SHALL be exposed under `codebus_core::verb` by this change. The `codebus_core::vault::init::run_init` function defined by foundation SHALL remain in its existing location and SHALL NOT be moved into `codebus_core::verb`.

#### Scenario: Verb library module path exists

- **WHEN** a downstream crate (codebus-cli or codebus-app) writes `use codebus_core::verb::{goal, query, fix, chat, quiz};`
- **THEN** the compilation SHALL succeed AND the five sub-modules SHALL resolve to public modules (goal/query/fix/chat each exporting one orchestration function; quiz exporting `run_quiz_plan` + `run_quiz_generate`)

#### Scenario: Init verb is not moved

- **WHEN** a downstream crate writes `use codebus_core::verb::init;`
- **THEN** the compilation SHALL fail (no such module) AND init orchestration SHALL remain accessible only via `codebus_core::vault::init::run_init`

#### Scenario: Chat sub-module exports run_chat_turn

- **WHEN** a downstream crate writes `use codebus_core::verb::chat::{run_chat_turn, ChatTurnOptions, ChatTurnReport, CHAT_TOOLSET};`
- **THEN** the compilation SHALL succeed AND `run_chat_turn` SHALL resolve to a function with the signature defined by the `chat-verb` capability

#### Scenario: Quiz sub-module exports plan and generate functions

- **WHEN** a downstream crate writes `use codebus_core::verb::quiz::{run_quiz_plan, run_quiz_generate, QuizPlanOptions, QuizGenerateOptions, QuizPlanOutcome, QuizReport};`
- **THEN** the compilation SHALL succeed AND `run_quiz_plan` / `run_quiz_generate` SHALL resolve to functions with the signatures defined by the `quiz` capability


<!-- @trace
source: v3-app-quiz
updated: 2026-05-16
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/spike-artifacts/quiz-fixture-vault/manifest.yaml
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/jwt-token-lifecycle.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/index.md
  - docs/spike-artifacts/spike-quiz-7-F5.jsonl
  - codebus-app/src-tauri/src/ipc/quiz.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/config/quiz.rs
  - docs/spike-artifacts/spike-quiz-7-F1.jsonl
  - codebus-app/src-tauri/src/ipc/config.rs
  - docs/2026-05-15-v3-app-quiz-spike-plan.md
  - docs/spike-artifacts/spike-quiz-7-F6.jsonl
  - docs/spike-artifacts/spike-quiz-8-E3.jsonl
  - docs/spike-artifacts/spike-quiz-9-S1.jsonl
  - codebus-core/src/verb/quiz.rs
  - docs/v3-app-roadmap.md
  - codebus-cli/src/commands/mod.rs
  - codebus-core/src/config/claude_code.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run2.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC1.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/user-store.md
  - docs/spike-artifacts/spike-quiz-10-R1-run1.jsonl
  - codebus-app/src-tauri/src/config.rs
  - codebus-app/src/lib/quiz-parse.ts
  - codebus-core/src/skill_bundle/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/wiki/log.md
  - docs/spike-artifacts/spike-quiz-7-F2.jsonl
  - docs/spike-artifacts/spike-quiz-8-E4.jsonl
  - codebus-app/src/components/workspace/QuizAnswering.tsx
  - docs/2026-05-15-v3-app-quiz-discussion.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/session-vs-token.md
  - docs/spike-artifacts/spike-quiz-8-E5.jsonl
  - codebus-cli/src/commands/quiz.rs
  - docs/spike-artifacts/spike-quiz-9-S3.jsonl
  - codebus-core/src/config/mod.rs
  - codebus-core/src/log/events/sink.rs
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - docs/spike-artifacts/spike-quiz-runbook.md
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/verb/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/CLAUDE.md
  - codebus-core/src/verb/event.rs
  - codebus-app/src/components/settings/SettingsModal.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - docs/spike-artifacts/spike-quiz-8-E2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/raw/code/auth.py
  - docs/spike-artifacts/spike-quiz-8-E1.jsonl
  - docs/spike-artifacts/spike-quiz-7-F3.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/.claude/skills/codebus-quiz/SKILL.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/auth-middleware.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/processes/login-flow.md
  - docs/spike-artifacts/spike-quiz-9-S2.jsonl
  - codebus-core/src/vault/source_gitignore.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run3.jsonl
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/spike-artifacts/spike-quiz-7-F4.jsonl
tests:
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-core/tests/vault_init.rs
  - codebus-cli/tests/bins/mock_claude.rs
  - codebus-cli/tests/quiz_flow.rs
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-core/tests/verb_library_surface.rs
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-app/src/components/workspace/QuizAnswering.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-cli/tests/cli_routing.rs
-->

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

---
### Requirement: Verb Event Enum

The system SHALL define `codebus_core::verb::VerbEvent` as a public enum with exactly three variants:

```
pub enum VerbEvent {
    Banner(VerbBanner),
    Stream(StreamEvent),
    Lifecycle(VerbLifecycleEvent),
}
```

`VerbBanner` SHALL be a new owning enum in `codebus_core::verb::event` that mirrors `codebus_core::render::Banner<'a>` but with owned fields (`PathBuf` instead of `&Path`, `String` instead of `&str`). The variant set SHALL match `Banner` exactly: `Start { repo_path: PathBuf }`, `Goal { goal_text: String }`, `SyncStart`, `SyncDone { files, mib, elapsed_ms }`, `PiiSummary { scanner: String, scanned, hits, action: String }`, `LintStart`, `LintDone { errors, warns, elapsed_ms }`, `CommitDone { sha7: String }`, `Done { wiki_path: PathBuf }`, `Hint { wiki_path: PathBuf }`. `VerbBanner` SHALL implement an `as_banner(&self) -> Banner<'_>` method that borrows the owning fields back into a `Banner<'_>` so CLI thin wrappers can reuse the existing `print_banner` renderer without duplicating formatting logic. The owning representation is required because `VerbEvent` flows through `impl FnMut(VerbEvent)` closures that must be `'static + Send` for cross-thread use (GUI Tauri event emit); the borrowed `Banner<'a>` cannot satisfy that constraint.

`StreamEvent` SHALL be the existing `codebus_core::stream::StreamEvent` from the `agent-stream-rendering` capability.

`VerbLifecycleEvent` SHALL be a public enum with at minimum these variants: `SpawnStart { verb: Verb }`, `SpawnEnd { verb: Verb, exit_code: Option<i32> }`, `FixIterationStart { iteration: u8 }`, `LintFinal { error_count: usize, warn_count: usize }`, AND `PromoteSuggestion { reason: String }`. The `PromoteSuggestion` variant SHALL be emitted exclusively by `verb::chat::run_chat_turn` when its stream parser detects the chat promote-suggestion line marker per the `chat-verb` capability — `run_goal`, `run_query`, and `run_fix` SHALL NOT emit this variant. Additional lifecycle variants MAY be added by future changes following minor-version semantics; downstream pattern matches SHALL be required to use a non-exhaustive marker or wildcard arm.

Each `run_*` function SHALL invoke `on_event` exactly once for each banner step it would have printed in its CLI form (wrapping each `Banner::*` as `VerbEvent::Banner(VerbBanner::*)`), exactly once for each `StreamEvent` produced by the underlying `agent::invoke` (wrapping each as `VerbEvent::Stream(...)`), and at appropriate lifecycle boundaries (wrapping each as `VerbEvent::Lifecycle(...)`).

#### Scenario: Banner events flow through on_event

- **WHEN** `run_goal` reaches the step where the CLI form would print `Banner::Start { repo_path }`
- **THEN** `on_event` SHALL be invoked with `VerbEvent::Banner(VerbBanner::Start { repo_path: PathBuf::from(repo) })` AND no direct stdout write SHALL occur from the library function

#### Scenario: VerbBanner as_banner round-trips into Banner renderer

- **WHEN** the CLI thin wrapper receives `VerbEvent::Banner(vb)` AND calls `print_banner(vb.as_banner(), &render_opts)`
- **THEN** the rendered stdout byte sequence SHALL equal what `print_banner(Banner::*, &render_opts)` would have produced when the equivalent borrowed `Banner<'a>` is built from the same field values (byte-equivalent CLI output)

#### Scenario: Stream events flow through on_event

- **WHEN** `agent::invoke` (called by `run_goal`) parses a stream-json line yielding `StreamEvent::ToolUse { name: "Read", input: ... }`
- **THEN** `on_event` SHALL be invoked with `VerbEvent::Stream(StreamEvent::ToolUse { name: "Read", input: ... })` AND no `print_event` call SHALL occur inside the library function

#### Scenario: Lifecycle events bracket spawn

- **WHEN** `run_query` is about to call `agent::invoke`
- **THEN** `on_event` SHALL be invoked with `VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart { verb: Verb::Query })` immediately before the spawn AND `VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd { verb: Verb::Query, exit_code: <exit-code> })` immediately after the child wait returns

#### Scenario: PromoteSuggestion emitted only by chat verb

- **WHEN** `run_chat_turn` is invoked AND the agent's assistant message begins with the chat promote-suggestion line marker
- **THEN** `on_event` SHALL be invoked with `VerbEvent::Lifecycle(VerbLifecycleEvent::PromoteSuggestion { reason })` exactly once for that message; AND when `run_goal`, `run_query`, or `run_fix` is invoked, `on_event` SHALL NOT be invoked with the `PromoteSuggestion` variant under any path

---
### Requirement: Verb Error Enum

The system SHALL define `codebus_core::verb::VerbError` as a public enum with exactly these variants:

```
pub enum VerbError {
    VaultMissing { path: PathBuf },
    ConfigParse { which: &'static str, source: ConfigLoadError },
    KeyringMissing { source: KeyringError },
    Spawn { source: io::Error },
    Cancelled,
    AgentFailed { exit_code: Option<i32> },
    Internal { message: String },
}
```

The enum SHALL implement `std::error::Error` (via `thiserror`) AND `std::fmt::Display`. The variant semantics SHALL be:

- `VaultMissing { path }` — returned by `run_query`, `run_fix`, AND `run_chat_turn` when `<repo>/.codebus/` is absent; `run_goal` SHALL auto-init instead (per Goal Verb Library Function) AND SHALL NOT return this variant
- `ConfigParse { which, source }` — returned by any `run_*` when a config section yaml fails to parse. The `which` field SHALL be one of `"claude_code"`, `"lint.fix"`, `"log"`, or `"pii"` (a `&'static str` chosen by the verb function based on which loader rejected the yaml) so the CLI thin wrapper can emit section-specific stderr (`error: {which} config parse failed at {path}: {source}`) preserving byte-equivalent output
- `KeyringMissing { source }` — returned by any `run_*` when `build_env_overrides` cannot resolve the Azure profile's API key from the OS keyring + env fallback chain. Surfaced to the CLI as exit code 3, preserving the pre-refactor `error: {verb}: {source}` stderr line
- `Spawn { source }` — returned by any `run_*` when `agent::invoke` returns an `io::Result::Err` (e.g., claude binary not on PATH, fork failure)
- `Cancelled` — returned by any `run_*` when the `cancel` signal flag was observed flipped to true during the run. The `chat` verb CLI subcommand (`commands/chat.rs`) SHALL pass `cancel: Some(flag)` AND observe `VerbError::Cancelled` on the user's first Ctrl+C; the `goal` / `query` / `fix` CLI thin wrappers SHALL continue to pass `cancel: None` AND never observe this variant. Downstream `match` arms on `VerbError` in CLI commands SHALL handle `Cancelled` only in the chat command's branch AND SHALL leave the other commands' branches as unreachable for that variant (current behavior preserved)
- `AgentFailed { exit_code }` — returned by `run_chat_turn` when the spawned agent child terminated with a non-zero exit code. The `exit_code` field carries the child's reported exit code (`None` represents signal termination on platforms where the child died without an integer code). Distinct from `Spawn` (which is a launch failure): `AgentFailed` indicates the agent launched successfully AND ran but exited with an error, so callers SHALL surface "the turn failed" instead of silently treating a non-zero exit as success — this prevents the regression that the codex-backend change fixed (RunLog row mislabeled `"succeeded"` when codex returned an error code). The one-shot verbs (`run_goal`, `run_query`, `run_fix`, `run_quiz_plan`, `run_quiz_generate`) SHALL NOT emit `AgentFailed`; they propagate the child's exit code through their report struct's `agent_exit_code` field instead so the CLI thin wrapper SHALL call `ExitCode::from(child_exit)` directly on the `Ok(report)` path (the one-shot verbs always succeed at the verb-library level — only the agent's process exit matters to the caller).
- `Internal { message }` — returned by any `run_*` for any other unrecoverable failure with a human-readable message

`VerbError` SHALL expose a `cli_exit_code(&self) -> u8` method that maps each variant to the per-verb exit code policy preserved by the refactor: `VaultMissing` → 2, `ConfigParse` → 2, `KeyringMissing` → 3, `Spawn` → 1, `Cancelled` → 0 (CLI never observes this — guard for completeness), `AgentFailed` → 1, `Internal` → 1. The `AgentFailed` mapping SHALL collapse the child exit code into the single non-success code `1` rather than propagating the child's exit code value — chat REPL semantics keep `1 = something failed` simple for shell consumers, in contrast to the one-shot verbs that propagate the child's exit code through `Ok(report).agent_exit_code` (this divergence is intentional AND not a defect; it reflects different consumption models, chat REPL vs scriptable one-shot). CLI thin wrappers in `codebus-cli/src/commands/{chat,goal,query,fix,quiz}.rs` SHALL `match` exhaustively on `VerbError` to derive the exit code AND to emit the verb-specific stderr message. The exhaustive match guarantees compile-time coverage when a future variant is added.

The CLI thin wrappers SHALL handle `AgentFailed` per the following split:

- The `chat` command's `translate_error` SHALL match `AgentFailed { exit_code }` as an active arm AND emit a user-facing stderr line containing the child exit code (`error: chat: agent exited with code <N>`, or `error: chat: agent exited without a recorded exit code` when `exit_code` is `None`) AND exit with the `cli_exit_code()` value.
- The `goal` / `query` / `fix` / `quiz` thin wrappers SHALL match `AgentFailed { exit_code }` as a defensive arm (the verb library functions SHALL NOT emit `AgentFailed` on those paths, but exhaustive match requires the arm); the arm SHALL emit a generic stderr fallback (`error: <verb>: agent exited with code <N>` or the `None` form) AND exit with the `cli_exit_code()` value. The defensive arms SHALL NOT use `unreachable!()` — using a generic fallback avoids panicking the binary if a future regression emits `AgentFailed` from a one-shot verb.

#### Scenario: ConfigParse propagates underlying error with section label

- **WHEN** `run_goal` is invoked AND `~/.codebus/config.yaml` contains a `claude_code` section that fails yaml parsing
- **THEN** the function SHALL return `Err(VerbError::ConfigParse { which: "claude_code", source })` where `source.to_string()` SHALL contain the failing field name AND `which` SHALL equal the literal string `"claude_code"`

#### Scenario: KeyringMissing surfaces when Azure profile key is unreachable

- **WHEN** `run_goal` is invoked AND `~/.codebus/config.yaml` selects `claude_code.active: azure` AND `build_env_overrides` returns `Err(KeyringError::*)`
- **THEN** the function SHALL return `Err(VerbError::KeyringMissing { source })` AND `agent::invoke` SHALL NOT have been spawned AND `VerbError::cli_exit_code()` SHALL equal `3`

#### Scenario: Spawn surfaces underlying io error

- **WHEN** `run_query` is invoked AND the `claude` binary cannot be located on PATH AND `CODEBUS_CLAUDE_BIN` is unset
- **THEN** the function SHALL return `Err(VerbError::Spawn { source })` where `source.kind()` SHALL equal `io::ErrorKind::NotFound` or equivalent

#### Scenario: Chat CLI observes Cancelled on user Ctrl+C

- **WHEN** the `codebus chat` CLI is mid-turn AND the user presses Ctrl+C AND the SIGINT trap flips the cancel flag to true
- **THEN** `run_chat_turn` SHALL return `Err(VerbError::Cancelled)` AND the chat CLI command branch SHALL match this variant AND print an interrupted-status line AND redisplay the REPL prompt

#### Scenario: Chat returns AgentFailed when agent exits non-zero

- **WHEN** `run_chat_turn` is invoked AND the spawned agent child terminates with a non-zero exit code (e.g., codex `exec resume` rejecting a cross-provider switch)
- **THEN** the function SHALL return `Err(VerbError::AgentFailed { exit_code })` where `exit_code` carries the child's reported integer code (or `None` for signal termination) AND `VerbError::cli_exit_code()` SHALL equal `1` AND the chat CLI command branch SHALL emit a stderr line containing the child exit code AND the appended `RunLog.outcome` for that turn SHALL equal `"failed"` (NOT `"succeeded"`)

#### Scenario: AgentFailed Display message includes exit code when present

- **WHEN** `VerbError::AgentFailed { exit_code: Some(42) }.to_string()` is called
- **THEN** the resulting string SHALL contain the literal substring `42` AND SHALL contain the literal substring `non-zero status`

##### Example: Display output by exit_code shape

| exit_code value | Resulting Display contains | Notes |
| --- | --- | --- |
| `Some(0)` | `(0)` | uncommon but representable; verb only emits AgentFailed for non-zero, but the type permits any i32 |
| `Some(1)` | `(1)` | typical |
| `Some(42)` | `(42)` | multi-digit code |
| `None` | no `(...)` parenthesised group after `non-zero status` | signal termination; condition-formatted with `unwrap_or_default()` |

#### Scenario: Goal CLI thin wrapper handles AgentFailed as defensive fallback

- **WHEN** a hypothetical regression causes `run_goal` to emit `Err(VerbError::AgentFailed { exit_code: Some(7) })` (the verb library SHALL NOT emit this on the goal path under the contract, but exhaustive match requires the arm)
- **THEN** the `goal` CLI thin wrapper's `translate_error` SHALL match the arm AND emit a stderr line containing `agent exited with code 7` AND exit with status `1` AND SHALL NOT panic (`unreachable!()` is forbidden on this arm)


<!-- @trace
source: codex-backend-cli-agent-failed-handling
updated: 2026-05-23
code:
  - codebus-cli/src/commands/chat.rs
  - codebus-core/src/verb/error.rs
  - codebus-cli/src/commands/query.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-cli/src/commands/quiz.rs
-->

---
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
### Requirement: Auto-Commit Skipped On Cancel Or Error

When `run_goal` or `run_fix` returns `Err(VerbError::*)` for any error variant, the function SHALL NOT invoke `git::auto_commit`. When `run_goal` or `run_fix` returns `Ok(report)` after a successful agent run, auto_commit SHALL be invoked exactly once with the existing message format (preserving CLI byte-equivalent commit messages: `wiki: <goal-text>` for goal, `wiki: lint fix loop` for fix). `run_query` SHALL NEVER invoke `auto_commit` regardless of return value (query is read-only per its Query Verb Library Function requirement).

The auto-commit skip on error SHALL apply to all `VerbError` variants, including but not limited to `Cancelled` and `Spawn`. The on-disk vault working tree SHALL retain any partial wiki writes performed by the agent before the error — the caller (CLI or GUI) decides whether to discard or preserve them.

#### Scenario: Spawn error skips auto-commit

- **WHEN** `run_goal` is invoked AND `agent::invoke` returns `io::Result::Err` (e.g., binary missing) AND the wiki working tree had no pre-existing uncommitted changes
- **THEN** the function SHALL return `Err(VerbError::Spawn { source })` AND `git::auto_commit` SHALL NOT have been invoked AND the wiki tree SHALL remain at its pre-call HEAD state

#### Scenario: Cancel preserves partial writes

- **WHEN** `run_goal` is invoked with a cancel flag AND the agent writes one wiki page to the working tree before the cancel is observed AND the cancel halts the run
- **THEN** the function SHALL return `Err(VerbError::Cancelled)` AND `git::auto_commit` SHALL NOT have been invoked AND the written wiki page SHALL remain on disk in the working tree (uncommitted) AND the caller MAY inspect or discard the change as appropriate

---
### Requirement: Agent Invoke Resume Session Support

The `codebus_core::agent::claude_cli::InvokeAgentOptions` struct SHALL include a field `pub resume_session_id: Option<String>`. When this field is `Some(id)`, the `invoke()` function SHALL append `--resume <id>` to the `claude` command arguments before the `--tools` / `--allowedTools` / `--permission-mode` flags. When the field is `None`, no `--resume` argument SHALL be added (preserving the pre-chat-verb spawn argv shape).

The `--resume` flag SHALL be passed exactly once and SHALL precede the toolset flags. The session id value SHALL be passed verbatim (no quoting or escaping beyond what `Command::arg` provides) — Claude CLI session ids are UUID-like strings that require no shell quoting.

The existing CLI thin wrappers for `goal`, `query`, and `fix` SHALL initialize `InvokeAgentOptions::resume_session_id` to `None`, preserving byte-equivalent spawn behavior for those verbs.

#### Scenario: Resume id Some triggers --resume argument

- **WHEN** `invoke()` is called with `InvokeAgentOptions { resume_session_id: Some("abc-123"), .. }`
- **THEN** the spawned `claude` child process argv SHALL include `--resume` AND `abc-123` as consecutive arguments AND SHALL precede the `--tools` flag in argument order

#### Scenario: Resume id None omits --resume argument

- **WHEN** `invoke()` is called with `InvokeAgentOptions { resume_session_id: None, .. }`
- **THEN** the spawned `claude` child process argv SHALL NOT contain the `--resume` flag

#### Scenario: Existing verb thin wrappers pass None

- **WHEN** `run_goal`, `run_query`, or `run_fix` constructs an `InvokeAgentOptions` value
- **THEN** the `resume_session_id` field SHALL equal `None` AND the spawned argv SHALL be byte-equivalent to the pre-chat-verb implementation for the same invocation

---
### Requirement: Goal Content Verification and Repair

The system SHALL provide an optional independent model-based content verification stage for the `goal` verb, sharing the verify→repair orchestration with the `quiz` verb through a common core. The orchestration core SHALL expose a content-review status type (values `ok`, or `flagged` with the list of still-flagged item identifiers), a verify-output parser (an explicit `CONTENT_OK` line yields no defects; lines of the form `<id> | <defect-type> | <suggestion>` yield per-item defects; output containing neither is unparseable), and a bounded repair loop (an independent verify step, then on defects a repair step fed those defects, then re-verify, hard-capped at three iterations with the best body kept when the cap is reached). The `quiz` verb's externally observable behavior — its persisted `content_review` value format, its events, its cap, and its best-effort semantics — SHALL be unchanged by this refactor.

`run_goal` SHALL run this stage **after the fix loop and before `auto_commit`**, gated by a `goal.content_verify` configuration key (boolean, default `false`). When the key is absent or `false`, `run_goal` SHALL behave exactly as without this requirement (no verify spawn, no content-review status). When `true`, `run_goal` SHALL determine the wiki pages this run created or modified by diffing the vault git repository against the revision captured before the goal agent spawn, restricted to the `wiki/` subtree. If no wiki page changed, the stage SHALL resolve to `ok` without spawning anything. Otherwise it SHALL run one independent, read-only verify spawn (permitted to read `raw/code/` for grounding) that judges each changed page against exactly this three-item defect contract:

1. **unfaithful** — the page asserts something not grounded in (or contradicting) the `raw/code/` source mirror.
2. **off-goal** — the page's content is unrelated to this run's goal.
3. **taxonomy-misplaced** — the content is in the wrong page type / folder.

The verify spawn SHALL resolve its model and effort via `cc_cfg.resolve(Verb::Verify)`, NOT `Verb::Goal` (`claude-code-config` Endpoint Profile Schema requirement defines the `Verb::Verify` resolution path). This ensures the verify spawn uses the dedicated `claude_code.system.verify` / `claude_code.azure.verify` sub-block, which is independent of the main goal spawn and the repair spawn that both continue to use `Verb::Goal`. The motivating use case is "expensive verification + cheaper main writing" (e.g., sonnet for goal main spawn, opus for verify).

On defects, `run_goal` SHALL run a Write-capable repair spawn fed the defect list, instructed to fix only the flagged pages in place, then re-verify; this loop SHALL be bounded by the shared cap of three iterations. The repair spawn SHALL resolve its model via `cc_cfg.resolve(Verb::Goal)` (NOT `Verb::Verify`) — the repair stage continues to use the same model as the original goal main spawn, so the cost profile is "verify with the dedicated verify model, repair with the same goal model used for main writing". Verify and repair events SHALL flow through the same event fan-out the goal spawn uses.

Residual defects after the cap SHALL be best-effort: a non-fatal warning SHALL be surfaced, no page SHALL be reverted, the `GoalReport` SHALL carry a content-review status (`ok` / `flagged` with pages / not-run), `run_goal` SHALL NOT return an error solely because content defects remain, the exit code SHALL be unchanged, and `auto_commit` SHALL still run (content verification SHALL NOT block the commit). A verify spawn failure or unparseable output SHALL be treated as non-fatal: a warning SHALL be surfaced and the status SHALL be `flagged` (never silently `ok`). An absent content-review status SHALL be read as "not verified" and SHALL NOT be treated as `ok`.

`run_goal` SHALL NOT emit verify-spawn model / effort metadata into the per-run `RunLog` entry. The `RunLog` `model` and `effort` fields SHALL continue to record the main goal spawn's model (`Verb::Goal` resolution); the verify spawn's model is observable via the `events.jsonl` per-run timeline (which already records every spawn's `SpawnStart` event including the model in use), but SHALL NOT appear in the consolidated `RunLog` row.

#### Scenario: Disabled by default

- **WHEN** `run_goal` runs and `goal.content_verify` is absent or `false`
- **THEN** no verify spawn SHALL run AND `GoalReport` SHALL carry no content-review status AND `auto_commit` and exit code SHALL be unchanged

#### Scenario: No changed wiki pages short-circuits

- **WHEN** `goal.content_verify` is `true` and the run modified no `wiki/` page
- **THEN** the stage SHALL resolve to `ok` without running any verify spawn

#### Scenario: Clean content marks ok

- **WHEN** `goal.content_verify` is `true` and the verify spawn reports `CONTENT_OK` for the changed pages
- **THEN** the `GoalReport` content-review status SHALL be `ok` AND no content warning SHALL be surfaced AND `auto_commit` SHALL run normally

#### Scenario: Defect triggers bounded repair

- **WHEN** the verify spawn flags a page as `unfaithful`
- **THEN** a repair spawn SHALL revise only the flagged page AND the stage SHALL re-verify, repeating at most three iterations AND the final content-review status SHALL be `ok` if cleared or `flagged` with the still-flagged pages otherwise

#### Scenario: Residual defects are best-effort and do not block commit

- **WHEN** defects remain after the iteration cap
- **THEN** a non-fatal warning SHALL be surfaced AND no page SHALL be reverted AND `run_goal` SHALL NOT return an error solely for this AND the exit code SHALL be unchanged AND `auto_commit` SHALL still run

#### Scenario: Quiz behavior unchanged by the shared refactor

- **WHEN** `run_quiz_generate` is exercised after the verify→repair orchestration is factored into the shared core
- **THEN** its persisted `content_review` value format, events, cap, and best-effort semantics SHALL be identical to before the refactor

#### Scenario: Goal verify spawn uses Verb::Verify model not Verb::Goal

- **WHEN** `goal.content_verify` is `true`, `claude_code.system.goal` resolves to `model: sonnet-4-6`, AND `claude_code.system.verify` resolves to `model: opus-4-6`
- **THEN** the verify spawn SHALL be invoked with `--model claude-opus-4-6` AND the goal main spawn and the repair spawn SHALL be invoked with `--model claude-sonnet-4-6`

#### Scenario: Goal repair spawn uses Verb::Goal model not Verb::Verify

- **WHEN** `goal.content_verify` is `true`, the verify spawn flags a page, AND `claude_code.system.verify` resolves to `model: opus-4-6` while `claude_code.system.goal` resolves to `model: sonnet-4-6`
- **THEN** the repair spawn SHALL be invoked with `--model claude-sonnet-4-6` (the goal model, NOT the verify model) — repair keeps the same model profile as the goal main spawn

#### Scenario: Goal RunLog model field records main spawn not verify

- **WHEN** `goal.content_verify` is `true`, the main goal spawn resolves to `sonnet-4-6`, AND the verify spawn resolves to `opus-4-6`
- **THEN** the per-run `RunLog` entry SHALL record `model: claude-sonnet-4-6` (the main spawn's model) AND SHALL NOT record the verify spawn's model in any RunLog field

<!-- @trace
source: goal-content-verify, verify-stage-independent-model
updated: 2026-05-20
code:
  - codebus-core/src/config/mod.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/verb/mod.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/verb/content_verify.rs
  - codebus-core/src/git/nested_repo.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/git/mod.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/config/goal.rs
tests:
  - codebus-cli/tests/bins/mock_claude.rs
  - codebus-cli/tests/goal_content_verify_cli.rs
  - codebus-cli/tests/goal_flow.rs
-->

---
### Requirement: Agent Spawn MCP Isolation

The `codebus_core::agent::claude_cli::build_claude_cmd` function — the single production code path that spawns the `claude` CLI for every verb (goal, query, fix, chat, quiz, and the goal content-verify spawn), used by both the CLI and the desktop app — SHALL compose the spawned `claude` command's arguments to include MCP load-layer isolation flags so that NO ambient Model Context Protocol (MCP) server is loaded into the spawned agent session.

Specifically, the composed argument vector SHALL include `--strict-mcp-config` and SHALL include `--mcp-config` immediately followed by the literal empty-configuration argument `{"mcpServers":{}}` (passed as a single argument value via `Command::arg`, not through a shell and not as a file path). With `--strict-mcp-config` present, the `claude` process SHALL use only the MCP servers from the supplied `--mcp-config`; because that configuration declares zero servers, no user-scope (`~/.claude.json`), project-scope (`.mcp.json`), or connector-scope MCP server SHALL be loaded, and no `mcp__*` tool SHALL be exposed to the agent.

These two flags SHALL be positioned after `--verbose` and before the optional `--model` / `--effort` flags. This isolation SHALL be unconditional: it applies to every toolset and every model/effort combination, with no configuration option or environment variable to disable it.

The pre-existing argument-order invariant SHALL be preserved: when `InvokeAgentOptions.resume_session_id` is `Some(id)`, `--resume <id>` SHALL still appear before `--tools`; the added MCP flags appear after `--tools` and therefore do not affect that relationship.

The `--tools` and `--allowedTools` flags continue to gate built-in tools only; the MCP isolation flags are the mechanism that gates the MCP load layer (the toolset flags do not, and SHALL NOT be relied upon to, exclude MCP tools).

#### Scenario: Spawn argv carries MCP isolation flags

- **WHEN** `build_claude_cmd` composes the command for any verb spawn
- **THEN** the resulting argument vector SHALL contain `--strict-mcp-config` AND SHALL contain `--mcp-config` immediately followed by the argument `{"mcpServers":{}}`

#### Scenario: MCP isolation flags positioned after toolset flags and before model flags

- **WHEN** `build_claude_cmd` composes the command with `model: Some("claude-opus-4-6")` and `effort: Some("high")`
- **THEN** `--strict-mcp-config` and `--mcp-config` SHALL appear after `--verbose` AND before `--model`

#### Scenario: MCP isolation does not break the resume-before-tools invariant

- **WHEN** `build_claude_cmd` composes the command with `resume_session_id: Some("abc-123")`
- **THEN** `--resume abc-123` SHALL appear before `--tools` AND the `--strict-mcp-config` / `--mcp-config` flags SHALL appear after `--tools`

#### Scenario: Spawned agent exposes no MCP tools

- **WHEN** a `claude -p` process is spawned via the flags `build_claude_cmd` produces, in an environment where ambient MCP servers (user-scope, connector-scope, or a project `.mcp.json`) are configured
- **THEN** the spawned session's reported tool set SHALL contain no `mcp__*` tool AND the reported MCP server list SHALL be empty, while the built-in tools permitted by `--tools` remain available