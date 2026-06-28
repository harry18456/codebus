# cli Specification

## Purpose

The command-line entry surface — the `codebus` binary's subcommand registry, global flags, and per-verb invocation contracts. Five subcommands: `init` (vault bootstrap), `goal` (ingest with auto-fix), `query` (read-only Q&A), `lint` (vault validation), `fix` (auto-repair). Each verb's behavior — sandbox flags passed to the spawned agent, exit code policy, auto-commit timing — is normatively specified here. Cross-cuts: spawn-pattern internals (sandbox composition, hook installation) live in `lint-feedback-loop`; vault structure init writes lives in `vault`; skill bundle materialization lives in `skill-bundles`.

## Requirements

### Requirement: Subcommand Registration

The `codebus` binary SHALL register nine subcommands at the top level: `init`, `goal`, `query`, `lint`, `fix`, `config`, `chat`, `quiz`, `mcp`. The `mcp` subcommand SHALL be gated behind the default-on `mcp` cargo feature; when that feature is disabled at build time the binary SHALL register only the other eight. No other subcommand SHALL be registered. The `--help` and `--version` flags SHALL be available at both the binary level and per subcommand. The `config` subcommand SHALL itself expose three sub-actions (`set-key`, `get-key`, `delete-key`); the sub-action contract is defined normatively in the `claude-code-config` capability. The `quiz` subcommand SHALL itself expose a `validate` sub-action (`codebus quiz validate <quiz-md-file | -> [--count <N>] [--json]`); the sub-action contract is defined normatively in the `Quiz Validate Sub-Action Behavior` requirement of this capability and the `Quiz Output Validation and Repair` requirement of the `quiz` capability. The `chat` subcommand contract is defined normatively in the `Chat Subcommand Behavior` requirement of this capability and the `Chat CLI Subcommand Behavior` requirement of the `chat-verb` capability. The `quiz` subcommand contract is defined normatively in the `Quiz Subcommand Behavior` requirement of this capability and the `quiz` capability. The `mcp` subcommand contract is defined normatively in the `mcp-server` capability.

#### Scenario: Help output lists the eight always-on subcommands plus feature-gated mcp

- **WHEN** `codebus --help` is invoked in a default build
- **THEN** the help output SHALL list `init`, `goal`, `query`, `lint`, `fix`, `config`, `chat`, `quiz`, `mcp` as the only subcommands AND SHALL exit with status zero

#### Scenario: Version flag prints cargo package version

- **WHEN** `codebus --version` is invoked
- **THEN** the output SHALL be the `codebus` cargo package version AND SHALL exit with status zero

#### Scenario: Quiz validate sub-action is registered under quiz

- **WHEN** `codebus quiz --help` is invoked
- **THEN** the help output SHALL document a `validate` sub-action AND the top-level `codebus --help` SHALL still list exactly the registered subcommands (the eight always-on plus `mcp` when its default-on feature is enabled) with no further undocumented top-level subcommand

<!-- @trace
source: quiz-validate-enforce-question-count, mcp-server-single-vault
updated: 2026-06-27
code:
  - codebus-cli/src/main.rs
  - codebus-cli/src/commands/mcp.rs
tests:
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: No-Arg Defaults to Init Dispatch

Invoking the `codebus` binary with zero arguments SHALL be treated equivalently to invoking `codebus init` with the subcommand's default arguments. The dispatch path SHALL be unified: the binary MUST NOT have separate code paths for "no-arg" and "explicit init" beyond the subcommand resolution layer.

#### Scenario: Bare invocation routes to init handler

- **WHEN** `codebus` is invoked with no arguments
- **THEN** the binary SHALL invoke the `init` subcommand handler exactly as if the user had typed `codebus init`

#### Scenario: Explicit init invocation produces identical behavior to bare

- **WHEN** `codebus` and `codebus init` are both invoked under identical environment and working directory
- **THEN** their stderr / stdout output AND exit status SHALL be identical


<!-- @trace
source: v3-workspace
updated: 2026-05-08
code:
  - codebus-cli/src/commands/query.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/main.rs
  - codebus-app/src/lib.rs
  - codebus-cli/src/commands/goal.rs
  - Cargo.toml
  - codebus-app/Cargo.toml
  - codebus-app/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-cli/src/commands/mod.rs
  - codebus-cli/src/commands/lint.rs
  - codebus-core/src/lib.rs
  - codebus-cli/Cargo.toml
  - codebus-cli/src/commands/fix.rs
tests:
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: Init Subcommand Behavior

The `init` subcommand SHALL accept the flags `--repo <PATH>` (default: current working directory) and `--no-obsidian-register` (boolean flag). When invoked, `init` SHALL orchestrate (in order): pre-flight sanity check, vault layout creation, **source repo `.gitignore` mutation**, raw mirror, per-repo schema file write, manifest write, skill bundle authoring, Obsidian vault registration, and global config starter write. The source repo `.gitignore` mutation step SHALL precede the raw mirror so the byte-count signal recorded in the manifest reflects the post-init source state — otherwise subsequent verb invocations (`goal` / `query`) would compute a different source signal from a fresh walk and falsely conclude that drift has occurred. The global config starter write step SHALL invoke the `write_starter_config_if_missing` primitive against `~/.codebus/config.yaml`; when the file is absent the system SHALL create the parent directory if necessary and write the starter content; when the file already exists the system SHALL NOT read or overwrite it. In default output mode `init` SHALL emit a sequence of banners (defined by the `Banner Output for Verb Commands` requirement) covering at minimum the `Start`, `SyncDone`, `PiiSummary`, `CommitDone`, and `Done` banner variants. Default mode SHALL NOT emit per-step `✓ <internal-detail>` progress lines (these are reserved for `--debug` mode). When `--debug` is passed, `init` SHALL emit the same banner sequence AND additionally emit the per-step `✓ <internal-detail>` progress lines (vault layout, source `.gitignore` mutation, schema file write, manifest write, skill bundle authoring, vault settings write, global config starter, and any others implementation chooses) AND the `[debug]` lines describing internal decisions, fs operations, computed source signal values, and target paths. `init` SHALL exit with status zero on success and non-zero only if a sanity-check refusal or unrecoverable filesystem error occurs. A failure of the global config starter write step SHALL emit a stderr warning prefixed with `warning: global config` AND SHALL NOT cause `init` to exit non-zero (the rest of init having succeeded means the per-vault state is usable).

