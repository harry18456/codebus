# cli Specification Delta — v3-lint

## REMOVED Requirements

### Requirement: Stub Verb Exit Behavior

**Reason**: All four subcommands (`goal`, `query`, `lint`, `fix`) now have explicit Subcommand Behavior requirements. `goal` and `query` were given explicit specs in archived changes `v3-goal` and `v3-query`; `lint` and `fix` are given explicit specs in the present change. No verbs remain as stubs in the binary.

**Migration**: None required. The four explicit Subcommand Behavior requirements (Init / Goal / Query / Lint / Fix) supersede this catch-all stub specification. Implementations of `goal` and `query` are unchanged; `lint` and `fix` gain explicit non-stub behavior in this change.

#### Scenario: No subcommand emits the not-yet-implemented stub message after this change

- **WHEN** any of `codebus goal`, `codebus query`, `codebus lint`, or `codebus fix` is invoked
- **THEN** the binary SHALL execute that subcommand's behavior per its respective Subcommand Behavior requirement AND stderr SHALL NOT contain the literal substring `not yet implemented`

## MODIFIED Requirements

### Requirement: Goal Subcommand Behavior

The `goal` subcommand SHALL accept one positional argument `<goal-text>` (the user's natural-language goal description) and the following flags: `--repo <PATH>` (default: current working directory), `--force-resync` (boolean, force raw mirror re-sync even when source-signal detection reports no drift), `--no-obsidian-register` (boolean, forwarded to the auto-init fallback), `--no-fix` (boolean, skip the post-agent lint-and-fix phase entirely), `--fix-max-iter <N>` (positive integer, override the fix loop's `outer_ping_max` for this invocation), and the inherited `--debug` global flag. When invoked, the subcommand SHALL execute exactly six steps in order: (1) resolve the target repository path, (2) when `<repo>/.codebus/` does not exist, run the `init` flow against the same repo (forwarding `--no-obsidian-register`); (3) perform source-signal drift detection and conditionally re-sync the raw mirror plus update the manifest; (4) spawn the goal agent CLI with the canonical sandbox flags described below and stream child stdout and stderr to the parent process; (5) after the goal agent terminates, run the lint-and-fix phase against the vault unless `--no-fix` is present or `lint.fix.enabled` is `false` in config; (6) after the lint-and-fix phase terminates (or is skipped), invoke the vault auto-commit operation with the message `wiki: <goal-text>` against the nested vault repository. The subcommand SHALL exit with the goal agent's exit code when the goal agent exited non-zero, otherwise with the fix loop's exit code, unless the auto-commit operation itself fails, in which case it SHALL exit with a non-zero code identifying the auto-commit failure.

The goal agent spawn SHALL pass the following arguments to the agent CLI: `-p` followed by the literal string `/codebus-goal "<goal-text>"`; `--tools` followed by the comma-separated string `Read,Glob,Grep,Write,Edit`; `--allowedTools` followed by the same comma-separated string; `--permission-mode` followed by `acceptEdits`. The goal agent's child process SHALL be invoked with the current working directory set to `<repo>/.codebus/` and its stdin SHALL be a closed stream (preventing the agent from blocking on input).

The lint-and-fix phase SHALL invoke the fix loop defined in the `lint-feedback-loop` capability, using the same vault root as the goal agent. The phase SHALL forward `--fix-max-iter <N>` to the fix loop when present. The phase SHALL be skipped when either `--no-fix` is present on the goal invocation or `lint.fix.enabled` is `false` in `~/.codebus/config.yaml`.

The auto-commit operation in step 6 SHALL use a single commit message `wiki: <goal-text>` covering both the goal agent's writes and any fix-loop edits, regardless of whether the lint-and-fix phase ran or how it terminated.

#### Scenario: Goal subcommand invokes auto-init when vault is missing

- **WHEN** `codebus goal "describe the auth flow"` is invoked against a repository whose `.codebus/` directory does not yet exist
- **THEN** the subcommand SHALL first run the init flow (creating `.codebus/`, vault layout, raw mirror, schema, manifest, skill bundles, nested git, and the initial `init: codebus vault` commit) AND afterwards SHALL proceed to spawn the goal agent against the freshly-initialized vault

