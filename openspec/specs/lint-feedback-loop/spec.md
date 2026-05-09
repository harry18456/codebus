# lint-feedback-loop Specification

## Purpose

TBD - created by archiving change 'v3-lint'. Update Purpose after archive.

## Requirements

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
### Requirement: Lint Read-Only Invariant

The lint subcommand SHALL NOT write, modify, or delete any file inside the vault directory tree. The vault `wiki/` and `raw/` subtrees SHALL be byte-identical before and after every lint invocation regardless of detected issues.

#### Scenario: Lint never modifies any vault file

- **WHEN** `codebus lint` runs against a vault containing files with lint errors
- **THEN** every file under `<vault-root>/wiki/` and `<vault-root>/raw/` SHALL be byte-identical before and after the invocation


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