#### Scenario: Init with --repo flag targets the specified directory

- **WHEN** `codebus init --repo /tmp/testrepo` is invoked
- **THEN** the system SHALL initialize the vault under `/tmp/testrepo/.codebus/` AND SHALL NOT touch the current working directory's filesystem

#### Scenario: Init with --no-obsidian-register skips Obsidian step

- **WHEN** `codebus init --no-obsidian-register` is invoked against a system with Obsidian installed
- **THEN** the system SHALL complete all other init steps AND the Obsidian config file SHALL be unchanged AND init SHALL exit with status zero

#### Scenario: Default mode emits banner sequence not per-step progress lines

- **WHEN** `codebus init` runs successfully against a fresh git repository on a system without Obsidian, without `--debug`
- **THEN** stdout SHALL contain the `Start` banner (a line that includes the literal token `🚌` when emoji are enabled or the literal token `▶` when emoji are disabled), the `SyncDone` banner (a line beginning with `✓` or `✅` and containing the substring `同步完成` or equivalent translation key), the `PiiSummary` banner, the `CommitDone` banner, and the `Done` banner — in that order — AND stdout SHALL NOT contain a line matching the literal prefix `✓ vault layout:`, `✓ schema file:`, `✓ manifest:`, `✓ skill bundles:`, `✓ vault settings:`, `✓ vault internal .gitignore:`, `✓ source .gitignore:`, or `✓ global config:` (those are reserved for `--debug` mode)

#### Scenario: Debug mode adds per-step progress lines and [debug] traces

- **WHEN** `codebus init --debug` runs against the same fresh git repository
- **THEN** stdout SHALL contain every banner from the default-mode scenario AND SHALL ADDITIONALLY contain the per-step progress lines `✓ vault layout:`, `✓ schema file:`, `✓ manifest:`, `✓ skill bundles:`, `✓ vault settings:`, and `✓ global config:` AND stderr or stdout SHALL contain at least one line beginning with `[debug]`

#### Scenario: PII match count is reported in PiiSummary banner

- **WHEN** `codebus init` runs against a repository whose mirrored content yields exactly N PII matches
- **THEN** stdout SHALL contain the `PiiSummary` banner whose body contains the substring `hits <N>` where `<N>` is the integer total, regardless of whether `--debug` is set

#### Scenario: Init exits zero on first successful run

- **WHEN** init runs to completion against a fresh repository
- **THEN** the binary SHALL exit with status zero

#### Scenario: Init writes 3 skill bundles to per-project Claude Code directory

- **WHEN** `codebus init --repo <path>` runs against a target with no existing skill bundles at `<path>/.claude/skills/codebus-{goal,query,fix}/`
- **THEN** after init the three bundle directories SHALL exist under `<path>/.claude/skills/` AND each SHALL contain a `SKILL.md` file. The system SHALL NOT write to `~/.claude/skills/codebus-{goal,query,fix}/` for this invocation.

#### Scenario: Init writes global config starter when missing

- **WHEN** `codebus init` runs against a system where `~/.codebus/config.yaml` does not exist
- **THEN** after init the file `~/.codebus/config.yaml` SHALL exist AND SHALL parse successfully as YAML AND the parsed `pii.scanner` field SHALL equal `regex_basic` AND the parsed `pii.on_hit` field SHALL equal `mask`

#### Scenario: Init does not overwrite existing global config

- **WHEN** `codebus init` runs against a system where `~/.codebus/config.yaml` already exists with custom user content (for example `pii:\n  on_hit: warn\n`)
- **THEN** after init the file SHALL contain byte-identical content to its pre-init state AND the parsed `pii.on_hit` SHALL equal `warn`

#### Scenario: Global config write failure does not abort init

- **WHEN** `codebus init` runs against a system where the parent directory of `~/.codebus/config.yaml` cannot be created (for example due to filesystem permissions)
- **THEN** the global config write step SHALL emit a stderr warning prefixed with `warning: global config` AND init SHALL exit with status zero AND all other init steps SHALL complete successfully

#### Scenario: Subsequent goal invocation does not trigger redundant re-sync

- **WHEN** `codebus init --repo <repo>` runs to completion against a fresh git repository, immediately followed by `codebus goal "..." --repo <repo>` with no intervening source mutation
- **THEN** the goal invocation SHALL NOT emit the `SyncStart` or `SyncDone` banner (the source-signal drift detection SHALL conclude no drift exists, skipping raw mirror re-sync)


<!-- @trace
source: v3-bug-fixes
updated: 2026-05-10
code:
  - codebus-core/src/wiki/lint/locate.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-cli/src/commands/init.rs
tests:
  - codebus-cli/tests/lint_flow.rs
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: Debug Flag Output

The `codebus` binary SHALL accept `--debug` as a global flag, available at the top-level command and inheritable by every subcommand (e.g., `codebus --debug init`, `codebus init --debug` SHALL behave equivalently). When `--debug` is set, the binary's verb handlers SHALL emit (in addition to the default-mode banner sequence) the per-step `✓ <internal-detail>` progress lines describing intermediate orchestration outcomes AND the `[debug]` lines describing internal decisions, fs operations, computed values, and target paths. When `--debug` is NOT set, the binary SHALL NOT emit any line beginning with `[debug]` AND SHALL NOT emit per-step `✓ <internal-detail>` progress lines (only the higher-level banner sequence emerges in default mode).

When `--debug` is set, the binary SHALL additionally render the agent stream in verbose form: it SHALL set `RenderOptions.verbose` to true so the agent-stream renderer (per the `agent-stream-rendering` capability `Stream Event Terminal Rendering` requirement) surfaces complete tool input and complete tool result without summarization, truncation, or suppression. When `--debug` is NOT set, `RenderOptions.verbose` SHALL be false and the agent stream SHALL render in the compact form (byte-identical to the pre-change behavior). This verbose stream rendering applies to the agent-spawning verbs (`goal`, `query`, `fix`, `chat`, `quiz`); it does not alter how non-agent subcommands render.

#### Scenario: Default mode suppresses [debug] lines

- **WHEN** `codebus init` runs without `--debug`
- **THEN** neither stdout nor stderr SHALL contain any line beginning with `[debug]`

#### Scenario: Debug mode emits both detail and trace lines

