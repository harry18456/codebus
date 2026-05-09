# lint-feedback-loop Specification

## Purpose

This capability defines the rules and mechanics for validating vault wiki content (lint) and automatically repairing detected issues via an agentic fix loop. It covers: lint rule set, vault location auto-detection, output formats, agent-driven self-loop pattern, CLI outer ping safety net, sandbox permissions, fix SKILL.md atomic contract, configuration schema, and standalone fix mode behavior.

## Requirements

## ADDED Requirements

### Requirement: Lint Rule Set

The system SHALL implement seven lint rules against the vault `wiki/` subtree, ported from the legacy `lint_wiki` reference implementation:

1. **Frontmatter parse failure** (severity: `error`) — knowledge page YAML frontmatter fails to parse.
2. **Cross-folder slug collision** (severity: `warn`) — two or more knowledge pages share the same filename across different type folders.
3. **Misplaced root page** (severity: `warn`) — a `.md` file lives at `wiki/` root and is not one of the special files (`index.md`, `log.md`).
4. **Frontmatter related[] format** (severity: `error`) — a `related[]` entry is not in the canonical `[[wikilink]]` format.
5. **Frontmatter related[] resolution** (severity: `error`) — a `related[]` slug does not match any existing knowledge page or special file.
6. **Body wikilink resolution** (severity: `warn`) — a `[[wikilink]]` in a knowledge page body does not resolve, after stripping fenced and inline code regions.
7. **Nav file presence and integrity** (severity: `warn`) — `index.md` or `log.md` missing, or its body contains broken `[[wikilinks]]`.

The lint SHALL scan all five type folders (`concepts/`, `entities/`, `modules/`, `processes/`, `synthesis/`) plus the two nav files (`index.md`, `log.md`) at `wiki/` root. The lint SHALL NOT scan `raw/`, `log/`, or any other vault subtree.

#### Scenario: All seven rules report against vault content

- **WHEN** the system runs lint against a vault with one trigger for each of the seven rules
- **THEN** the lint result SHALL contain at least seven issues, one per rule

#### Scenario: Lint scans only wiki subtree

- **WHEN** the system runs lint against a vault containing files under `raw/`, `log/`, and `wiki/`
- **THEN** issues SHALL only reference paths under `wiki/` AND no path under `raw/` or `log/` SHALL appear in the issues list

---

### Requirement: Vault Root Auto-Detection

The lint subsystem SHALL determine the vault root using the following precedence on each invocation:

1. If the `--repo <PATH>` flag is provided, the system SHALL use `<PATH>/.codebus/` as the vault root.
2. Otherwise, if the current working directory contains a `wiki/` subdirectory, the system SHALL treat the current working directory itself as the vault root.
3. Otherwise, if the current working directory contains a `.codebus/wiki/` path, the system SHALL treat the current working directory as the source repository root and `<cwd>/.codebus/` as the vault root.
4. Otherwise, the system SHALL exit with status 2 and emit a stderr hint instructing the user to run `codebus init` first.

The auto-detection SHALL apply only to the lint subcommand and the lint phase of the fix subcommand. The `init`, `goal`, and `query` subcommands SHALL retain their existing behavior of treating the input path as the source repository root.

#### Scenario: Lint detects vault when cwd is vault root itself

- **WHEN** `codebus lint` is invoked from a working directory containing a `wiki/` subdirectory
- **THEN** the lint SHALL operate on `<cwd>/wiki/` AND SHALL NOT look for `<cwd>/.codebus/`

#### Scenario: Lint detects vault when cwd is source repo root

- **WHEN** `codebus lint` is invoked from a working directory containing `.codebus/wiki/` but not a direct `wiki/` subdirectory
- **THEN** the lint SHALL operate on `<cwd>/.codebus/wiki/`

#### Scenario: Lint refuses when no vault is locatable

- **WHEN** `codebus lint` is invoked from a directory containing neither `wiki/` nor `.codebus/wiki/`, with no `--repo` flag
- **THEN** the system SHALL exit with status 2 AND stderr SHALL contain a message instructing the user to run `codebus init`

#### Scenario: Explicit --repo flag overrides cwd-based detection

- **WHEN** `codebus lint --repo /path/to/other-repo` is invoked
- **THEN** the system SHALL use `/path/to/other-repo/.codebus/` as the vault root regardless of the cwd contents

