# cli Specification

## Purpose

The command-line entry surface — the `codebus` binary's subcommand registry, global flags, and per-verb invocation contracts. Five subcommands: `init` (vault bootstrap), `goal` (ingest with auto-fix), `query` (read-only Q&A), `lint` (vault validation), `fix` (auto-repair). Each verb's behavior — sandbox flags passed to the spawned agent, exit code policy, auto-commit timing — is normatively specified here. Cross-cuts: spawn-pattern internals (sandbox composition, hook installation) live in `lint-feedback-loop`; vault structure init writes lives in `vault`; skill bundle materialization lives in `skill-bundles`.

## Requirements

### Requirement: Subcommand Registration

The `codebus` binary SHALL register exactly five subcommands at the top level: `init`, `goal`, `query`, `lint`, `fix`. No other subcommand SHALL be registered. The `--help` and `--version` flags SHALL be available at both the binary level and per subcommand.

#### Scenario: Help output lists exactly the five subcommands

- **WHEN** `codebus --help` is invoked
- **THEN** the help output SHALL list `init`, `goal`, `query`, `lint`, `fix` as the only subcommands AND SHALL exit with status zero

#### Scenario: Version flag prints cargo package version

- **WHEN** `codebus --version` is invoked
- **THEN** the binary SHALL print a single line containing the cargo package version of the `codebus-cli` crate AND SHALL exit with status zero

#### Scenario: Unknown subcommand is rejected by clap

- **WHEN** `codebus mcp` or `codebus randomverb` is invoked
- **THEN** the binary SHALL print a clap error message to stderr identifying the unknown subcommand AND SHALL exit with non-zero status


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

The `init` subcommand SHALL accept the flags `--repo <PATH>` (default: current working directory) and `--no-obsidian-register` (boolean flag). When invoked, `init` SHALL orchestrate (in order): pre-flight sanity check, vault layout creation, raw mirror, source repo `.gitignore` mutation, per-repo schema file write, manifest write, skill bundle authoring, Obsidian vault registration, and global config starter write. The global config starter write step SHALL invoke the `write_starter_config_if_missing` primitive against `~/.codebus/config.yaml`; when the file is absent the system SHALL create the parent directory if necessary and write the starter content; when the file already exists the system SHALL NOT read or overwrite it. In default output mode `init` SHALL emit a sequence of banners (defined by the `Banner Output for Verb Commands` requirement) covering at minimum the `Start`, `SyncDone`, `PiiSummary`, `CommitDone`, and `Done` banner variants. Default mode SHALL NOT emit per-step `✓ <internal-detail>` progress lines (these are reserved for `--debug` mode). When `--debug` is passed, `init` SHALL emit the same banner sequence AND additionally emit the per-step `✓ <internal-detail>` progress lines (vault layout, source `.gitignore` mutation, schema file write, manifest write, skill bundle authoring, vault settings write, global config starter, and any others implementation chooses) AND the `[debug]` lines describing internal decisions, fs operations, computed source signal values, and target paths. `init` SHALL exit with status zero on success and non-zero only if a sanity-check refusal or unrecoverable filesystem error occurs. A failure of the global config starter write step SHALL emit a stderr warning prefixed with `warning: global config` AND SHALL NOT cause `init` to exit non-zero (the rest of init having succeeded means the per-vault state is usable).

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
### Requirement: Debug Flag Output

The `codebus` binary SHALL accept `--debug` as a global flag, available at the top-level command and inheritable by every subcommand (e.g., `codebus --debug init`, `codebus init --debug` SHALL behave equivalently). When `--debug` is set, the binary's verb handlers SHALL emit (in addition to the default-mode banner sequence) the per-step `✓ <internal-detail>` progress lines describing intermediate orchestration outcomes AND the `[debug]` lines describing internal decisions, fs operations, computed values, and target paths. When `--debug` is NOT set, the binary SHALL NOT emit any line beginning with `[debug]` AND SHALL NOT emit per-step `✓ <internal-detail>` progress lines (only the higher-level banner sequence emerges in default mode).

#### Scenario: Default mode suppresses [debug] lines

- **WHEN** `codebus init` runs without `--debug`
- **THEN** neither stdout nor stderr SHALL contain any line beginning with `[debug]`

#### Scenario: Debug mode emits both detail and trace lines

- **WHEN** `codebus init --debug` runs against any repository
- **THEN** stdout SHALL contain at least one per-step `✓ <internal-detail>` progress line AND at least one `[debug]` trace line


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
### Requirement: Goal Subcommand Behavior