- **WHEN** `codebus init --debug` runs against any repository
- **THEN** stdout SHALL contain at least one per-step `✓ <internal-detail>` progress line AND at least one `[debug]` trace line

#### Scenario: Debug flag enables verbose agent stream rendering

- **WHEN** a `codebus` agent-spawning verb runs with `--debug`
- **THEN** the `RenderOptions` passed to the agent-stream renderer SHALL have `verbose` set to true

#### Scenario: Default mode keeps compact agent stream rendering

- **WHEN** a `codebus` agent-spawning verb runs without `--debug`
- **THEN** the `RenderOptions` passed to the agent-stream renderer SHALL have `verbose` set to false

#### Scenario: Quiz inherits verbose agent stream rendering under debug

- **WHEN** `codebus quiz "<topic>"` runs with `--debug` (the quiz subcommand is an agent-spawning verb that consumes the same `--debug`-derived `RenderOptions` snapshot as `goal` / `query` / `fix` / `chat`)
- **THEN** the `RenderOptions` passed to the quiz generate spawn's agent-stream renderer SHALL have `verbose` set to true, so the quiz agent stream surfaces complete tool input and complete tool result without truncation or summarization


<!-- @trace
source: run-log-spec-include-quiz
updated: 2026-05-30
code:
  - codebus-cli/src/main.rs
tests:
  - codebus-cli/tests/quiz_flow.rs
-->

---
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

---
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

---
### Requirement: Lint Subcommand Behavior

The `lint` subcommand SHALL accept the following flags: `--repo <PATH>` (optional, overrides the cwd-based vault auto-detection defined in the `lint-feedback-loop` capability), `--format <text|json>` (default: `text`), and the inherited `--debug` global flag. When invoked, the subcommand SHALL execute exactly three steps in order: (1) resolve the vault root via the auto-detection rules from the `lint-feedback-loop` capability, (2) run the lint rule set in-process against the vault `wiki/` subtree, (3) emit output in the selected format and exit.

The lint subcommand SHALL be read-only — it SHALL NOT modify any vault file, SHALL NOT spawn any agent, SHALL NOT trigger raw mirror re-sync, SHALL NOT update the vault manifest, and SHALL NOT create any nested-git commit.

The subcommand SHALL exit with status zero when the lint result has zero errors (warnings do not affect exit status). The subcommand SHALL exit with status one when the lint result has one or more errors. The subcommand SHALL exit with status two when no vault root can be located.

#### Scenario: Lint exits zero on clean vault

- **WHEN** `codebus lint` runs against a vault whose lint reports zero errors and zero warnings
- **THEN** the subcommand SHALL exit with status zero

#### Scenario: Lint exits zero with warnings only

- **WHEN** `codebus lint` runs against a vault whose lint reports zero errors and one or more warnings
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a representation of the warnings

#### Scenario: Lint exits one with errors

- **WHEN** `codebus lint` runs against a vault whose lint reports one or more errors
- **THEN** the subcommand SHALL exit with status one

#### Scenario: Lint never modifies vault content

- **WHEN** `codebus lint` runs against a vault containing files that would trigger lint errors
- **THEN** every file under `<vault-root>/wiki/`, `<vault-root>/raw/`, and `<vault-root>/log/` SHALL be byte-identical before and after the invocation AND no nested-git commit SHALL be produced

#### Scenario: Lint --format json default is text

- **WHEN** `codebus lint` is invoked without `--format`
- **THEN** stdout SHALL be human-readable text AND SHALL NOT parse as a single JSON value

#### Scenario: Lint --format json emits a single JSON value

- **WHEN** `codebus lint --format json` is invoked
- **THEN** stdout SHALL parse as a single valid JSON object containing the fields specified by the `lint-feedback-loop` capability's Lint Output Formats requirement


<!-- @trace
source: v3-lint
updated: 2026-05-09
code:
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/config/mod.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/wiki/fix/session.rs
  - codebus-core/src/wiki/lint/locate.rs
  - codebus-core/src/wiki/lint/rules/root_page.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/wiki/lint/rules/missing_nav.rs
  - codebus-core/src/wiki/lint/rules/broken_wikilink.rs
  - codebus-cli/Cargo.toml
  - codebus-cli/src/commands/fix.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/wiki/mod.rs
  - codebus-core/src/wiki/fix/prompt.rs
  - codebus-core/src/wiki/lint/rules/duplicate_slug.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/wiki/lint/rules/mod.rs
  - codebus-core/src/wiki/lint/output.rs
  - codebus-cli/src/commands/query.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/wiki/lint/rules/frontmatter_integrity.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-cli/src/commands/lint.rs
  - codebus-core/src/wiki/lint/factory.rs
  - codebus-core/src/wiki/lint/mod.rs
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/wiki/frontmatter.rs
  - codebus-core/src/wiki/lint/rule.rs
  - codebus-core/src/wiki/types.rs
tests:
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/lint_flow.rs
  - codebus-core/tests/vault_init.rs
  - codebus-cli/tests/cli_routing.rs
-->

---
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

---
### Requirement: Banner Output for Verb Commands

