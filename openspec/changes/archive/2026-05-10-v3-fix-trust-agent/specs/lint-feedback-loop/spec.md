# lint-feedback-loop Specification Delta — v3-fix-trust-agent

## REMOVED Requirements

### Requirement: Fix CLI Outer Ping Loop

**Reason**: v3-fix-trust-agent replaces the multi-spawn outer ping loop with a single-shot trust-agent model. Real e2e against claude v2.1.138 showed that CLI outer ping mechanism is redundant when CLI runs an authoritative final lint check after the agent terminates: the agent's in-session edits + Claude Code's own max-turns / context budget naturally cap runaway loops without the CLI managing pings. The new flow is captured in the `Fix Single-Shot Verification` requirement (ADDED below).

**Migration**: Code that previously called `run_fix_loop(vault_root, outer_ping_max)` SHALL be updated to call the new single-shot entry point that takes only `vault_root`. The `outer_ping_max` parameter is removed entirely; `--fix-max-iter` CLI flag is removed (see `Fix Loop Configuration` MODIFIED below).

#### Scenario: Outer ping mechanism replaced by Fix Single-Shot Verification

- **WHEN** `codebus fix` is invoked against a vault whose lint precheck reports issues, after this change is applied
- **THEN** the system SHALL spawn the agent CLI exactly one time AND SHALL NOT issue any `--resume` follow-up pings AND SHALL NOT honor any `outer_ping_max` config or `--fix-max-iter` flag — all replaced by the single-shot flow defined in `Fix Single-Shot Verification`

## ADDED Requirements

### Requirement: Fix Single-Shot Verification

The `codebus fix` subcommand and the lint-and-fix phase of the `codebus goal` subcommand SHALL execute exactly the following five steps in order:

1. Run lint via in-process call (lint precheck). If zero issues, exit success without spawning any agent.
2. Spawn the agentic CLI exactly once with `claude -p "/codebus-fix"`, the sandbox specified in the `Fix Loop Agent Sandbox` requirement, current working directory at the vault root, and stdin closed.
3. Wait for the agent process to terminate. The CLI SHALL NOT spawn any additional agent processes for this fix run.
4. Run lint via in-process call (final verification).
5. Use the final lint result as the authoritative state: zero issues → success exit code; one or more issues → failure exit code.

The CLI SHALL NOT pass `--session-id`, `--resume`, or `--continue` to the spawned agent. The agent operates in a one-shot session and is free to internally invoke `codebus lint` (subject to the Fix Bash Hook Restriction) any number of times within its session; the CLI's authority is limited to the final post-spawn lint check.

#### Scenario: Fix skips agent entirely when initial lint is clean

- **WHEN** `codebus fix` is invoked against a vault whose lint precheck reports zero issues
- **THEN** no agentic CLI process SHALL be spawned AND the subcommand SHALL exit with status zero AND no auto-commit SHALL be performed (working tree is clean)

#### Scenario: Fix spawns the agent exactly once on dirty vault

- **WHEN** `codebus fix` is invoked against a vault whose lint precheck reports one or more issues
- **THEN** the system SHALL spawn the agentic CLI exactly one time AND SHALL NOT spawn any subsequent agent processes for this run regardless of post-agent lint state

#### Scenario: Fix spawn arguments contain no session continuity flags

- **WHEN** the system spawns the agentic CLI for a fix run
- **THEN** the spawned command line SHALL NOT contain `--session-id`, `--resume`, or `--continue`

#### Scenario: Final lint determines exit code

- **WHEN** the agent process terminates and the post-agent lint reports zero issues
- **THEN** the `codebus fix` subcommand SHALL exit with status zero

#### Scenario: Issues remaining after agent yield non-zero exit

- **WHEN** the agent process terminates and the post-agent lint reports one or more issues
- **THEN** the `codebus fix` subcommand SHALL exit with non-zero status AND SHALL still invoke the auto-commit operation

---

### Requirement: Fix Bash Hook Installation

The `codebus init` subcommand SHALL write a `<vault_root>/.claude/settings.json` file containing a Claude Code `PreToolUse` hook configuration that intercepts every `Bash` tool invocation and routes it through the `codebus hook check-bash` subcommand. The settings file SHALL use the standard Claude Code settings schema with `hooks.PreToolUse` configured to match `Bash` and invoke `codebus hook check-bash` as a `command`-type hook.

The system SHALL apply write-if-missing semantics for this file: if `<vault_root>/.claude/settings.json` already exists, init SHALL NOT modify it (preserving any user-customized hook chain or other settings). The file SHALL NOT be written to `<repo>/.claude/settings.json` (source repository root) — the settings are vault-internal so the hook only applies to agent processes spawned with cwd at the vault root.

