## ADDED Requirements

### Requirement: Verb RunLog Capture and Persistence

The `goal`, `query`, and `fix` subcommands SHALL each capture exactly one `RunLog` entry per invocation (per the `run-log` capability `RunLog Schema and Per-Invocation Capture` requirement) and SHALL persist it to the configured `LogSink` (resolved per the `run-log` capability `Log Configuration Schema` and `Default Log Directory Resolution` requirements). The persistence step SHALL run as the verb's penultimate action — after the auto-commit step (where applicable) and before the `Done` banner — so that the entry includes the final `wiki_changed`, `lint_error_count`, and `lint_warn_count` values. When the verb fails (agent crash, lint phase failure, auto-commit failure), the persistence step SHALL still run so the `RunLog` records the partial-state outcome; the verb SHALL NOT skip log persistence on its failure paths. When the `LogSink::write_run` call returns an error, the verb SHALL emit a stderr warning prefixed with `warning: run-log` and SHALL NOT propagate the failure into its exit code (per the `run-log` capability `RunLog Write Failure Is Non-Fatal` requirement).

#### Scenario: Each verb invocation appends exactly one RunLog entry

- **WHEN** `codebus goal "X"` runs to completion against a vault configured with `log.sink: jsonl`
- **THEN** the file `<vault>/.codebus/log/runs-<YYYY-MM-DD>.jsonl` SHALL gain exactly one new line containing a single JSON object whose `goal` field equals `"X"` and whose `mode` field equals `"goal"`

#### Scenario: RunLog written even when agent exits non-zero

- **WHEN** the agent spawn exits with non-zero status during a `codebus goal "X"` invocation
- **THEN** the `RunLog` entry SHALL still be appended to the jsonl file AND its `tokens` field SHALL reflect any `Usage` events that were streamed before the failure AND its `mode` SHALL equal `"goal"`

#### Scenario: Default sink resolves to vault-local log directory

- **WHEN** `codebus goal "X"` runs against a vault and no `log:` section is present in `~/.codebus/config.yaml`
- **THEN** the default `Jsonl { dir: None }` config SHALL resolve to `<vault>/.codebus/log/` AND the entry SHALL be appended there

#### Scenario: Explicit none sink suppresses persistence

- **WHEN** `~/.codebus/config.yaml` contains `log:\n  sink: none\n` AND `codebus goal "X"` runs to completion
- **THEN** no file under `<vault>/.codebus/log/` SHALL be created or modified

## MODIFIED Requirements

### Requirement: Goal Subcommand Behavior