#### Scenario: Goal subcommand spawns agent with cwd at vault root

- **WHEN** the goal subcommand reaches its agent-spawn step against a vault rooted at `<repo>/.codebus/`
- **THEN** the goal agent child process SHALL be spawned with current working directory equal to `<repo>/.codebus/` AND the child SHALL receive the prompt argument `/codebus-goal "<goal-text>"` AND the child's stdin SHALL be closed

#### Scenario: Goal subcommand passes the canonical triple-flag sandbox to the goal agent

- **WHEN** the goal subcommand spawns the goal agent
- **THEN** the spawned command line SHALL include the flag pair `--tools Read,Glob,Grep,Write,Edit`, the flag pair `--allowedTools Read,Glob,Grep,Write,Edit`, and the flag pair `--permission-mode acceptEdits`

#### Scenario: Goal flow runs lint-and-fix between goal agent termination and auto-commit

- **WHEN** the goal subcommand spawns the goal agent and the agent terminates after writing one or more files inside `<repo>/.codebus/wiki/`, with `--no-fix` absent and `lint.fix.enabled: true`
- **THEN** the lint-and-fix phase SHALL run against the vault before any auto-commit AND any file modifications produced by the fix loop SHALL be included in the same commit as the goal agent's writes

#### Scenario: Goal flow folds goal writes and fix edits into a single commit

- **WHEN** the goal subcommand completes the goal agent step (with vault changes), then completes the lint-and-fix phase (with additional vault changes), then reaches the auto-commit step
- **THEN** the auto-commit SHALL produce exactly one new commit on the nested vault git repo AND the commit message SHALL equal `wiki: <goal-text>`

#### Scenario: Goal flow skips fix when --no-fix is supplied

- **WHEN** `codebus goal "X" --no-fix` is invoked against a vault that would have lint issues after the goal agent runs
- **THEN** the goal subcommand SHALL NOT spawn any fix agent AND the auto-commit SHALL still produce a commit with message `wiki: X`

#### Scenario: Goal subcommand auto-commits on goal agent failure with partial writes

- **WHEN** the goal agent exits with non-zero status after writing one or more files inside `<repo>/.codebus/wiki/`
- **THEN** the lint-and-fix phase SHALL still run (unless skipped by `--no-fix` or config) AND afterwards the subcommand SHALL invoke vault auto-commit with message `wiki: <goal-text>` AND the subcommand SHALL propagate the goal agent's non-zero exit code

#### Scenario: Goal subcommand no-op commit when neither goal agent nor fix make changes

- **WHEN** the goal agent exits without modifying any file under `<repo>/.codebus/wiki/` AND the lint-and-fix phase produces no further changes
- **THEN** the auto-commit operation SHALL be a no-op (working tree clean) AND `git -C <repo>/.codebus rev-list --count HEAD` SHALL equal the same count as before the goal invocation

#### Scenario: Force-resync flag bypasses detection

- **WHEN** `codebus goal "..." --force-resync` is invoked against a vault whose source signal would otherwise be unchanged
- **THEN** the subcommand SHALL re-run the raw mirror unconditionally AND SHALL update the manifest's `last_sync_at` timestamp regardless of whether the source content changed

#### Scenario: Goal subcommand propagates goal agent non-zero exit code

- **WHEN** the goal agent terminates with non-zero exit status N
- **THEN** the goal subcommand SHALL exit with status N regardless of whether the fix loop subsequently succeeds, unless the auto-commit operation itself fails

---

### Requirement: Debug Flag Output

The `codebus` binary SHALL accept `--debug` as a global flag, available at the top-level command and inheritable by every subcommand (e.g., `codebus --debug init`, `codebus init --debug` SHALL behave equivalently). When `--debug` is set, the binary's verb handlers SHALL emit additional `[debug]` lines describing internal state and operations beyond the default `✓` progress lines. When `--debug` is NOT set, the binary SHALL NOT emit any line beginning with `[debug]`.

Each subcommand's debug output content SHALL be defined by that subcommand's Behavior requirement (Init Subcommand Behavior, Goal Subcommand Behavior, Query Subcommand Behavior, Lint Subcommand Behavior, Fix Subcommand Behavior). The binary SHALL NOT contain a separate stub debug behavior; every subcommand has explicit content guarantees.