---

### Requirement: Lint Output Formats

The lint subcommand SHALL support two output formats selectable via `--format <text|json>` (default: `text`).

The `text` format SHALL print a human-readable report grouped by file path, with each issue path expressed as a vault-relative path (e.g., `concepts/auth.md`, `index.md`). The text format SHALL include a coverage summary line stating the number of pages scanned, the number of nav files scanned, the error count, and the warning count.

The `json` format SHALL print a single JSON object on stdout containing fields `vault_root` (absolute path string), `pages_scanned` (integer), `nav_files_scanned` (integer), `error_count` (integer), `warn_count` (integer), and `issues` (array). Each element of `issues` SHALL contain `path` (absolute filesystem path string, NOT vault-relative), `severity` (`"error"` or `"warn"`), `rule` (a stable kebab-case rule identifier from the rule set), and `message` (human-readable description).

The two formats SHALL never be mixed in the same invocation; selecting `--format json` SHALL suppress all human-readable text output to stdout.

#### Scenario: Text format emits vault-relative paths

- **WHEN** `codebus lint` runs without `--format json` against a vault with an issue in `wiki/concepts/auth.md`
- **THEN** stdout SHALL contain the substring `concepts/auth.md` AND SHALL NOT contain the substring of the absolute path leading up to that file

#### Scenario: JSON format emits absolute paths

- **WHEN** `codebus lint --format json` runs against a vault rooted at `<abs-vault>/wiki/` with an issue in `<abs-vault>/wiki/concepts/auth.md`
- **THEN** stdout SHALL contain a single JSON object whose `issues[].path` for that issue equals `<abs-vault>/wiki/concepts/auth.md` (absolute path)

#### Scenario: JSON format includes vault_root field

- **WHEN** `codebus lint --format json` runs against a vault rooted at `<abs-vault>/`
- **THEN** the JSON object SHALL contain a `vault_root` string field equal to the absolute path of `<abs-vault>/`

#### Scenario: JSON format suppresses human text

- **WHEN** `codebus lint --format json` runs against any vault
- **THEN** stdout SHALL parse as a single valid JSON value AND SHALL NOT contain emoji, ANSI color codes, or any non-JSON prefix or suffix

---

### Requirement: Lint Read-Only Invariant

The lint subcommand SHALL NOT write, modify, or delete any file inside the vault directory tree. The vault `wiki/` and `raw/` subtrees SHALL be byte-identical before and after every lint invocation regardless of detected issues.

#### Scenario: Lint never modifies any vault file

- **WHEN** `codebus lint` runs against a vault containing files with lint errors
- **THEN** every file under `<vault-root>/wiki/` and `<vault-root>/raw/` SHALL be byte-identical before and after the invocation

---

### Requirement: Fix Loop Agent Sandbox

The fix subsystem SHALL spawn the agentic CLI with the following sandbox flags:

- `--tools` value: `Read,Glob,Grep,Write,Edit,Bash(codebus lint *)`
- `--allowedTools` value: `Read,Glob,Grep,Write,Edit,Bash(codebus lint *)`
- `--permission-mode` value: `acceptEdits`

The `Bash(codebus lint *)` permission specifier SHALL grant the agent permission to invoke `codebus lint` with arbitrary arguments AND SHALL NOT grant permission to invoke any other binary. The agent SHALL NOT have permission to invoke `codebus init`, `codebus goal`, `codebus query`, `codebus fix`, or any non-codebus binary via the Bash tool.

The agent's working directory SHALL be set to the vault root (`<repo>/.codebus/`) and the agent's stdin SHALL be a closed stream.

#### Scenario: Fix spawn includes the codebus-lint bash whitelist

- **WHEN** the fix subsystem spawns the agentic CLI for any iteration of the loop
- **THEN** the spawned command line SHALL include the literal substring `Bash(codebus lint *)` in both the `--tools` and `--allowedTools` flag values

#### Scenario: Fix spawn permits Read, Glob, Grep, Write, Edit alongside the bash whitelist

- **WHEN** the fix subsystem spawns the agentic CLI
- **THEN** the spawned command line SHALL include `Read`, `Glob`, `Grep`, `Write`, and `Edit` in both the `--tools` and `--allowedTools` flag values