The system SHALL render run-lifecycle messages as a sequence of structured banners with codebus brand identity (the bus / boarding metaphor). The banner set SHALL contain at minimum these ten variants, each carrying a fixed set of payload fields: `Start { repo_path }`, `Goal { goal_text }`, `SyncStart`, `SyncDone { files, mib, elapsed_ms }`, `PiiSummary { scanner, scanned, hits, action }`, `LintStart`, `LintDone { errors, warns, elapsed_ms }`, `CommitDone { sha7 }`, `Done { wiki_path }`, `Hint { wiki_path }`. Each banner SHALL render to a single stdout line. The `PiiSummary` `action` field SHALL be formatted as `critical=<X>, warn=<Y>` where `<X>` is always `mask` (security floor per `pii-filter` capability `On-Hit Policy Default`) and `<Y>` is the resolved Warn-severity policy from `pii.on_hit` (one of `warn` / `skip` / `mask`). The system SHALL provide both an emoji-leading form (e.g., `🚌 來囉來囉~ CodeBus 駛入 <path>...` for `Start`) and a symbol-leading fallback form (e.g., `▶ 來囉來囉~ CodeBus 駛入 <path>...`); selection between forms SHALL be governed by the `Environment-Aware Output Styling` requirement. The verb command modules (`init`, `goal`, `query`, `fix`, `lint`) SHALL invoke the appropriate banner sequence at lifecycle transitions in their own stdout (NOT in the spawned agent's stdout — agent output remains a passthrough).

#### Scenario: Start banner appears at verb invocation

- **WHEN** any of `codebus init`, `codebus goal "X"`, `codebus query "X"`, `codebus fix`, or `codebus lint` is invoked against a valid repo, in default output mode with emoji enabled
- **THEN** the first stdout line SHALL be the `Start` banner — a single line containing `🚌` and the resolved repo path

#### Scenario: Done banner appears at successful completion

- **WHEN** `codebus init` or `codebus goal "X"` runs to successful completion in default output mode with emoji enabled
- **THEN** the last stdout banner before process exit SHALL be the `Done` banner — a single line containing `🎉` and the wiki path

#### Scenario: SyncDone banner reports counts and elapsed time

- **WHEN** the raw mirror sync runs (during init OR during goal with drift detected)
- **THEN** stdout SHALL contain the `SyncDone` banner — a single line whose body matches the pattern `<files> 檔, <mib> MiB, <elapsed_ms> ms` for some integer `files`, decimal `mib`, and integer `elapsed_ms`

#### Scenario: CommitDone banner reports short SHA

- **WHEN** the vault auto-commit step produces a non-empty commit
- **THEN** stdout SHALL contain the `CommitDone` banner — a single line whose body contains the 7-character short SHA

##### Example: init banner sequence on a fresh repo

- **GIVEN** a fresh git repo with one tracked file, Obsidian installed and registered, default `pii.on_hit` (Warn)
- **WHEN** `codebus init` runs in default mode with emoji enabled
- **THEN** stdout banner sequence SHALL be: `🚌 來囉來囉~ CodeBus 駛入 ./...`, `✓ 同步完成 (1 檔, 0.0 MiB, <ms> ms)`, `🛡 PII：regex_basic, scanned 1, hits 0, action critical=mask, warn=warn`, `📌 commit <sha7>`, `🎉 掰掰~下車囉！wiki 已生成於 ./.codebus/wiki`, `💡 請用 Obsidian 開 ./.codebus/wiki`

#### Scenario: PiiSummary action field reflects per-severity dispatch

- **WHEN** `codebus init` runs with `pii.on_hit: mask` configured
- **THEN** the `PiiSummary` banner action field SHALL contain the substring `critical=mask, warn=mask`

##### Example: PiiSummary action under default Warn policy

- **GIVEN** a config file omitting `pii.on_hit` (so the loader applies the default)
- **WHEN** `codebus init` runs and emits the `PiiSummary` banner
- **THEN** the banner body SHALL contain the substring `action critical=mask, warn=warn`

#### Scenario: Symbol fallback used when emoji disabled

- **WHEN** `codebus init` runs in an environment where emoji are disabled (non-TTY OR `NO_COLOR=1`)
- **THEN** the `Start` banner SHALL begin with `▶` (not `🚌`) AND the `Done` banner SHALL begin with `✓` (not `🎉`) AND the `LintStart` banner SHALL begin with `~` (not `🔍`)


<!-- @trace
source: v3-pii-severity-dispatch
updated: 2026-05-10
code:
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/config/pii.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/raw_sync.rs
-->

---
### Requirement: Environment-Aware Output Styling

The system SHALL detect terminal capabilities once at process start and apply them uniformly across all banners and lint text output. Three independent capability flags SHALL be derived: `use_emoji` SHALL be true when stdout is a TTY (per `std::io::IsTerminal`) and false otherwise; `use_color` SHALL be true when stdout is a TTY AND the environment variable `NO_COLOR` is unset; `use_hyperlinks` SHALL be true when `use_color` is true AND the `supports-hyperlinks` crate reports stdout supports OSC 8 hyperlinks. The system SHALL NOT consult `~/.codebus/config.yaml` for any output-styling field (no `emoji:` key, no `color:` key, no `hyperlinks:` key). The system SHALL NOT recognize a `--emoji on|off`, `--no-emoji`, or `NO_EMOJI` env override; presence of these inputs SHALL either fail clap parsing (for the flags) or be silently ignored (for the env var).

#### Scenario: NO_COLOR disables ANSI color but keeps emoji

- **WHEN** `codebus init` runs with `NO_COLOR=1` set in the environment, in a TTY
- **THEN** stdout banners SHALL still include emoji glyphs AND lint output SHALL NOT contain any ANSI escape sequence (no `\x1b[31m`, no `\x1b[33m`, no `\x1b]8;;...\x1b\\`)

#### Scenario: Non-TTY pipe disables emoji and color and hyperlinks

- **WHEN** `codebus init` is invoked with stdout redirected to a file or piped to another process
- **THEN** stdout content SHALL NOT contain any emoji glyph (no `🚌`, no `🎉`, no `🛡`) AND SHALL NOT contain any ANSI escape sequence — banners use only the symbol-fallback form

#### Scenario: --emoji flag is rejected by clap

- **WHEN** the user invokes `codebus init --emoji on`
- **THEN** clap argument parsing SHALL reject the unknown `--emoji` flag AND the binary SHALL exit with non-zero status

#### Scenario: NO_EMOJI env is silently ignored

- **WHEN** `codebus init` runs with `NO_EMOJI=1` set in the environment, in a TTY (with `NO_COLOR` unset)
- **THEN** stdout banners SHALL still include emoji glyphs (the env var has no observable effect on output)

#### Scenario: Detection runs once per process

- **WHEN** any single verb invocation emits multiple banners over its lifecycle
- **THEN** the emoji / color / hyperlink decisions SHALL be consistent across every banner in that invocation (no per-banner re-detection)

<!-- @trace
source: v3-render-polish
updated: 2026-05-10
code:
  - codebus-core/src/render/lint_text.rs
  - docs/v3-roadmap.md
  - codebus-cli/src/commands/lint.rs
  - codebus-core/src/render/banner.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-cli/src/main.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/render/mod.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/render/options.rs
  - codebus-cli/src/commands/query.rs
  - Cargo.toml
  - codebus-core/src/vault/obsidian_register.rs
  - codebus-core/Cargo.toml
  - codebus-core/src/lib.rs
  - codebus-core/src/wiki/lint/output.rs
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/fix_flow.rs
-->

---
### Requirement: Verb RunLog Capture and Persistence

The `goal`, `query`, `fix`, `chat`, and `quiz` subcommands SHALL each capture `RunLog` entries per the `run-log` capability `RunLog Schema and Per-Invocation Capture` requirement and SHALL persist them to the configured `LogSink` (resolved per the `run-log` capability `Log Configuration Schema` and `Default Log Directory Resolution` requirements).

For `goal`, `query`, and `fix`, exactly one `RunLog` entry SHALL be appended per invocation; the persistence step SHALL run as the verb's penultimate action — after the auto-commit step (where applicable) and before the `Done` banner — so that the entry includes the final `wiki_changed`, `lint_error_count`, and `lint_warn_count` values.

For `chat`, exactly one `RunLog` entry SHALL be appended per chat turn (the REPL invokes `run_chat_turn` once per turn; each call writes one entry). Each chat turn's `RunLog` SHALL have `mode == "chat"` AND `session_id == Some(session_id_from_init_event)`. The `chat` subcommand SHALL NOT write a separate aggregate `RunLog` entry for the REPL session as a whole — only the per-turn entries.

When a `chat` turn culminates in a confirmed promote-to-goal, the spawned `codebus goal` child subprocess SHALL itself write its own `RunLog` entry (with `mode == "goal"` and no `session_id` field) per the same `run-log` capability rules; the parent `codebus chat` process SHALL NOT additionally write that entry.

For `quiz`, exactly one `RunLog` entry SHALL be appended per `codebus quiz` invocation. It SHALL be written by the generate spawn (`run_quiz_generate`); the plan sub-step (`run_quiz_plan`) SHALL NOT write a `RunLog`. The quiz `RunLog` SHALL have `mode == "quiz"`, `goal` equal to the comma-joined selected page paths, AND `session_id` carrying the generate spawn's id (recorded for logging only, NOT used for resume), per the `run-log` capability and the `quiz` capability.

When the verb fails (agent crash, lint phase failure, auto-commit failure), the persistence step SHALL still run so the `RunLog` records the partial-state outcome; the verb SHALL NOT skip log persistence on its failure paths. When the `LogSink::write_run` call returns an error, the verb SHALL emit a stderr warning prefixed with `warning: run-log` and SHALL NOT propagate the failure into its exit code (per the `run-log` capability `RunLog Write Failure Is Non-Fatal` requirement).

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

#### Scenario: Chat turn writes RunLog with mode chat and session_id

- **WHEN** the user enters one prompt during a `codebus chat` session AND the chat turn completes successfully
- **THEN** the run-log jsonl SHALL gain exactly one new line whose `mode` field equals `"chat"` AND whose `session_id` field equals `Some(<session_id_from_init_event>)`

#### Scenario: Promote-to-goal subprocess writes its own RunLog

- **WHEN** a chat turn culminates in confirmed promote AND the spawned `codebus goal` subprocess completes successfully
- **THEN** the run-log jsonl SHALL gain one additional line (beyond the chat turn's row) with `mode == "goal"` AND SHALL NOT contain a `session_id` field on that goal row

#### Scenario: Quiz subcommand appends one RunLog with mode quiz

- **WHEN** `codebus quiz "auth"` runs the generate spawn to completion against a vault with selected pages `["wiki/modules/auth.md"]`
- **THEN** the file `<vault>/.codebus/log/runs-<YYYY-MM-DD>.jsonl` SHALL gain exactly one new line whose `mode` field equals `"quiz"` AND whose `goal` field equals `"wiki/modules/auth.md"` AND no separate `RunLog` line SHALL be written for the plan sub-step


<!-- @trace
source: run-log-spec-include-quiz
updated: 2026-05-30
code:
  - codebus-cli/src/main.rs
tests:
  - codebus-cli/tests/quiz_flow.rs
-->

---
### Requirement: Config Parse Failure Aborts Invocation

Every `codebus` subcommand that reads `~/.codebus/config.yaml` SHALL distinguish three load outcomes and behave deterministically for each:

1. **File missing** (`io::ErrorKind::NotFound`) — the system SHALL apply the corresponding section's `Default::default()` and proceed with the invocation. This preserves first-time-setup ergonomics and is unchanged from the prior behavior.
2. **Load succeeds** — the system SHALL use the parsed configuration and proceed. Unknown keys SHALL remain silently ignored (forward-compat); fields absent from the document SHALL be filled with section defaults. Both behaviors are unchanged from the prior behavior.
3. **File exists but a config-section loader returns `Err`** (yaml syntax error, schema validation failure such as an invalid `SystemModel` enum value, missing required field under an active profile, or any other `ConfigLoadError`) — the system SHALL emit a stderr message naming the failing section AND the parse-error detail returned by the loader, AND SHALL exit with non-zero status, AND SHALL NOT perform any side effect that depends on the broken section (no `claude` child spawn, no keyring read/write/delete, no wiki file write, no run-log append, no auto-commit).

The third outcome applies to every codebus subcommand whose execution depends on the broken section: `goal`, `query`, `fix`, and `config` (including all three `config` sub-actions `set-key`, `get-key`, `delete-key`). When the broken section is unused by a given subcommand the system MAY proceed; however the helper implementations defined by this change apply fail-loud uniformly across every helper (`load_pii_config_with_warning`, `load_claude_code_config_with_warning`, `load_log_config_with_warning`, the inline `load_lint_fix_config` handlers in `goal.rs` / `fix.rs`, AND `read_azure_keyring_service_from_config` in `commands/config.rs`) so callers cannot accidentally skip the gate.

#### Scenario: Yaml syntax error aborts goal verb before agent spawn

- **WHEN** `~/.codebus/config.yaml` contains a yaml syntax error (e.g. missing colon on the `pii` key) AND the user runs `codebus goal "ingest X"`
- **THEN** the binary SHALL exit with non-zero status AND stderr SHALL contain a parse-error message naming the failing section AND no `claude` child process SHALL be spawned AND no wiki file under `<vault>/wiki/` SHALL be created or modified

#### Scenario: Invalid SystemModel value aborts query verb

- **WHEN** `~/.codebus/config.yaml` contains `claude_code.system.goal.model: gpt-4` (a value rejected by the `SystemModel` enum) AND the user runs `codebus query "what is X"`
- **THEN** the binary SHALL exit with non-zero status AND stderr SHALL contain a message naming `claude_code.system.goal.model` AND the invalid variant text AND no `claude` child process SHALL be spawned

#### Scenario: Yaml syntax error aborts config delete-key before keyring access

- **WHEN** `~/.codebus/config.yaml` contains a yaml syntax error AND the user runs `codebus config delete-key azure`
- **THEN** the binary SHALL exit with non-zero status AND stderr SHALL contain a parse-error message AND the keyring entry for any service name SHALL remain unchanged (no `Entry::delete_credential` call)

#### Scenario: Missing config file preserves default behavior

- **WHEN** `~/.codebus/config.yaml` does not exist AND the user runs `codebus goal "X"`
- **THEN** the binary SHALL proceed with each section's `Default::default()` AND SHALL NOT emit any parse-error message on stderr

#### Scenario: Unknown key under valid yaml does not trigger fail-loud

- **WHEN** `~/.codebus/config.yaml` parses cleanly under every section loader but contains a `future_field: hello` key the binary does not recognise AND the user runs `codebus query "X"`
- **THEN** the binary SHALL proceed normally AND SHALL NOT emit any parse-error message on stderr AND the unknown key SHALL have no observable effect on the invocation

##### Example: behavior matrix

| Config file state                        | All verb commands  | `config set-key` | `config get-key` | `config delete-key` |
| ---------------------------------------- | ------------------ | ---------------- | ---------------- | ------------------- |
| File absent                              | Proceed (defaults) | Proceed          | Proceed          | Proceed             |
| Parses cleanly, unknown keys present     | Proceed            | Proceed          | Proceed          | Proceed             |
| Parses cleanly, recognised values        | Proceed            | Proceed          | Proceed          | Proceed             |
| Yaml syntax error                        | Abort, exit ≠ 0    | Abort            | Abort            | Abort               |
| Schema validation failure in any section | Abort, exit ≠ 0    | Abort            | Abort            | Abort               |

---
### Requirement: Spawn Verb Library Delegation

The CLI subcommand handlers for the three thin-wrapper spawn verbs (`codebus-cli/src/commands/goal.rs`, `codebus-cli/src/commands/query.rs`, `codebus-cli/src/commands/fix.rs`) SHALL act as thin wrappers that delegate orchestration to `codebus_core::verb::goal::run_goal`, `codebus_core::verb::query::run_query`, and `codebus_core::verb::fix::run_fix` respectively (see the `verb-library` capability). Each handler SHALL be responsible for: (1) clap argument parsing into the verb-specific options struct, (2) constructing a closure that maps `VerbEvent::Banner(_)` to `print_banner` and `VerbEvent::Stream(_)` to `print_event` against the active `RenderOptions`, (3) calling the library function with the closure and a `None` cancel signal, (4) matching exhaustively on `VerbError` to derive the exit code preserving the existing per-error mapping, and (5) writing the resulting `RunLog` entry via the shared run-log persistence helpers.

The CLI subcommand handler for the `chat` verb (`codebus-cli/src/commands/chat.rs`) SHALL NOT follow the thin-wrapper one-shot pattern. The chat handler SHALL instead implement a REPL loop that calls `run_chat_turn` once per user turn, maintains transcript state across turns, registers a SIGINT trap to flip an `AtomicBool` cancel flag passed as `cancel: Some(flag)` to each `run_chat_turn` invocation, observes `VerbLifecycleEvent::PromoteSuggestion` events emitted via `on_event`, prompts the user for `(y/n)` confirmation on promote suggestions, and (when confirmed) spawns a `codebus goal` child subprocess (NOT a `run_goal` library call). The detailed REPL contract is defined normatively in the `chat-verb` capability `Chat CLI Subcommand Behavior` requirement and related requirements.

#### Scenario: Goal handler is a thin wrapper

- **WHEN** the `codebus-cli/src/commands/goal.rs` handler is invoked for `codebus goal "X"`
- **THEN** the handler SHALL parse clap args AND call `run_goal` exactly once AND match exhaustively on `VerbError` AND SHALL NOT contain a stdin read loop AND SHALL pass `cancel: None`

#### Scenario: Chat handler runs a REPL with cancel signal

- **WHEN** the `codebus-cli/src/commands/chat.rs` handler is invoked for `codebus chat`
- **THEN** the handler SHALL register a SIGINT trap AND enter a stdin read loop AND call `run_chat_turn` once per user turn AND pass `cancel: Some(flag)` on every call AND NOT pass `cancel: None`

#### Scenario: Promote confirmation spawns subprocess not library call

- **WHEN** the chat REPL receives `VerbLifecycleEvent::PromoteSuggestion { reason }` AND the user confirms with `y`
- **THEN** the handler SHALL spawn a `codebus goal "<transcript>"` child subprocess via `std::process::Command` AND SHALL NOT call `codebus_core::verb::goal::run_goal` from within the chat command

---
### Requirement: Chat Subcommand Behavior

The `codebus chat` subcommand SHALL accept zero positional arguments and SHALL accept the standard global flags (`--debug`, `--no-emoji`, `--no-color`, etc., resolved per `Environment-Aware Output Styling`). When invoked, the subcommand SHALL launch the interactive multi-turn chat REPL per the `chat-verb` capability `Chat CLI Subcommand Behavior` requirement, using the read-only sandbox toolset `CHAT_TOOLSET` (`Read,Glob,Grep`) passed verbatim to `agent::invoke` per the `Cancellation Signal Polling` requirement of the `verb-library` capability.

The chat subcommand SHALL exit with status zero when the user requests REPL exit (`exit`, `:q`, Ctrl+D, or second Ctrl+C). The chat subcommand SHALL exit with non-zero status only when an unrecoverable `VerbError` other than `Cancelled` propagates from `run_chat_turn` (in which case the per-error exit code mapping defined by the `verb-library` capability `Verb Error Enum` requirement applies).

The chat subcommand SHALL operate under the standard vault precondition: when `<repo>/.codebus/` is absent, the first `run_chat_turn` invocation returns `VerbError::VaultMissing` and the CLI SHALL print a stderr message instructing the user to run `codebus init` first AND exit with status 2 before entering the REPL loop.

#### Scenario: Chat exits zero on REPL exit alias

- **WHEN** the user types `exit\n` at any chat REPL prompt
- **THEN** the `codebus chat` process SHALL exit with status zero

#### Scenario: Chat aborts on missing vault before first turn

- **WHEN** `codebus chat` is invoked against a directory where `<cwd>/.codebus/` does not exist
- **THEN** the process SHALL print a stderr message indicating the vault is missing AND SHALL exit with status 2 AND SHALL NOT enter the REPL loop

#### Scenario: Chat accepts no positional args

- **WHEN** `codebus chat "extra positional"` is invoked
- **THEN** clap SHALL reject the invocation AND SHALL emit a usage error to stderr AND SHALL exit with non-zero status

---
### Requirement: Quiz Subcommand Behavior

`codebus quiz "<topic>"` SHALL invoke `codebus_core::verb::quiz::run_quiz` with `QuizScope::Goal { text: <topic> }`. The subcommand SHALL accept an optional `--count <N>` flag (integer 3–10). When `--count` is omitted, the subcommand SHALL resolve `question_count` from the shared `quiz.default_length` config key, defaulting to 5 when that key is absent. The subcommand SHALL pass the agent the sandbox flags `--tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits`. The subcommand SHALL be read-only and SHALL NOT auto-commit. The CLI SHALL NOT present an interactive scope-confirmation gate (the confirm gate is a GUI-only affordance); after the plan spawn emits scope, the CLI SHALL print the planned page list and proceed directly to the generate spawn. The CLI SHALL persist the resulting quiz file with caller-injected frontmatter per the `quiz` capability.

Exit status SHALL be zero on a successful quiz generation. A `[CODEBUS_QUIZ_NO_MATCH]` outcome SHALL be treated as a successful run (exit zero) with the no-match reason printed to stdout and no quiz file written. A spawn failure or `VerbError` SHALL produce a non-zero exit status.

#### Scenario: Quiz with explicit count

- **WHEN** `codebus quiz "JWT lifecycle" --count 7` is invoked against a vault whose wiki covers JWT
- **THEN** `run_quiz` SHALL be called with `question_count = 7` AND a quiz file with seven question sections SHALL be persisted AND the process SHALL exit zero

#### Scenario: Quiz count falls back to config then default

- **WHEN** `codebus quiz "auth"` is invoked with no `--count` AND `quiz.default_length` is absent from config
- **THEN** `run_quiz` SHALL be called with `question_count = 5`

#### Scenario: No-match exits zero without a file

- **WHEN** `codebus quiz "quantum mechanics"` is invoked against a vault whose wiki does not cover the topic
- **THEN** the CLI SHALL print the `[CODEBUS_QUIZ_NO_MATCH]` reason to stdout AND SHALL NOT write any quiz file AND SHALL exit with status zero

#### Scenario: Quiz does not auto-commit

- **WHEN** `codebus quiz "<topic>"` completes successfully in a git working tree
- **THEN** the subcommand SHALL NOT create any git commit

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
### Requirement: Quiz Validate Sub-Action Behavior

The `codebus quiz validate <quiz-md-file | ->` sub-action SHALL run the deterministic quiz validator (defined normatively in the `quiz` capability's `Quiz Output Validation and Repair` requirement) over a quiz markdown body and SHALL share the same underlying validator function the library final-verify uses. The body source SHALL be either a file path argument OR standard input when the argument is `-` or omitted. The sub-action SHALL accept an OPTIONAL `--count <N>` flag (integer): when supplied, the sub-action SHALL pass `N` to the validator as the expected question count so the validator can flag a question-count mismatch (per the `quiz` capability); when omitted, the validator SHALL receive no expected count and SHALL NOT check the question count. Human output SHALL report zero issues and exit with status zero when the body has no findings, SHALL list each finding (question identifier, rule, message) and exit with status one when findings exist, and SHALL exit with status two on a setup error (no locatable vault, or an unreadable file argument). With `--json`, the sub-action SHALL emit a machine-readable findings array (each entry carrying at least `rule`, `severity`, the question identifier, and a message) and SHALL apply the same exit-code contract.

The stdin source exists so the codebus-quiz generate agent can self-validate the draft held in its context without first writing it to disk. This avoids a scratch-file lifecycle plus a write-then-emit double-write for a verb whose deliverable is a stdout body persisted by the caller; it is a process-simplicity choice and is NOT motivated by sandbox least-privilege (the `goal` and `fix` agents already run with un-gated vault Write).

The codebus-quiz agent's generate spawn sandbox SHALL grant the agent a Bash tool hard-gated to invoking only `codebus quiz validate` (whitelist specifier of the form `Bash(codebus quiz validate *)`), installed via the same PreToolUse hook mechanism the `lint-feedback-loop` capability defines for the codebus-fix agent. The `--count <N>` flag falls within that wildcard suffix and SHALL NOT require a hook change. The generate toolset SHALL NOT add Write or Edit (the agent has no scratch file to write — it pipes its in-context draft via stdin). The always-blocked tool set (WebFetch, WebSearch, Task, MCP, and the other globally forbidden tools) SHALL remain blocked.

#### Scenario: Clean file exits zero

- **WHEN** `codebus quiz validate <file>` runs over a structurally valid quiz whose citations all resolve
- **THEN** human output SHALL report zero issues AND the process SHALL exit with status zero

#### Scenario: Findings exit one with details

- **WHEN** `codebus quiz validate <file>` runs over a quiz with a malformed question or a broken `[[slug]]` citation
- **THEN** the output SHALL list each finding with its question identifier and rule AND the process SHALL exit with status one

#### Scenario: Count flag flags a question-count mismatch

- **WHEN** `codebus quiz validate <file> --count 5` runs over a structurally valid quiz body that contains nine question blocks
- **THEN** the output SHALL list a question-count finding stating the expected count (five) and the actual count (nine) AND the process SHALL exit with status one

#### Scenario: Count flag omitted skips the count check

- **WHEN** `codebus quiz validate <file>` runs without `--count` over an otherwise valid quiz body that contains nine question blocks
- **THEN** the output SHALL report zero issues AND the process SHALL exit with status zero (the question count is not checked)

#### Scenario: JSON output is machine-readable

- **WHEN** `codebus quiz validate <file> --json` runs over a quiz with findings
- **THEN** the output SHALL be a JSON findings array where each entry carries at least `rule`, `severity`, the question identifier, and a message AND the process SHALL exit with status one

#### Scenario: Body is read from stdin when the argument is `-`

- **WHEN** a quiz markdown body is piped to `codebus quiz validate -`
- **THEN** the validator SHALL run over the piped body AND SHALL apply the same finding output and exit-code contract as the file-path form

#### Scenario: Agent Bash is hard-gated to quiz validate only

- **WHEN** the codebus-quiz generate spawn agent attempts a Bash command other than `codebus quiz validate ...`
- **THEN** the PreToolUse hook SHALL block it AND only `codebus quiz validate ...` invocations SHALL be permitted


<!-- @trace
source: quiz-validate-enforce-question-count
updated: 2026-06-07
code:
  - codebus-core/src/verb/quiz_validate.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/commands/quiz.rs
tests:
  - codebus-cli/tests/quiz_validate_subaction.rs
  - codebus-cli/tests/quiz_flow.rs
-->

---
### Requirement: Quiz Content Verify CLI Behavior

The `codebus quiz "<topic>"` subcommand SHALL participate in the optional content verification stage defined by the `quiz` capability's `Quiz Content Verification and Repair` requirement, without introducing any new top-level subcommand or sub-action (content verification is an internal spawn; Subcommand Registration is unchanged).

The CLI SHALL resolve `quiz.content_verify` from the shared `quiz.*` config namespace (default `false`; the CLI SHALL NOT read the app-only `app.*` namespace for this key). When `false`, `codebus quiz` behaviour SHALL be unchanged from the deterministic-only flow. When `true`, the CLI SHALL pass the originating topic (the `"<topic>"` argument — the CLI is always the Goal flow) into `run_quiz_generate` so the off-topic defect check can run, and the live stream SHALL render the additional verify and repair spawns through the existing agent stream rendering. The persisted quiz file's caller frontmatter SHALL include the `content_review` field as defined by the `quiz` capability. Exit status SHALL remain unchanged by content verification (residual content defects are best-effort, not a failure).

#### Scenario: Default off leaves CLI flow unchanged

- **WHEN** `codebus quiz "<topic>"` runs and `quiz.content_verify` is absent or `false`
- **THEN** no verify spawn SHALL run AND the persisted quiz SHALL NOT contain a `content_review` field AND the exit status contract SHALL be unchanged

#### Scenario: Enabled CLI threads topic and surfaces stream

- **WHEN** `codebus quiz "JWT lifecycle"` runs with `quiz.content_verify` set to `true`
- **THEN** the originating topic SHALL be supplied to `run_quiz_generate` for the off-topic check AND the verify/repair spawns SHALL be rendered in the live stream AND the persisted frontmatter SHALL carry `content_review`

#### Scenario: No new subcommand is registered

- **WHEN** `codebus --help` is invoked with this change applied
- **THEN** the top-level subcommand list SHALL be unchanged AND no content-verify subcommand or `quiz` sub-action SHALL be added

<!-- @trace
source: quiz-content-verify
updated: 2026-05-19
code:
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/config/quiz.rs
  - codebus-cli/src/commands/quiz.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-app/src-tauri/src/ipc/quiz.rs
tests:
  - codebus-cli/tests/quiz_flow.rs
  - codebus-core/tests/verb_library_surface.rs
  - codebus-cli/tests/bins/mock_claude.rs
-->

---
### Requirement: Goal Content Verify CLI Behavior

The `codebus goal "<goal>"` subcommand SHALL participate in the optional content verification stage defined by the `verb-library` capability's `Goal Content Verification and Repair` requirement, without introducing any new top-level subcommand (the registered subcommand set is unchanged; content verification is an internal stage of `run_goal`).

The CLI SHALL resolve `goal.content_verify` from the shared `goal.*` config namespace (default `false`; the CLI SHALL NOT read the app-only `app.*` namespace for this key). When `false`, `codebus goal` behavior SHALL be unchanged from the deterministic-only flow. When `true`, the CLI SHALL pass the originating goal text into `run_goal` so the off-goal defect check can run, the live stream SHALL render the additional verify and repair spawns through the existing agent stream rendering, and the run-log / `GoalReport` SHALL reflect the content-review status. The subcommand exit status and `auto_commit` behavior SHALL be unchanged by content verification (residual content defects are best-effort, not a failure, and never block the commit).

#### Scenario: Default off leaves CLI flow unchanged

- **WHEN** `codebus goal "<goal>"` runs and `goal.content_verify` is absent or `false`
- **THEN** no verify spawn SHALL run AND the exit code and `auto_commit` behavior SHALL be unchanged

#### Scenario: Enabled CLI runs the stage and surfaces the stream

- **WHEN** `codebus goal "describe auth"` runs with `goal.content_verify` set to `true`
- **THEN** the originating goal text SHALL be supplied to `run_goal` for the off-goal check AND the verify/repair spawns SHALL be rendered in the live stream AND the run reflects the content-review status

#### Scenario: No new subcommand is registered

- **WHEN** `codebus --help` is invoked with this change applied
- **THEN** the top-level subcommand list SHALL be unchanged AND no content-verify subcommand SHALL be added

<!-- @trace
source: goal-content-verify
updated: 2026-05-19
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
### Requirement: Global Config Starter Content Shape

The global config starter content written by the `write_starter_config_if_missing` primitive SHALL consist of a single shared header comment block followed by pure default key/value pairs. The starter SHALL NOT carry inline per-field teaching comments. The header SHALL be a short comment block (at most a few lines) that points the reader to the field reference documentation instead of embedding field documentation in the config file itself.

The header SHALL be defined as a single source-of-truth constant named `CONFIG_HEADER`, exported from codebus-core, so the exact same header text is reused by the app's config save path (see the `app-shell` capability's Config Save Hygiene requirement). Field-level teaching that previously lived in inline comments SHALL be migrated to a dedicated documentation file at `docs/config-reference.md`.

Removing inline comments SHALL NOT change any default value: the starter body SHALL continue to round-trip through every section loader to that section's `Default::default()`.

#### Scenario: Starter begins with the shared header and omits inline field comments

- **WHEN** `write_starter_config_if_missing` writes a new starter file because none exists
- **THEN** the written content SHALL begin with the shared `CONFIG_HEADER` block AND SHALL NOT contain inline per-field teaching comments beyond that header

#### Scenario: Starter still round-trips to defaults

- **WHEN** the starter content is loaded back through the PII, agent, hooks, lint, and log section loaders
- **THEN** each loaded section SHALL equal its `Default::default()` value

<!-- @trace
source: config-save-robustness
updated: 2026-06-28
code:
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/config/mod.rs
tests:
  - codebus-core/src/config/global_starter.rs
-->