The `codebus hook check-bash` subcommand SHALL implement the following stdin/stdout contract:

- **Input** (stdin): a single JSON object matching Claude Code's PreToolUse hook input schema. The relevant fields are `tool_name` (expected `"Bash"`) and `tool_input.command` (the shell command string the agent intends to run).
- **Allow**: when the command's first argv token resolves to a `codebus` binary (file basename `codebus` or `codebus.exe`, case-insensitive match) AND the second argv token is exactly `lint`, the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON to stdout.
- **Block**: in all other cases (different binary, missing `lint` subcommand, malformed input, parse error), the subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` describes why the command was blocked.
- **Cross-platform**: the binary basename match SHALL be case-insensitive on Windows (`codebus.EXE` and `codebus.exe` both allowed) and case-sensitive on Unix.

The `<vault_root>/.gitignore` (vault internal) SHALL include the line `.claude/settings.local.json` so user-added local override settings are not committed to the vault git repository.

#### Scenario: Init writes settings.json on fresh vault

- **WHEN** `codebus init` runs against a repository with no existing `<vault_root>/.claude/settings.json`
- **THEN** the system SHALL create `<vault_root>/.claude/settings.json` AND the file content SHALL parse as JSON AND SHALL contain a `hooks.PreToolUse` array with a Bash matcher entry whose hook command invokes `codebus hook check-bash`

#### Scenario: Init does not overwrite existing settings.json

- **WHEN** `codebus init` runs against a vault where `<vault_root>/.claude/settings.json` already exists with custom content
- **THEN** the system SHALL NOT modify the existing file AND its byte-content SHALL be identical before and after init

#### Scenario: Init does not write settings.json to repo root

- **WHEN** `codebus init` runs against `<repo>`
- **THEN** the system SHALL NOT create or modify `<repo>/.claude/settings.json`

#### Scenario: hook check-bash allows bare codebus lint invocation

- **WHEN** `codebus hook check-bash` receives stdin JSON `{"tool_name":"Bash","tool_input":{"command":"codebus lint --format json"}}`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash allows codebus lint via absolute path

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` value is `/usr/local/bin/codebus lint --repo /path` OR (on Windows) `D:/dev/codebus.exe lint --format json`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash blocks non-codebus binaries

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `echo MARKER`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field is a non-empty string