The `goal` subcommand SHALL accept one positional argument `<goal-text>` (the user's natural-language goal description) and the following flags: `--repo <PATH>` (default: current working directory), `--force-resync` (boolean, force raw mirror re-sync even when source-signal detection reports no drift), `--no-obsidian-register` (boolean, forwarded to the auto-init fallback), `--no-fix` (boolean, skip the post-agent lint-and-fix phase entirely), and the inherited `--debug` global flag. When invoked, the subcommand SHALL execute exactly six steps in order: (1) resolve the target repository path, (2) when `<repo>/.codebus/` does not exist, run the `init` flow against the same repo (forwarding `--no-obsidian-register`); (3) perform source-signal drift detection and conditionally re-sync the raw mirror plus update the manifest; (4) spawn the goal agent CLI with the canonical sandbox flags described below and stream child stdout and stderr to the parent process; (5) after the goal agent terminates, run the lint-and-fix phase against the vault unless `--no-fix` is present or `lint.fix.enabled` is `false` in config; (6) after the lint-and-fix phase terminates (or is skipped), invoke the vault auto-commit operation with the message `wiki: <goal-text>` against the nested vault repository. The subcommand SHALL exit with the goal agent's exit code when the goal agent exited non-zero, otherwise with the fix phase's exit code, unless the auto-commit operation itself fails, in which case it SHALL exit with a non-zero code identifying the auto-commit failure.

The goal agent spawn SHALL pass the following arguments to the agent CLI: `-p` followed by the literal string `/codebus-goal "<goal-text>"`; `--tools` followed by the comma-separated string `Read,Glob,Grep,Write,Edit`; `--allowedTools` followed by the same comma-separated string; `--permission-mode` followed by `acceptEdits`. The goal agent's child process SHALL be invoked with the current working directory set to `<repo>/.codebus/` and its stdin SHALL be a closed stream (preventing the agent from blocking on input).

The lint-and-fix phase SHALL invoke the single-shot fix flow defined by the `Fix Single-Shot Verification` requirement of the `lint-feedback-loop` capability, using the same vault root as the goal agent. The phase SHALL be skipped when either `--no-fix` is present on the goal invocation or `lint.fix.enabled` is `false` in `~/.codebus/config.yaml`. The system SHALL NOT recognize a `--fix-max-iter` flag (removed in v3-fix-trust-agent) — attempts to pass it SHALL fail at clap argument parsing.

The auto-commit operation in step 6 SHALL use a single commit message `wiki: <goal-text>` covering both the goal agent's writes and any fix-phase edits, regardless of whether the lint-and-fix phase ran or how it terminated.

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
- **THEN** the lint-and-fix phase SHALL run against the vault before any auto-commit AND any file modifications produced by the fix phase SHALL be included in the same commit as the goal agent's writes

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
- **THEN** the goal subcommand SHALL exit with status N regardless of whether the fix phase subsequently succeeds, unless the auto-commit operation itself fails

#### Scenario: --fix-max-iter is no longer a recognized goal flag

- **WHEN** the user runs `codebus goal "X" --fix-max-iter 5`
- **THEN** clap argument parsing SHALL reject the unknown `--fix-max-iter` flag AND the binary SHALL exit with non-zero status


<!-- @trace
source: v3-fix-trust-agent
updated: 2026-05-10
code:
  - codebus-cli/src/commands/query.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-cli/src/commands/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/Cargo.toml
  - codebus-cli/src/commands/hook.rs
  - codebus-core/src/vault/mod.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-core/src/wiki/fix/prompt.rs
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/wiki/fix/session.rs
  - codebus-core/src/vault/settings.rs
tests:
  - codebus-core/tests/vault_init.rs
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: Query Subcommand Behavior

The `query` subcommand SHALL accept one positional argument `<query-text>` (the user's natural-language question) and the inherited `--repo <PATH>` (default: current working directory) and `--debug` global flags. When invoked, the subcommand SHALL execute exactly four steps in order: (1) resolve the target repository path, (2) check the vault precondition — if `<repo>/.codebus/` does not exist, exit with status 2 and emit a stderr message instructing the user to run `codebus init` first; do NOT auto-run init; (3) spawn the agent CLI with the read-only sandbox flags described below and stream child stdout and stderr to the parent process; (4) after the child terminates, exit with the child process's exit code without performing any vault auto-commit operation.

The agent spawn SHALL pass the following arguments to the agent CLI: `-p` followed by the literal string `/codebus-query "<query-text>"`; `--tools` followed by the comma-separated string `Read,Glob,Grep`; `--allowedTools` followed by the same comma-separated string; `--permission-mode` followed by `acceptEdits`. The agent's child process SHALL be invoked with the current working directory set to `<repo>/.codebus/` and its stdin SHALL be a closed stream.

The query subcommand SHALL NOT trigger raw-mirror re-sync, SHALL NOT update the vault manifest, SHALL NOT modify any file inside the vault, and SHALL NOT create any nested-git commit.

#### Scenario: Query refuses when vault is missing

- **WHEN** `codebus query "what does Foo do"` is invoked against a repository whose `.codebus/` directory does not exist
- **THEN** the subcommand SHALL exit with status 2 AND stderr SHALL contain a message instructing the user to run `codebus init` first AND no agent process SHALL be spawned

#### Scenario: Query spawns agent with cwd at vault root

- **WHEN** the query subcommand reaches its agent-spawn step against a vault rooted at `<repo>/.codebus/`
- **THEN** the agent child process SHALL be spawned with current working directory equal to `<repo>/.codebus/` AND the child SHALL receive the prompt argument `/codebus-query "<query-text>"` AND the child's stdin SHALL be closed

#### Scenario: Query passes the read-only triple-flag sandbox

- **WHEN** the query subcommand spawns the agent
- **THEN** the spawned command line SHALL include the flag pair `--tools Read,Glob,Grep`, the flag pair `--allowedTools Read,Glob,Grep`, and the flag pair `--permission-mode acceptEdits`
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

<!-- @trace
source: v3-query
updated: 2026-05-09
code:
  - codebus-cli/src/commands/query.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-cli/tests/query_flow.rs
-->

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

The `fix` subcommand SHALL accept the following flags: `--repo <PATH>` (default: current working directory), `--no-fix` (boolean, when present the subcommand SHALL exit with status zero and a stderr message stating fix is disabled, without spawning any agent or running lint), and the inherited `--debug` global flag. When invoked without `--no-fix`, the subcommand SHALL execute exactly six steps in order: (1) resolve the target repository path and verify `<repo>/.codebus/` exists (otherwise exit with status 2 and a stderr hint instructing the user to run `codebus init`), (2) run lint pre-check; if zero issues, exit with status zero and skip remaining steps, (3) spawn the fix agent CLI exactly once per the `Fix Single-Shot Verification` requirement of the `lint-feedback-loop` capability, (4) wait for the agent process to terminate (no further agent spawns), (5) run lint final check, (6) invoke vault auto-commit with message `wiki: lint fix loop` and exit with status reflecting the final lint state.

The fix subcommand SHALL NOT trigger source-signal drift detection, SHALL NOT call raw mirror sync, SHALL NOT update the vault manifest, SHALL NOT modify any source code outside the vault, and SHALL NOT spawn a goal-style agent. The only agent process the fix subcommand spawns SHALL be one fix agent per invocation defined by the `Fix Loop Agent Sandbox` requirement of the `lint-feedback-loop` capability.

The system SHALL NOT recognize a `--fix-max-iter` flag (removed in v3-fix-trust-agent) — attempts to pass it SHALL fail at clap argument parsing.

The subcommand SHALL exit with status zero when the post-spawn lint reports zero issues. The subcommand SHALL exit with status one when one or more issues remain after the agent process terminates. The subcommand SHALL exit with status two when the vault precondition fails.

#### Scenario: Fix refuses when vault is missing

- **WHEN** `codebus fix --repo <repo>` is invoked against a path whose `.codebus/` directory does not exist
- **THEN** the subcommand SHALL exit with status 2 AND stderr SHALL contain a message instructing the user to run `codebus init` first AND no agent process SHALL be spawned

#### Scenario: Fix exits zero on clean vault

- **WHEN** `codebus fix` runs against a vault whose initial lint reports zero issues
- **THEN** no agent process SHALL be spawned AND the subcommand SHALL exit with status zero AND no new git commit SHALL be created

#### Scenario: Fix --no-fix flag disables fix entirely

- **WHEN** `codebus fix --no-fix` is invoked against any vault
- **THEN** the subcommand SHALL exit with status zero AND no agent process SHALL be spawned AND no lint SHALL be performed AND stderr SHALL contain a message stating fix is disabled

#### Scenario: Fix spawns the agent exactly once

- **WHEN** `codebus fix` runs against a vault whose lint precheck reports issues
- **THEN** the subcommand SHALL spawn the agent CLI exactly one time for the entire fix run AND SHALL NOT spawn additional agent processes regardless of post-agent lint state

#### Scenario: Fix auto-commits with lint fix loop message

- **WHEN** `codebus fix` runs and the agent's in-session work produces at least one change under `wiki/`
- **THEN** the subcommand SHALL invoke vault auto-commit AND `git -C <repo>/.codebus log --pretty=%s -1` SHALL print exactly the line `wiki: lint fix loop`

#### Scenario: Fix exits one when post-spawn lint has issues

- **WHEN** `codebus fix` completes its single agent spawn and the post-spawn lint reports at least one issue
- **THEN** the subcommand SHALL exit with status one AND SHALL still invoke the auto-commit operation

#### Scenario: --fix-max-iter is no longer a recognized fix flag

- **WHEN** the user runs `codebus fix --fix-max-iter 5`
- **THEN** clap argument parsing SHALL reject the unknown `--fix-max-iter` flag AND the binary SHALL exit with non-zero status

<!-- @trace
source: v3-fix-trust-agent
updated: 2026-05-10
code:
  - codebus-cli/src/commands/query.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-cli/src/commands/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/Cargo.toml
  - codebus-cli/src/commands/hook.rs
  - codebus-core/src/vault/mod.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-core/src/wiki/fix/prompt.rs
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/wiki/fix/session.rs
  - codebus-core/src/vault/settings.rs
tests:
  - codebus-core/tests/vault_init.rs
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: Claude Code Configuration Schema

The system SHALL load Claude Code agent configuration from `~/.codebus/config.yaml` under the top-level key `claude_code`. The schema SHALL define exactly three optional subsections (`goal`, `query`, `fix`), each with two optional fields (`model`, `effort`). The default values SHALL be: `goal: { model: opus, effort: high }`, `query: { model: haiku, effort: low }`, `fix: { model: sonnet, effort: medium }`. When the file is missing, the `claude_code` section is absent, a verb subsection is absent, or any individual field is absent, the system SHALL apply the corresponding default value. The system SHALL NOT validate the value of `model` or `effort` against any enumeration — strings SHALL be passed through to the agent CLI verbatim, allowing future model and effort identifiers without codebus-side schema changes. Unknown keys inside the `claude_code` section or its subsections SHALL be silently ignored to preserve forward-compatibility.

#### Scenario: Default config when file missing

- **WHEN** `~/.codebus/config.yaml` does not exist
- **THEN** the loaded `ClaudeCodeConfig` SHALL equal `{ goal: { model: opus, effort: high }, query: { model: haiku, effort: low }, fix: { model: sonnet, effort: medium } }`

#### Scenario: Per-verb override applies only to that verb

- **WHEN** `~/.codebus/config.yaml` contains `claude_code:\n  goal:\n    model: sonnet\n` and no other `claude_code.*` keys
- **THEN** the loaded `ClaudeCodeConfig.goal.model` SHALL equal `sonnet` AND `ClaudeCodeConfig.goal.effort` SHALL equal `high` (the default) AND `ClaudeCodeConfig.query` SHALL equal `{ model: haiku, effort: low }` (defaults preserved)

#### Scenario: Arbitrary model string is accepted

- **WHEN** `~/.codebus/config.yaml` contains `claude_code:\n  goal:\n    model: claude-opus-4-7\n`
- **THEN** the loader SHALL succeed AND the loaded `ClaudeCodeConfig.goal.model` SHALL equal the literal string `claude-opus-4-7` AND no validation against an allowed-models list SHALL occur

#### Scenario: Unknown subkey silently ignored

- **WHEN** `~/.codebus/config.yaml` contains `claude_code:\n  goal:\n    model: opus\n    future_field: hello\n`
- **THEN** the loader SHALL succeed AND the loaded `ClaudeCodeConfig.goal.model` SHALL equal `opus` AND the unknown `future_field` SHALL have no observable effect


<!-- @trace
source: v3-config
updated: 2026-05-10
code:
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-cli/src/commands/query.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/config/mod.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/config/pii.rs
tests:
  - codebus-cli/tests/query_flow.rs
  - codebus-cli/tests/cli_routing.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/lint_flow.rs
  - codebus-core/tests/vault_init.rs
-->

---
### Requirement: Agent Spawn Model and Effort Forwarding

When the `goal`, `query`, or `fix` subcommand spawns its agent CLI child process, the system SHALL append `--model <X>` and `--effort <Y>` to the spawned command line, where `<X>` and `<Y>` are the values from the corresponding `claude_code.<verb>.model` and `claude_code.<verb>.effort` config fields after default fill-in. The forwarded flags SHALL be appended in addition to the existing `--tools`, `--allowedTools`, and `--permission-mode` flags defined by the per-verb subcommand requirements; their presence SHALL NOT remove or alter any other flag. When a config field's effective value is `None` (not currently reachable through the documented schema, but possible through a future deletion-style override), the system SHALL omit the corresponding flag entirely (rather than passing an empty string).

#### Scenario: Goal subcommand forwards configured model and effort

- **WHEN** the goal subcommand spawns the agent against a config containing `claude_code.goal: { model: opus, effort: high }`
- **THEN** the spawned command line SHALL include the flag pair `--model opus` AND the flag pair `--effort high`

#### Scenario: Query subcommand forwards configured model and effort

- **WHEN** the query subcommand spawns the agent against a config containing `claude_code.query: { model: haiku, effort: low }`
- **THEN** the spawned command line SHALL include the flag pair `--model haiku` AND the flag pair `--effort low`

#### Scenario: Fix subcommand forwards configured model and effort

- **WHEN** the fix subcommand spawns its agent against a config containing `claude_code.fix: { model: sonnet, effort: medium }`
- **THEN** the spawned command line SHALL include the flag pair `--model sonnet` AND the flag pair `--effort medium`

#### Scenario: User-provided non-default values flow through

- **WHEN** `~/.codebus/config.yaml` overrides `claude_code.goal.model` to `claude-opus-4-7` and the goal subcommand spawns
- **THEN** the spawned command line SHALL include the flag pair `--model claude-opus-4-7`

#### Scenario: Per-verb defaults differ across verbs

- **WHEN** `~/.codebus/config.yaml` is absent and the user runs `codebus goal "X"` then `codebus query "Y"` then `codebus fix`
- **THEN** the goal spawn SHALL include `--model opus --effort high`, the query spawn SHALL include `--model haiku --effort low`, and the fix spawn SHALL include `--model sonnet --effort medium`

<!-- @trace
source: v3-config
updated: 2026-05-10
code:
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-cli/src/commands/query.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/config/mod.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/config/pii.rs
tests:
  - codebus-cli/tests/query_flow.rs
  - codebus-cli/tests/cli_routing.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/lint_flow.rs
  - codebus-core/tests/vault_init.rs
-->

---
### Requirement: Banner Output for Verb Commands

The system SHALL render run-lifecycle messages as a sequence of structured banners with codebus brand identity (the bus / boarding metaphor). The banner set SHALL contain at minimum these ten variants, each carrying a fixed set of payload fields: `Start { repo_path }`, `Goal { goal_text }`, `SyncStart`, `SyncDone { files, mib, elapsed_ms }`, `PiiSummary { scanner, scanned, hits, action }`, `LintStart`, `LintDone { errors, warns, elapsed_ms }`, `CommitDone { sha7 }`, `Done { wiki_path }`, `Hint { wiki_path }`. Each banner SHALL render to a single stdout line. The system SHALL provide both an emoji-leading form (e.g., `🚌 來囉來囉~ CodeBus 駛入 <path>...` for `Start`) and a symbol-leading fallback form (e.g., `▶ 來囉來囉~ CodeBus 駛入 <path>...`); selection between forms SHALL be governed by the `Environment-Aware Output Styling` requirement. The verb command modules (`init`, `goal`, `query`, `fix`, `lint`) SHALL invoke the appropriate banner sequence at lifecycle transitions in their own stdout (NOT in the spawned agent's stdout — agent output remains a passthrough).

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

- **GIVEN** a fresh git repo with one tracked file, Obsidian installed and registered
- **WHEN** `codebus init` runs in default mode with emoji enabled
- **THEN** stdout banner sequence SHALL be: `🚌 來囉來囉~ CodeBus 駛入 ./...`, `✓ 同步完成 (1 檔, 0.0 MiB, <ms> ms)`, `🛡 PII：regex_basic, scanned 1, hits 0, action mask`, `📌 commit <sha7>`, `🎉 掰掰~下車囉！wiki 已生成於 ./.codebus/wiki`, `💡 請用 Obsidian 開 ./.codebus/wiki`

#### Scenario: Symbol fallback used when emoji disabled

- **WHEN** `codebus init` runs in an environment where emoji are disabled (non-TTY OR `NO_COLOR=1`)
- **THEN** the `Start` banner SHALL begin with `▶` (not `🚌`) AND the `Done` banner SHALL begin with `✓` (not `🎉`) AND the `LintStart` banner SHALL begin with `~` (not `🔍`)


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