#### Scenario: Fix spawn cwd is vault root with stdin closed

- **WHEN** the fix subsystem spawns the agentic CLI
- **THEN** the child process SHALL be invoked with current working directory equal to `<repo>/.codebus/` AND its stdin SHALL be a closed stream

---

### Requirement: Fix SKILL.md Atomic Contract

The `codebus-fix` SKILL.md content authored by `codebus init` SHALL define the agent's task as atomic: receive a set of lint issues (either from invoking `codebus lint --format json` or from prompt context), repair the corresponding `wiki/` files, and exit. The SKILL.md SHALL NOT prescribe a multi-iteration loop; loop control SHALL belong to the caller (the CLI fix subcommand or the user invoking the skill manually).

The SKILL.md SHALL instruct the agent to invoke `codebus lint --format json` to retrieve issues when no issues are present in the prompt context, AND to use the absolute paths in the JSON `issues[].path` field directly without path translation.

#### Scenario: Fix SKILL.md does not contain loop control language

- **WHEN** init writes the codebus-fix SKILL.md
- **THEN** the body SHALL NOT contain the words `iteration`, `iterate`, `loop`, `retry`, or `again` in any context that prescribes the agent itself to repeat its own work

#### Scenario: Fix SKILL.md instructs absolute path usage

- **WHEN** init writes the codebus-fix SKILL.md
- **THEN** the body SHALL state that paths returned in the lint JSON `issues[].path` field are absolute and SHALL be used directly with Read, Write, and Edit tools

---

### Requirement: Fix CLI Outer Ping Loop

The `codebus fix` subcommand and the lint-and-fix phase of the `codebus goal` subcommand SHALL implement a CLI-controlled outer loop with the following structure:

1. Run lint via in-process call. If zero issues, exit success without spawning any agent.
2. Generate a UUID for this loop session.
3. Spawn the agentic CLI with `claude -p "/codebus-fix" --session-id <uuid>` and the sandbox specified in the Fix Loop Agent Sandbox requirement.
4. After the agent process terminates, run lint again.
5. If the post-lint reports zero issues, terminate successfully.
6. Otherwise, if the iteration count is below `outer_ping_max`, spawn the agentic CLI with `claude -p "<follow-up-prompt-with-remaining-issues>" --resume <uuid>` and goto step 4.
7. If `outer_ping_max` is reached and issues still remain, terminate with non-zero status.

The CLI SHALL NOT use the `--continue` flag; session continuity SHALL be achieved exclusively via `--session-id` on first spawn and `--resume <uuid>` on subsequent spawns. The follow-up prompt SHALL include the current list of remaining lint issues serialized into the prompt body.

#### Scenario: Fix loop skips agent entirely when initial lint is clean

- **WHEN** `codebus fix` is invoked against a vault whose lint reports zero issues
- **THEN** no agentic CLI process SHALL be spawned AND the subcommand SHALL exit with status zero

#### Scenario: Fix loop spawns agent with --session-id on first iteration

- **WHEN** the fix loop reaches its first agent spawn
- **THEN** the spawned command line SHALL include the flag pair `--session-id <uuid>` for some valid UUID AND SHALL NOT include `--resume`

#### Scenario: Fix loop uses --resume for outer pings

- **WHEN** the fix loop completes its first agent invocation, runs lint, finds remaining issues, and the iteration counter is below `outer_ping_max`
- **THEN** the next spawned command line SHALL include the flag pair `--resume <uuid>` matching the UUID from the first spawn AND SHALL NOT include `--session-id`

#### Scenario: Fix loop terminates on clean post-lint

- **WHEN** the fix loop runs an agent iteration and the subsequent lint reports zero issues
- **THEN** the loop SHALL terminate without spawning further agent iterations

#### Scenario: Fix loop respects outer_ping_max cap

- **WHEN** the fix loop has performed `outer_ping_max + 1` total agent invocations (one initial plus `outer_ping_max` pings) and lint still reports issues
- **THEN** the loop SHALL terminate without spawning further agents AND the `codebus fix` subcommand SHALL exit with non-zero status

---

### Requirement: Fix Loop Configuration

The system SHALL load fix loop configuration from `~/.codebus/config.yaml` under the key path `lint.fix`. The schema SHALL define two fields:

