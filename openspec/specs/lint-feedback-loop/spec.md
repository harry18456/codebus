# lint-feedback-loop Specification

## Purpose

The wiki-content validation rule set, lint output formats, fix-loop spawn pattern, and Bash sandbox hook installation — 7 deterministic lint rules over `<vault>/wiki/` (frontmatter integrity, slug uniqueness, body wikilink resolution, nav file integrity, related-format, root-page placement), text and JSON output formats with vault-relative / absolute path conventions, single-shot fix flow that spawns the codebus-fix agent at most once per CLI invocation with CLI as final-only verifier, the `lint.fix.*` configuration schema, and the PreToolUse hook installed via `<vault>/.claude/settings.json` that hard-gates the agent's Bash tool to `codebus lint *` only. Does NOT cover the SKILL.md content the agent loads (lives in `skill-bundles` per-verb workflow content), nor source-repo or vault structural concerns (live in `vault`).

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

1. If the `--repo <PATH>` flag is provided, the system SHALL inspect `<PATH>` to disambiguate the user's intent: when `<PATH>/wiki/` is a directory the system SHALL treat `<PATH>` itself as the vault root (the user passed the `.codebus/` directory directly); otherwise the system SHALL treat `<PATH>` as the source repository root and use `<PATH>/.codebus/` as the vault root.
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

#### Scenario: Explicit --repo flag pointing at source repo uses joined path

- **WHEN** `codebus lint --repo /path/to/source-repo` is invoked, where `/path/to/source-repo` does NOT contain a `wiki/` subdirectory but its `.codebus/wiki/` path exists
- **THEN** the system SHALL use `/path/to/source-repo/.codebus/` as the vault root

#### Scenario: Explicit --repo flag pointing at vault root uses path directly

- **WHEN** `codebus lint --repo /path/to/source-repo/.codebus` is invoked, where the path itself contains a `wiki/` subdirectory
- **THEN** the system SHALL use `/path/to/source-repo/.codebus/` as the vault root (no second `.codebus` join) AND lint SHALL produce identical output to invoking `codebus lint --repo /path/to/source-repo`

##### Example: same vault, two --repo argument forms

- **GIVEN** an initialized vault at `/repo/.codebus/wiki/` with one broken wikilink in `concepts/foo.md`
- **WHEN** the user runs `codebus lint --repo /repo` and separately `codebus lint --repo /repo/.codebus`
- **THEN** both invocations SHALL emit the same stdout report containing the broken-wikilink warning for `wiki/concepts/foo.md`

#### Scenario: Explicit --repo flag with non-existent path falls back to joined form

- **WHEN** `codebus lint --repo /nonexistent/path` is invoked, where neither `/nonexistent/path/wiki/` nor `/nonexistent/path/.codebus/wiki/` exists
- **THEN** the system SHALL use `/nonexistent/path/.codebus/` as the vault root (preserving the prior contract that `--repo` does not validate existence — the lint phase that follows will surface the absence of any wiki content)


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
### Requirement: Lint Output Formats

The lint subcommand SHALL support two output formats selectable via `--format <text|json>` (default: `text`).

The `text` format SHALL print a human-readable report grouped by file path, with each issue path expressed as a vault-relative path (e.g., `concepts/auth.md`, `index.md`). The text format SHALL include a coverage summary line stating the number of pages scanned, the number of nav files scanned, the error count, and the warning count. The text format SHALL be styled per the `Environment-Aware Output Styling` requirement of the `cli` capability: when `use_emoji` is true, the clean-result header SHALL begin with `✅`, the issue-result header SHALL begin with `🔍`, the per-file path lead SHALL be `✗ ` for files containing errors and `⚠ ` for files containing warnings only; when `use_emoji` is false, those leads SHALL be `ok`, `#`, `x ` and `! ` respectively (the legacy ASCII forms). When `use_color` is true, the issue severity tags SHALL be wrapped in ANSI color escapes — `error:` in red (`\x1b[31m...\x1b[0m`) and `warn: ` in yellow (`\x1b[33m...\x1b[0m`); the surrounding message text and rule identifier SHALL NOT be colored. When `use_hyperlinks` is true AND a non-empty Obsidian vault id is available for the vault root, each per-file path lead's `wiki/<rel-path>` substring SHALL be wrapped in an OSC 8 hyperlink whose target URL is `obsidian://open?vault=<percent-encoded-vault-id>&file=<percent-encoded-rel-path-from-wiki-root>`; when any of those preconditions is false, the path SHALL appear as plain text without OSC 8 escapes.