The `goal` subcommand SHALL accept one positional argument `<goal-text>` (the user's natural-language goal description) and the following flags: `--repo <PATH>` (default: current working directory), `--force-resync` (boolean, force raw mirror re-sync even when source-signal detection reports no drift), `--no-obsidian-register` (boolean, forwarded to the auto-init fallback), `--no-fix` (boolean, skip the post-agent lint-and-fix phase entirely), and the inherited `--debug` global flag. When invoked, the subcommand SHALL execute exactly seven steps in order: (1) resolve the target repository path, (2) when `<repo>/.codebus/` does not exist, run the `init` flow against the same repo (forwarding `--no-obsidian-register`); (3) perform source-signal drift detection and conditionally re-sync the raw mirror plus update the manifest; (4) spawn the goal agent CLI with the canonical sandbox flags described below AND consume its stdout via the stream-rendering pipeline defined by the `agent-stream-rendering` capability — agent stdout SHALL NOT pass through verbatim; the parser-renderer pipeline transforms each `assistant` / `user` / `result` JSON line into terminal-friendly `Thought` / `ToolUse` / `ToolResult` events while accumulating `Usage` events into a per-invocation `TokenUsage`; (5) after the goal agent terminates, run the lint-and-fix phase against the vault unless `--no-fix` is present or `lint.fix.enabled` is `false` in config; (6) after the lint-and-fix phase terminates (or is skipped), invoke the vault auto-commit operation with the message `wiki: <goal-text>` against the nested vault repository; (7) capture and persist a `RunLog` entry per the `Verb RunLog Capture and Persistence` requirement before printing the `Done` banner. The subcommand SHALL exit with the goal agent's exit code when the goal agent exited non-zero, otherwise with the fix phase's exit code, unless the auto-commit operation itself fails, in which case it SHALL exit with a non-zero code identifying the auto-commit failure.

The goal agent spawn SHALL pass the following arguments to the agent CLI: `-p` followed by the literal string `/codebus-goal "<goal-text>"`; `--tools` followed by the comma-separated string `Read,Glob,Grep,Write,Edit`; `--allowedTools` followed by the same comma-separated string; `--permission-mode` followed by `acceptEdits`; `--output-format stream-json`; `--verbose`; `--input-format stream-json`. The goal agent's child process SHALL be invoked with the current working directory set to `<repo>/.codebus/`, its stdin SHALL be a closed stream, and both stdout and stderr SHALL be `Stdio::piped()` so the parent process can consume stdout for stream parsing and copy stderr through to the parent's stderr verbatim.

The lint-and-fix phase SHALL invoke the single-shot fix flow defined by the `Fix Single-Shot Verification` requirement of the `lint-feedback-loop` capability, using the same vault root as the goal agent. The phase SHALL be skipped when either `--no-fix` is present on the goal invocation or `lint.fix.enabled` is `false` in `~/.codebus/config.yaml`. The system SHALL NOT recognize a `--fix-max-iter` flag (removed in v3-fix-trust-agent) — attempts to pass it SHALL fail at clap argument parsing.

The auto-commit operation in step 6 SHALL use a single commit message `wiki: <goal-text>` covering both the goal agent's writes and any fix-phase edits, regardless of whether the lint-and-fix phase ran or how it terminated.

#### Scenario: Goal subcommand invokes auto-init when vault is missing

- **WHEN** `codebus goal "describe the auth flow"` is invoked against a repository whose `.codebus/` directory does not yet exist
- **THEN** the subcommand SHALL first run the init flow (creating `.codebus/`, vault layout, raw mirror, schema, manifest, skill bundles, nested git, and the initial `init: codebus vault` commit) AND afterwards SHALL proceed to spawn the goal agent against the freshly-initialized vault

#### Scenario: Goal subcommand spawns agent with cwd at vault root

- **WHEN** the goal subcommand reaches its agent-spawn step against a vault rooted at `<repo>/.codebus/`
- **THEN** the goal agent child process SHALL be spawned with current working directory equal to `<repo>/.codebus/` AND the child SHALL receive the prompt argument `/codebus-goal "<goal-text>"` AND the child's stdin SHALL be closed AND both stdout and stderr SHALL be `piped()` (NOT inherited)

#### Scenario: Goal subcommand passes the canonical triple-flag sandbox plus stream-json flags

- **WHEN** the goal subcommand spawns the goal agent
- **THEN** the spawned command line SHALL include the flag pair `--tools Read,Glob,Grep,Write,Edit`, the flag pair `--allowedTools Read,Glob,Grep,Write,Edit`, the flag pair `--permission-mode acceptEdits`, `--output-format stream-json`, `--verbose`, AND `--input-format stream-json`

#### Scenario: Goal stdout consumed line-by-line and rendered as stream events

- **WHEN** the goal agent emits one stream-json line containing an `assistant` event with a single text content item
- **THEN** the parent process SHALL print one rendered `Thought` line to its own stdout (per the `agent-stream-rendering` capability `Stream Event Terminal Rendering` requirement) AND the agent's raw JSON line SHALL NOT appear on the parent stdout

#### Scenario: Goal flow runs lint-and-fix between goal agent termination and auto-commit

- **WHEN** the goal subcommand spawns the goal agent and the agent terminates after writing one or more files inside `<repo>/.codebus/wiki/`, with `--no-fix` absent and `lint.fix.enabled: true`
- **THEN** the lint-and-fix phase SHALL run against the vault before any auto-commit AND any file modifications produced by the fix phase SHALL be included in the same commit as the goal agent's writes

#### Scenario: Goal flow folds goal writes and fix edits into a single commit

- **WHEN** the goal subcommand completes the goal agent step (with vault changes), then completes the lint-and-fix phase (with additional vault changes), then reaches the auto-commit step
- **THEN** the auto-commit SHALL produce exactly one new commit on the nested vault git repo AND the commit message SHALL equal `wiki: <goal-text>`

#### Scenario: Goal flow skips fix when --no-fix is supplied

- **WHEN** `codebus goal "X" --no-fix` is invoked against a vault that would have lint issues after the goal agent runs
- **THEN** the goal subcommand SHALL NOT spawn any fix agent AND the auto-commit SHALL still produce a commit with message `wiki: X`

#### Scenario: Goal subcommand auto-commits on goal agent failure with partial writes

- **WHEN** the goal agent exits with non-zero status after writing one or more files inside `<repo>/.codebus/wiki/`
- **THEN** the lint-and-fix phase SHALL still run (unless skipped by `--no-fix` or config) AND afterwards the subcommand SHALL invoke vault auto-commit with message `wiki: <goal-text>` AND the subcommand SHALL propagate the goal agent's non-zero exit code AND a `RunLog` entry SHALL still be persisted before the verb exits

#### Scenario: Goal subcommand no-op commit when neither goal agent nor fix make changes

- **WHEN** the goal agent exits without modifying any file under `<repo>/.codebus/wiki/` AND the lint-and-fix phase produces no further changes
- **THEN** the auto-commit operation SHALL be a no-op (working tree clean) AND `git -C <repo>/.codebus rev-list --count HEAD` SHALL equal the same count as before the goal invocation

#### Scenario: Force-resync flag bypasses detection

- **WHEN** `codebus goal "..." --force-resync` is invoked against a vault whose source signal would otherwise be unchanged
- **THEN** the subcommand SHALL re-run the raw mirror unconditionally AND SHALL update the manifest's `last_sync_at` timestamp regardless of whether the source content changed

#### Scenario: Goal subcommand propagates goal agent non-zero exit code

- **WHEN** the goal agent terminates with non-zero exit status N
- **THEN** the goal subcommand SHALL exit with status N regardless of whether the fix phase subsequently succeeds, unless the auto-commit operation itself fails

#### Scenario: --fix-max-iter is no longer a recognized goal flag

- **WHEN** the user runs `codebus goal "X" --fix-max-iter 5`
- **THEN** clap argument parsing SHALL reject the unknown `--fix-max-iter` flag AND the binary SHALL exit with non-zero status

### Requirement: Query Subcommand Behavior

The `query` subcommand SHALL accept one positional argument `<query-text>` (the user's natural-language question) and the inherited `--repo <PATH>` (default: current working directory) and `--debug` global flags. When invoked, the subcommand SHALL execute exactly five steps in order: (1) resolve the target repository path, (2) check the vault precondition — if `<repo>/.codebus/` does not exist, exit with status 2 and emit a stderr message instructing the user to run `codebus init` first; do NOT auto-run init; (3) spawn the agent CLI with the read-only sandbox flags described below AND consume its stdout via the stream-rendering pipeline defined by the `agent-stream-rendering` capability; (4) after the child terminates, exit with the child process's exit code without performing any vault auto-commit operation; (5) capture and persist a `RunLog` entry per the `Verb RunLog Capture and Persistence` requirement before exiting (mode `"query"`, `wiki_changed: false`, lint counts both 0).

The agent spawn SHALL pass the following arguments to the agent CLI: `-p` followed by the literal string `/codebus-query "<query-text>"`; `--tools` followed by the comma-separated string `Read,Glob,Grep`; `--allowedTools` followed by the same comma-separated string; `--permission-mode` followed by `acceptEdits`; `--output-format stream-json`; `--verbose`; `--input-format stream-json`. The agent's child process SHALL be invoked with the current working directory set to `<repo>/.codebus/`, its stdin SHALL be closed, and both stdout and stderr SHALL be `Stdio::piped()`.

The query subcommand SHALL NOT trigger raw-mirror re-sync, SHALL NOT update the vault manifest, SHALL NOT modify any file inside the vault, and SHALL NOT create any nested-git commit.

#### Scenario: Query refuses when vault is missing

- **WHEN** `codebus query "what does Foo do"` is invoked against a repository whose `.codebus/` directory does not exist
- **THEN** the subcommand SHALL exit with status 2 AND stderr SHALL contain a message instructing the user to run `codebus init` first AND no agent process SHALL be spawned

#### Scenario: Query spawns agent with cwd at vault root and piped stdio

- **WHEN** the query subcommand reaches its agent-spawn step against a vault rooted at `<repo>/.codebus/`
- **THEN** the agent child process SHALL be spawned with current working directory equal to `<repo>/.codebus/` AND the child SHALL receive the prompt argument `/codebus-query "<query-text>"` AND the child's stdin SHALL be closed AND both stdout and stderr SHALL be `piped()`

#### Scenario: Query passes the read-only triple-flag sandbox plus stream-json flags

- **WHEN** the query subcommand spawns the agent
- **THEN** the spawned command line SHALL include the flag pair `--tools Read,Glob,Grep`, the flag pair `--allowedTools Read,Glob,Grep`, the flag pair `--permission-mode acceptEdits`, `--output-format stream-json`, `--verbose`, AND `--input-format stream-json`
- **AND** the spawned command line SHALL NOT include `Write` or `Edit` in either the `--tools` or `--allowedTools` value

#### Scenario: Query does not auto-commit

- **WHEN** the query subcommand spawns the agent and the agent exits with any status code
- **THEN** the subcommand SHALL NOT call the vault auto-commit operation AND running `git -C <repo>/.codebus rev-list --count HEAD` after the query SHALL equal the same count as before the query invocation

#### Scenario: Query propagates agent exit code

- **WHEN** the query subcommand spawns the agent and the agent terminates with exit status code N (where N is any non-negative integer)
- **THEN** the query subcommand SHALL exit with status code N

#### Scenario: Query does not modify any vault file

- **WHEN** the query subcommand runs to completion against a vault with existing wiki pages
- **THEN** running `git -C <repo>/.codebus status --porcelain` after the query SHALL print no output (vault working tree clean)

#### Scenario: Query persists a RunLog entry with mode query

- **WHEN** `codebus query "..."` runs to completion
- **THEN** the appended `RunLog` SHALL have `mode == "query"` AND `wiki_changed == false` AND `lint_error_count == 0` AND `lint_warn_count == 0`

### Requirement: Fix Subcommand Behavior

The `fix` subcommand SHALL accept the following flags: `--repo <PATH>` (default: current working directory), `--no-fix` (boolean, when present the subcommand SHALL exit with status zero and a stderr message stating fix is disabled, without spawning any agent or running lint), and the inherited `--debug` global flag. When invoked without `--no-fix`, the subcommand SHALL execute exactly seven steps in order: (1) resolve the target repository path and verify `<repo>/.codebus/` exists (otherwise exit with status 2 and a stderr hint instructing the user to run `codebus init`), (2) run lint pre-check; if zero issues, exit with status zero and skip remaining steps, (3) spawn the fix agent CLI exactly once per the `Fix Single-Shot Verification` requirement of the `lint-feedback-loop` capability AND consume its stdout via the stream-rendering pipeline defined by the `agent-stream-rendering` capability, (4) wait for the agent process to terminate (no further agent spawns), (5) run lint final check, (6) invoke vault auto-commit with message `wiki: lint fix loop`, (7) capture and persist a `RunLog` entry per the `Verb RunLog Capture and Persistence` requirement (mode `"fix"`, `goal: ""`, lint counts from the final lint check). The subcommand SHALL exit with status reflecting the final lint state.

The fix subcommand SHALL NOT trigger source-signal drift detection, SHALL NOT call raw mirror sync, SHALL NOT update the vault manifest, SHALL NOT modify any source code outside the vault, and SHALL NOT spawn a goal-style agent. The only agent process the fix subcommand spawns SHALL be one fix agent per invocation defined by the `Fix Loop Agent Sandbox` requirement of the `lint-feedback-loop` capability, with both stdout and stderr `piped()` for stream consumption.

The system SHALL NOT recognize a `--fix-max-iter` flag (removed in v3-fix-trust-agent) — attempts to pass it SHALL fail at clap argument parsing.

The subcommand SHALL exit with status zero when the post-spawn lint reports zero issues. The subcommand SHALL exit with status one when one or more issues remain after the agent process terminates. The subcommand SHALL exit with status two when the vault precondition fails.

#### Scenario: Fix refuses when vault is missing

- **WHEN** `codebus fix --repo <repo>` is invoked against a path whose `.codebus/` directory does not exist
- **THEN** the subcommand SHALL exit with status 2 AND stderr SHALL contain a message instructing the user to run `codebus init` first AND no agent process SHALL be spawned

#### Scenario: Fix exits zero on clean vault without persisting a RunLog

- **WHEN** `codebus fix` runs against a vault whose initial lint reports zero issues
- **THEN** no agent process SHALL be spawned AND the subcommand SHALL exit with status zero AND no new git commit SHALL be created AND the verb SHALL skip RunLog persistence on this short-circuit path (no agent ran → no tokens to record)

#### Scenario: Fix --no-fix flag disables fix entirely

- **WHEN** `codebus fix --no-fix` is invoked against any vault
- **THEN** the subcommand SHALL exit with status zero AND no agent process SHALL be spawned AND no lint SHALL be performed AND stderr SHALL contain a message stating fix is disabled

#### Scenario: Fix spawns the agent exactly once with stream-json flags

- **WHEN** `codebus fix` runs against a vault whose lint precheck reports issues
- **THEN** the subcommand SHALL spawn the agent CLI exactly one time for the entire fix run AND SHALL NOT spawn additional agent processes regardless of post-agent lint state AND the spawned argv SHALL include `--output-format stream-json`, `--verbose`, AND `--input-format stream-json`

#### Scenario: Fix auto-commits with lint fix loop message

- **WHEN** `codebus fix` runs and the agent's in-session work produces at least one change under `wiki/`
- **THEN** the subcommand SHALL invoke vault auto-commit AND `git -C <repo>/.codebus log --pretty=%s -1` SHALL print exactly the line `wiki: lint fix loop`

#### Scenario: Fix exits one when post-spawn lint has issues

- **WHEN** `codebus fix` completes its single agent spawn and the post-spawn lint reports at least one issue
- **THEN** the subcommand SHALL exit with status one AND SHALL still invoke the auto-commit operation AND SHALL still persist a `RunLog` entry recording the residual lint counts

#### Scenario: Fix RunLog records final lint counts

- **WHEN** `codebus fix` runs to completion and the post-spawn lint reports E errors and W warnings
- **THEN** the appended `RunLog.lint_error_count` SHALL equal E AND `RunLog.lint_warn_count` SHALL equal W

#### Scenario: --fix-max-iter is no longer a recognized fix flag

- **WHEN** the user runs `codebus fix --fix-max-iter 5`
- **THEN** clap argument parsing SHALL reject the unknown `--fix-max-iter` flag AND the binary SHALL exit with non-zero status