- `lint.fix.enabled` (boolean, default `true`) — when `false`, the lint-and-fix phase of `codebus goal` SHALL be skipped entirely; `codebus fix` standalone invocation SHALL exit with status zero and a stderr message indicating fix is disabled.
- `lint.fix.outer_ping_max` (positive integer, default `2`) — caps the number of CLI outer ping iterations performed after the initial agent invocation.

The CLI SHALL accept two override flags: `--no-fix` (forces `enabled` to `false` for this invocation) and `--fix-max-iter <N>` (overrides `outer_ping_max` to `N` for this invocation, where `N` SHALL be a positive integer). When both flags are present, `--no-fix` SHALL take precedence and `--fix-max-iter` SHALL have no observable effect.

#### Scenario: Default config enables fix with outer_ping_max two

- **WHEN** `~/.codebus/config.yaml` is missing or has no `lint.fix` section
- **AND** the user runs `codebus goal "X"` with no override flags
- **THEN** the goal flow's fix phase SHALL run with `enabled = true` and `outer_ping_max = 2`

#### Scenario: --no-fix flag disables fix even when config enables it

- **WHEN** `~/.codebus/config.yaml` has `lint.fix.enabled: true`
- **AND** the user runs `codebus goal "X" --no-fix`
- **THEN** the goal flow SHALL skip the fix phase entirely AND SHALL NOT spawn any fix agent

#### Scenario: --fix-max-iter overrides config outer_ping_max

- **WHEN** `~/.codebus/config.yaml` has `lint.fix.outer_ping_max: 2`
- **AND** the user runs `codebus goal "X" --fix-max-iter 5`
- **THEN** the fix loop SHALL use `outer_ping_max = 5` for that invocation

#### Scenario: --no-fix wins when both override flags are present

- **WHEN** the user runs `codebus goal "X" --no-fix --fix-max-iter 5`
- **THEN** the fix phase SHALL be skipped AND `--fix-max-iter` SHALL have no observable effect

---

### Requirement: Standalone Fix Mode

The `codebus fix` subcommand SHALL be an entry point dedicated to running the fix loop against an existing vault, distinct from the goal-flow integration. Standalone fix mode SHALL NOT trigger source-signal drift detection, SHALL NOT re-sync the raw mirror, SHALL NOT update the vault manifest, AND SHALL NOT modify any source code outside the vault.

After the fix loop terminates (whether successfully or by exhausting the ping budget), the standalone fix SHALL invoke the vault auto-commit operation with commit message `wiki: lint fix loop` against the nested vault git repository. If the loop produced no file changes, the commit SHALL be a no-op (working tree clean, no new commit).

The standalone fix SHALL exit with status zero when the final post-loop lint reports zero issues, and with status one when issues remain after exhausting the ping budget.

#### Scenario: Standalone fix skips ingest

- **WHEN** the user runs `codebus fix --repo <repo>`
- **THEN** the system SHALL NOT call any raw mirror sync, SHALL NOT update the manifest, AND SHALL NOT spawn any goal-style agent

#### Scenario: Standalone fix commits with lint fix loop message

- **WHEN** standalone fix runs and the loop produces at least one change under `wiki/`
- **THEN** the system SHALL invoke vault auto-commit AND running `git -C <repo>/.codebus log --pretty=%s -1` SHALL print exactly the line `wiki: lint fix loop`

#### Scenario: Standalone fix no-op commit when loop makes no changes

- **WHEN** standalone fix runs and the loop terminates without any file modification under `wiki/`
- **THEN** running `git -C <repo>/.codebus rev-list --count HEAD` after fix SHALL equal the count before fix

#### Scenario: Standalone fix exits non-zero when issues remain

- **WHEN** standalone fix exhausts `outer_ping_max + 1` total agent invocations and the final lint still reports at least one issue
- **THEN** the subcommand SHALL exit with non-zero status AND SHALL still invoke the auto-commit operation

#### Scenario: Standalone fix requires existing vault

- **WHEN** the user runs `codebus fix --repo <repo>` against a path whose `.codebus/` directory does not exist
- **THEN** the system SHALL exit with status 2 AND stderr SHALL contain a message instructing the user to run `codebus init` first AND no agent process SHALL be spawned