The `json` format SHALL print a single JSON object on stdout containing fields `vault_root` (absolute path string), `pages_scanned` (integer), `nav_files_scanned` (integer), `error_count` (integer), `warn_count` (integer), and `issues` (array). Each element of `issues` SHALL contain `path` (absolute filesystem path string, NOT vault-relative), `severity` (`"error"` or `"warn"`), `rule` (a stable kebab-case rule identifier from the rule set), and `message` (human-readable description). The JSON format SHALL NOT contain any emoji glyph, ANSI escape sequence, or OSC 8 hyperlink escape regardless of terminal capability.

The two formats SHALL never be mixed in the same invocation; selecting `--format json` SHALL suppress all human-readable text output to stdout.

#### Scenario: Text format emits vault-relative paths

- **WHEN** `codebus lint` runs without `--format json` against a vault with an issue in `wiki/concepts/auth.md`
- **THEN** stdout SHALL contain the substring `concepts/auth.md` AND SHALL NOT contain the substring of the absolute path leading up to that file

#### Scenario: Text format emoji header on clean vault when emoji enabled

- **WHEN** `codebus lint` runs against a vault with zero issues, in a TTY environment with `NO_COLOR` unset
- **THEN** stdout SHALL contain a line beginning with `✅` followed by the coverage substring `<N> page` where `<N>` is an integer

#### Scenario: Text format ASCII fallback when emoji disabled

- **WHEN** `codebus lint` runs against a vault with zero issues, with stdout redirected to a file (non-TTY)
- **THEN** stdout SHALL contain a line beginning with the literal token `ok` followed by the coverage substring AND SHALL NOT contain any emoji glyph

#### Scenario: Text format ANSI red error tag when color enabled

- **WHEN** `codebus lint` runs against a vault with one error in `wiki/concepts/foo.md`, in a TTY environment with `NO_COLOR` unset
- **THEN** stdout SHALL contain the substring `\x1b[31merror:\x1b[0m` (the ANSI red wrap on the `error:` tag)

#### Scenario: Text format ANSI yellow warn tag when color enabled

- **WHEN** `codebus lint` runs against a vault with one warning in `wiki/concepts/foo.md`, in a TTY environment with `NO_COLOR` unset
- **THEN** stdout SHALL contain the substring `\x1b[33mwarn: \x1b[0m` (the ANSI yellow wrap on the `warn: ` tag)

#### Scenario: Text format suppresses ANSI when NO_COLOR set

- **WHEN** `codebus lint` runs against a vault with one error, with `NO_COLOR=1` in the environment
- **THEN** stdout SHALL NOT contain any byte sequence beginning with `\x1b[` (no ANSI escapes at all)

#### Scenario: Text format wraps wiki path in OSC 8 hyperlink when vault id present

- **WHEN** `codebus lint` runs against a vault with one issue in `wiki/concepts/foo.md`, in a TTY environment that supports OSC 8, where `lookup_vault_id` returns `Some("abcdef")`
- **THEN** stdout SHALL contain the substring `\x1b]8;;obsidian://open?vault=abcdef&file=concepts/foo.md\x1b\\wiki/concepts/foo.md\x1b]8;;\x1b\\` (an OSC 8 wrap whose visible label is `wiki/concepts/foo.md` and whose URL targets the Obsidian vault opener)

#### Scenario: Text format omits OSC 8 when vault id absent

- **WHEN** `codebus lint` runs against a vault with one issue, in a TTY environment that supports OSC 8, where `lookup_vault_id` returns `None` (Obsidian config not present or vault not registered)
- **THEN** stdout SHALL contain the literal string `wiki/concepts/foo.md` AND SHALL NOT contain any OSC 8 escape sequence beginning with `\x1b]8;`

#### Scenario: Text format URL-encodes vault id and file path

- **WHEN** the resolved Obsidian vault id is `my vault` (containing a space) and the issue path is `processes/auth flow.md`
- **THEN** stdout SHALL contain the substring `vault=my%20vault&file=processes/auth%20flow.md` (with spaces percent-encoded as `%20`)

#### Scenario: JSON format emits absolute paths

- **WHEN** `codebus lint --format json` runs against a vault rooted at `<abs-vault>/wiki/` with an issue in `<abs-vault>/wiki/concepts/auth.md`
- **THEN** stdout SHALL contain a single JSON object whose `issues[].path` for that issue equals `<abs-vault>/wiki/concepts/auth.md` (absolute path)

#### Scenario: JSON format includes vault_root field

- **WHEN** `codebus lint --format json` runs against a vault rooted at `<abs-vault>/`
- **THEN** the JSON object SHALL contain a `vault_root` string field equal to the absolute path of `<abs-vault>/`