#### Scenario: hook check-bash blocks codebus subcommands other than lint

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus fix --no-fix`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-bash fails closed on malformed input

- **WHEN** `codebus hook check-bash` receives stdin that does not parse as JSON
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (fail-closed default — never silently allow on parse failure)

#### Scenario: Vault internal gitignore excludes settings.local.json

- **WHEN** `codebus init` runs against `<repo>` and reaches the vault internal `.gitignore` mutation step
- **THEN** the file `<vault_root>/.gitignore` SHALL contain a line equal to `.claude/settings.local.json`

## MODIFIED Requirements

### Requirement: Fix SKILL.md Atomic Contract

The `codebus-fix` SKILL.md content authored by `codebus init` SHALL describe the fix workflow as a self-directed repair flow (the requirement keeps its v3-lint heading for spec stability, but its content is fully rewritten — the original "atomic contract" semantics are removed). The agent is invoked once, has access to `Read`, `Glob`, `Grep`, `Write`, `Edit`, and a restricted `Bash` (only `codebus lint *` permitted by the PreToolUse hook), and SHALL repair the wiki content. The agent SHALL freely invoke `codebus lint --format json` as needed to obtain or re-check the issue list within its session; the SKILL.md SHALL NOT prescribe a maximum iteration count, a single-round atomic contract, or a prohibition on internal lint loops.

The SKILL.md SHALL instruct the agent to use the absolute paths returned in the lint JSON `issues[].path` field directly with `Read`, `Write`, and `Edit` tools without path translation.

The SKILL.md SHALL acknowledge that loop control belongs to the agent itself within its session, and that the `codebus fix` CLI provides only a final post-session lint check as the authoritative success signal — there is no CLI-side iteration outside the single agent spawn.

#### Scenario: Fix SKILL.md does not prescribe a single-round atomic contract

- **WHEN** init writes the codebus-fix SKILL.md
- **THEN** the body SHALL NOT contain phrases that prescribe single-round-only semantics — specifically, it SHALL NOT contain the literal substrings `ONE round of repair`, `atomic contract`, or `MUST NOT spawn nested fix invocations or loop internally`

#### Scenario: Fix SKILL.md instructs absolute path usage

- **WHEN** init writes the codebus-fix SKILL.md
- **THEN** the body SHALL state that paths returned in the lint JSON `issues[].path` field are absolute and SHALL be used directly with Read, Write, and Edit tools

#### Scenario: Fix SKILL.md mentions CLI is the final-only verifier

- **WHEN** init writes the codebus-fix SKILL.md
- **THEN** the body SHALL state that the CLI runs lint after the agent session terminates and uses that result as the authoritative success signal, AND that the agent itself decides when its in-session repair work is complete

---

### Requirement: Fix Loop Configuration

The system SHALL load fix configuration from `~/.codebus/config.yaml` under the key path `lint.fix`. The schema SHALL define exactly one user-tunable field:

- `lint.fix.enabled` (boolean, default `true`) — when `false`, the lint-and-fix phase of `codebus goal` SHALL be skipped entirely; `codebus fix` standalone invocation SHALL exit with status zero and a stderr message indicating fix is disabled.

The CLI SHALL accept exactly one override flag: `--no-fix` (forces `enabled` to `false` for this invocation). The system SHALL NOT recognize a `--fix-max-iter` flag (removed in v3-fix-trust-agent); attempts to pass it SHALL fail at clap argument parsing.

The system SHALL silently ignore any `lint.fix.outer_ping_max` key found in the config file (forward-compat; legacy v3-lint config files remain readable). The system SHALL NOT use that value to influence runtime behavior.

#### Scenario: Default config enables fix

- **WHEN** `~/.codebus/config.yaml` is missing or has no `lint.fix` section
- **AND** the user runs `codebus goal "X"` with no override flags
- **THEN** the goal flow's fix phase SHALL run with `enabled = true`

#### Scenario: --no-fix flag disables fix even when config enables it

- **WHEN** `~/.codebus/config.yaml` has `lint.fix.enabled: true`
- **AND** the user runs `codebus goal "X" --no-fix`
- **THEN** the goal flow SHALL skip the fix phase entirely AND SHALL NOT spawn any fix agent

#### Scenario: Legacy outer_ping_max key is silently ignored

- **WHEN** `~/.codebus/config.yaml` contains a `lint.fix.outer_ping_max: 10` entry left over from v3-lint
- **AND** the user runs any verb
- **THEN** the system SHALL parse the config without error AND SHALL NOT report the unknown key AND the value SHALL have no observable effect on fix behavior

#### Scenario: --fix-max-iter is no longer a recognized flag

- **WHEN** the user runs `codebus fix --fix-max-iter 5`
- **THEN** clap argument parsing SHALL reject the unknown `--fix-max-iter` flag AND the binary SHALL exit with non-zero status

---

### Requirement: Standalone Fix Mode

The `codebus fix` subcommand SHALL be an entry point dedicated to running the single-shot fix flow against an existing vault, distinct from the goal-flow integration. Standalone fix mode SHALL NOT trigger source-signal drift detection, SHALL NOT re-sync the raw mirror, SHALL NOT update the vault manifest, AND SHALL NOT modify any source code outside the vault.

After the single agent spawn terminates and the final lint check completes, the standalone fix SHALL invoke the vault auto-commit operation with commit message `wiki: lint fix loop` against the nested vault git repository. If no file changes occurred under `<vault_root>/wiki/`, the commit SHALL be a no-op (working tree clean, no new commit recorded).

The standalone fix SHALL exit with status zero when the post-spawn lint reports zero issues, and with status one when one or more issues remain.

#### Scenario: Standalone fix skips ingest

- **WHEN** the user runs `codebus fix --repo <repo>`
- **THEN** the system SHALL NOT call any raw mirror sync, SHALL NOT update the manifest, AND SHALL NOT spawn any goal-style agent

#### Scenario: Standalone fix commits with lint fix loop message

- **WHEN** standalone fix runs and the agent's in-session work produces at least one change under `wiki/`
- **THEN** the system SHALL invoke vault auto-commit AND running `git -C <repo>/.codebus log --pretty=%s -1` SHALL print exactly the line `wiki: lint fix loop`

#### Scenario: Standalone fix no-op commit when agent makes no changes

- **WHEN** standalone fix runs and the agent terminates without modifying any file under `wiki/`
- **THEN** running `git -C <repo>/.codebus rev-list --count HEAD` after fix SHALL equal the count before fix

#### Scenario: Standalone fix exits non-zero when post-spawn lint has issues

- **WHEN** standalone fix completes its single agent spawn and the post-spawn lint still reports at least one issue
- **THEN** the subcommand SHALL exit with non-zero status AND SHALL still invoke the auto-commit operation

#### Scenario: Standalone fix requires existing vault

- **WHEN** the user runs `codebus fix --repo <repo>` against a path whose `.codebus/` directory does not exist
- **THEN** the system SHALL exit with status 2 AND stderr SHALL contain a message instructing the user to run `codebus init` first AND no agent process SHALL be spawned