#### Scenario: --debug flag accepted at top-level position

- **WHEN** `codebus --debug init --repo <path>` is invoked against a writable target
- **THEN** stdout SHALL contain at least one line beginning with `[debug]` AND init SHALL otherwise complete successfully AND exit with status zero

#### Scenario: --debug flag accepted at subcommand position

- **WHEN** `codebus init --debug --repo <path>` is invoked against a writable target
- **THEN** the binary SHALL behave identically to `codebus --debug init --repo <path>` (same exit code, same `[debug]` line presence)

#### Scenario: Without --debug, no debug lines are emitted

- **WHEN** `codebus init --repo <path>` runs successfully (no `--debug` flag anywhere)
- **THEN** stdout SHALL NOT contain any line beginning with `[debug]`

## ADDED Requirements

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

---

### Requirement: Fix Subcommand Behavior

The `fix` subcommand SHALL accept the following flags: `--repo <PATH>` (default: current working directory), `--no-fix` (boolean, when present the subcommand SHALL exit with status zero and a stderr message stating fix is disabled, without spawning any agent or running lint), `--fix-max-iter <N>` (positive integer, override `outer_ping_max` for this invocation), and the inherited `--debug` global flag. When invoked without `--no-fix`, the subcommand SHALL execute exactly five steps in order: (1) resolve the target repository path and verify `<repo>/.codebus/` exists (otherwise exit with status 2 and a stderr hint instructing the user to run `codebus init`), (2) run lint pre-check; if zero issues, exit with status zero and skip remaining steps, (3) run the fix loop defined by the `lint-feedback-loop` capability against the vault, (4) invoke vault auto-commit with message `wiki: lint fix loop`, (5) exit with status reflecting the post-loop lint state.

The fix subcommand SHALL NOT trigger source-signal drift detection, SHALL NOT call raw mirror sync, SHALL NOT update the vault manifest, SHALL NOT modify any source code outside the vault, and SHALL NOT spawn a goal-style agent. The only agent process the fix subcommand spawns SHALL be the fix agent defined by the Fix Loop Agent Sandbox requirement of the `lint-feedback-loop` capability.

The subcommand SHALL exit with status zero when the post-loop lint reports zero issues. The subcommand SHALL exit with status one when issues remain after exhausting the ping budget. The subcommand SHALL exit with status two when the vault precondition fails.

#### Scenario: Fix refuses when vault is missing

- **WHEN** `codebus fix --repo <repo>` is invoked against a path whose `.codebus/` directory does not exist
- **THEN** the subcommand SHALL exit with status 2 AND stderr SHALL contain a message instructing the user to run `codebus init` first AND no agent process SHALL be spawned

#### Scenario: Fix exits zero on clean vault

- **WHEN** `codebus fix` runs against a vault whose initial lint reports zero issues
- **THEN** no agent process SHALL be spawned AND the subcommand SHALL exit with status zero AND no new git commit SHALL be created

#### Scenario: Fix --no-fix flag disables the loop entirely

- **WHEN** `codebus fix --no-fix` is invoked against any vault
- **THEN** the subcommand SHALL exit with status zero AND no agent process SHALL be spawned AND no lint SHALL be performed AND stderr SHALL contain a message stating fix is disabled

#### Scenario: Fix auto-commits with lint fix loop message

- **WHEN** `codebus fix` runs and the fix loop produces at least one change under `wiki/`
- **THEN** the subcommand SHALL invoke vault auto-commit AND `git -C <repo>/.codebus log --pretty=%s -1` SHALL print exactly the line `wiki: lint fix loop`

#### Scenario: Fix exits one when issues remain

- **WHEN** `codebus fix` runs the loop, exhausts `outer_ping_max + 1` total agent invocations, and the final lint reports at least one issue
- **THEN** the subcommand SHALL exit with status one AND SHALL still invoke the auto-commit operation

#### Scenario: Fix --fix-max-iter overrides config

- **WHEN** `codebus fix --fix-max-iter 5` is invoked against a vault with `lint.fix.outer_ping_max: 2` in config
- **THEN** the fix loop SHALL use `outer_ping_max = 5` for that invocation