#### Scenario: JSON format suppresses human text and styling escapes

- **WHEN** `codebus lint --format json` runs against any vault, regardless of TTY / `NO_COLOR` / OSC 8 support
- **THEN** stdout SHALL parse as a single valid JSON value AND SHALL NOT contain emoji, ANSI color codes, OSC 8 hyperlink escapes, or any non-JSON prefix or suffix


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
- **Allow**: when the command's first argv token resolves to a `codebus` binary (file basename `codebus` or `codebus.exe`, case-insensitive match) AND EITHER (a) the second argv token is exactly `lint`, OR (b) the second argv token is exactly `quiz` AND the third argv token is exactly `validate`, the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON to stdout. The two allowed forms correspond to the codebus-fix agent self-checking via `codebus lint` and the codebus-quiz generate agent self-validating via `codebus quiz validate` respectively; no other `codebus` subcommand and no other binary is permitted.
- **Block**: in all other cases (different binary, neither the `lint` nor the `quiz validate` form, malformed input, parse error), the subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` describes why the command was blocked.
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

#### Scenario: hook check-bash allows codebus quiz validate invocation

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus quiz validate -` OR `/usr/local/bin/codebus quiz validate draft.md --json`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash blocks non-codebus binaries

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `echo MARKER`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field is a non-empty string

#### Scenario: hook check-bash blocks codebus subcommands other than the two allowed forms

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus fix --no-fix` OR `codebus quiz "some topic"` (the generate form, not `quiz validate`)
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-bash fails closed on malformed input

- **WHEN** `codebus hook check-bash` receives stdin that does not parse as JSON
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (fail-closed default — never silently allow on parse failure)

#### Scenario: Vault internal gitignore excludes settings.local.json

- **WHEN** `codebus init` runs against `<repo>` and reaches the vault internal `.gitignore` mutation step
- **THEN** the file `<vault_root>/.gitignore` SHALL contain a line equal to `.claude/settings.local.json`

---
### Requirement: Fix Loop Library Invocation Entry Point

The single-shot fix loop SHALL be reachable from two entry points: (a) `codebus_core::verb::fix::run_fix` (the `fix` verb library function defined by the `verb-library` capability) AND (b) `codebus_core::verb::goal::run_goal` (the goal verb's post-agent lint-and-fix phase, when `GoalOptions.no_fix` is false). Both entry points SHALL invoke the same `codebus_core::wiki::fix::run_fix_loop` primitive with identical semantics — the existing fix loop behavior contracts in this capability (Fix Loop Agent Sandbox, Fix Single-Shot Verification, Fix Loop Configuration, Fix Bash Hook Installation, Standalone Fix Mode) SHALL apply unchanged at both entry points.

The CLI subcommand handlers in `codebus-cli/src/commands/{goal,fix}.rs` SHALL NOT invoke `run_fix_loop` directly. Direct invocation of `run_fix_loop` from the CLI binary crate SHALL be forbidden — the only callers SHALL be the verb library functions in `codebus_core::verb`. This delegation contract preserves a single source of truth for the fix loop's orchestration shape so future GUI callers (e.g., `v3-app-workspace-goal`) reuse identical behavior without re-implementing the spawn, hook installation, and verification sequence.

The fix loop's caller-observable behavior — Bash tool gated to `codebus lint *`, single-shot agent spawn, CLI as final-only verifier, `auto_commit` message strings — SHALL remain byte-equivalent after this change.

#### Scenario: Fix loop reachable from run_fix library function

- **WHEN** `codebus_core::verb::fix::run_fix` is invoked AND the lint pre-check finds at least one issue
- **THEN** the function SHALL invoke `codebus_core::wiki::fix::run_fix_loop` exactly once AND the loop's behavior contracts (Bash hook gate, agent toolset, final-only lint verification) SHALL be identical to the behavior reachable via `codebus fix` CLI invocation

#### Scenario: Fix loop reachable from run_goal library function

- **WHEN** `codebus_core::verb::goal::run_goal` is invoked with `GoalOptions { no_fix: false, .. }` AND the post-agent lint detects at least one issue AND `lint.fix.enabled` is true in config
- **THEN** the function SHALL invoke `codebus_core::wiki::fix::run_fix_loop` exactly once after the goal agent terminates AND before the final auto-commit step

#### Scenario: CLI binary does not call run_fix_loop directly

- **WHEN** a static search is performed across `codebus-cli/src/**/*.rs` for direct references to `codebus_core::wiki::fix::run_fix_loop`
- **THEN** the search SHALL return zero matches (the only callers SHALL be inside `codebus-core/src/verb/{goal,fix}.rs`)
