# lint-feedback-loop Specification

## Purpose

The wiki-content validation rule set, lint output formats, fix-loop spawn pattern, and Bash sandbox hook installation — 7 deterministic lint rules over `<vault>/wiki/` (frontmatter integrity, slug uniqueness, body wikilink resolution, nav file integrity, related-format, root-page placement), text and JSON output formats with vault-relative / absolute path conventions, single-shot fix flow that spawns the codebus-fix agent at most once per CLI invocation with CLI as final-only verifier, the `lint.fix.*` configuration schema, and the PreToolUse hook installed via `<vault>/.claude/settings.json` that hard-gates the agent's Bash tool to `codebus lint *` or `codebus quiz validate *` only (quiz Mode B's self-validation reuses the same hook). Does NOT cover the SKILL.md content the agent loads (lives in `skill-bundles` per-verb workflow content), nor source-repo or vault structural concerns (live in `vault`).

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
- **Block (shell metacharacter)**: when the command string contains any byte from the metacharacter rejection set — semicolon, ampersand, pipe, dollar sign, backtick, greater-than, less-than, open-paren, close-paren, line feed (LF, byte 0x0A), or carriage return (CR, byte 0x0D) — the subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` identifies the rejected metacharacter and the command being blocked. The metacharacter rejection SHALL apply regardless of whether the byte appears outside quotes, inside double quotes, inside single quotes, or after an escape character — the predicate is byte-level on the raw command string AND SHALL NOT depend on shell quote parsing. The metacharacter rejection SHALL be evaluated BEFORE the argv-tokenization-based allow predicate, so a metacharacter hit blocks the command even when its leading argv tokens satisfy the allow form on their own. The **Allow (quiz-validate heredoc)** structural exception defined below SHALL be evaluated BEFORE this metacharacter rejection; when that exception's conditions are met, the command SHALL be allowed even though it contains the less-than and line-feed metacharacters.
- **Allow**: when the command string contains no metacharacter from the rejection set AND the command's first argv token resolves to a `codebus` binary (file basename `codebus` or `codebus.exe`, case-insensitive match on Windows, case-sensitive on Unix) AND EITHER (a) the second argv token is exactly `lint`, OR (b) the second argv token is exactly `quiz` AND the third argv token is exactly `validate`, the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON to stdout. The two allowed forms correspond to the codebus-fix agent self-checking via `codebus lint` AND the codebus-quiz generate agent self-validating via `codebus quiz validate` respectively; no other `codebus` subcommand AND no other binary is permitted.
- **Allow (quiz-validate heredoc)**: As a structural exception to the metacharacter rejection — the sole circumstance under which a command containing the less-than or line-feed metacharacters SHALL be allowed — the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON when the command string is a well-formed single-quoted `codebus quiz validate` here-document, defined as ALL of the following holding:
  1. When the command string is split on line feed (LF) into lines (each line's trailing carriage return removed for comparison), the FIRST line SHALL consist of a `codebus quiz validate` invocation immediately followed by a here-document redirection operator of the form `<<'<MARKER>'`, where `<MARKER>` is a non-empty sequence of word characters (`[A-Za-z0-9_]+`). On the first line, the portion preceding the `<<` operator SHALL contain no metacharacter from the rejection set AND SHALL satisfy the `codebus quiz validate` argv form (first argv token resolves to a `codebus` binary, second argv token is exactly `quiz`, third argv token is exactly `validate`); the portion following the closing single quote of `<MARKER>` SHALL be empty or whitespace only (no trailing command).
  2. The here-document marker SHALL be enclosed in single quotes. A here-document operator whose marker is not single-quoted SHALL NOT qualify — an unquoted marker permits parameter and command substitution inside the body, which would re-introduce the shell-injection vector this exception is designed to avoid.
  3. Exactly one subsequent line SHALL equal `<MARKER>` verbatim (the closing delimiter, at column zero with no leading whitespace). Every line after that closing delimiter line SHALL be empty or whitespace only.
  4. The lines between the first line and the closing delimiter line (the here-document body) SHALL be treated as opaque standard-input content: they SHALL NOT be scanned for metacharacters AND their content SHALL NOT affect the decision.
  When any of these conditions fails, the command SHALL fall through to the metacharacter rejection AND argv-tokenization allow/block paths (so a non-conforming command containing less-than or LF is blocked there).
- **Block (other)**: in all other cases (different binary, neither the `lint` nor the `quiz validate` form nor a conforming quiz-validate here-document, malformed input, parse error), the subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` describes why the command was blocked.
- **Cross-platform**: the binary basename match SHALL be case-insensitive on Windows (`codebus.EXE` AND `codebus.exe` both allowed) AND case-sensitive on Unix. The metacharacter rejection set SHALL be the union of POSIX shell (bash, Git Bash), PowerShell, AND `cmd.exe` high-risk symbols AND SHALL NOT vary per OS — identical byte set is rejected on every platform.

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

#### Scenario: hook check-bash allows quiz validate single-quoted heredoc self-validation

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is a single-quoted `codebus quiz validate` here-document — first line `codebus quiz validate - <<'CBQZ'`, one or more body lines, and a final line equal to `CBQZ`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

##### Example: well-formed quiz-validate heredoc allowed

- **GIVEN** the command string `codebus quiz validate - <<'CBQZ'\n## Q1. What is a vault?\nA) ...\n## Answer: A\nCBQZ` (where `\n` denotes a line feed)
- **WHEN** `codebus hook check-bash` evaluates it
- **THEN** the subcommand SHALL exit zero with no `decision` JSON on stdout

#### Scenario: hook check-bash allows quiz validate heredoc with json flag

- **WHEN** `codebus hook check-bash` receives a command whose first line is `codebus quiz validate --json - <<'CBQZ'`, followed by body lines and a closing line equal to `CBQZ`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash allows heredoc body containing shell metacharacters

- **WHEN** `codebus hook check-bash` receives a single-quoted `codebus quiz validate` here-document whose body lines contain shell metacharacters (for example a quiz question whose text includes dollar sign, pipe, semicolon, and parentheses), with the first line `codebus quiz validate - <<'CBQZ'` and a closing line equal to `CBQZ`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON (the here-document body is opaque standard-input content and SHALL NOT be scanned for metacharacters)

#### Scenario: hook check-bash blocks non-codebus binaries

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `echo MARKER`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field is a non-empty string

#### Scenario: hook check-bash blocks codebus subcommands other than the two allowed forms

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus fix --no-fix` OR `codebus quiz "some topic"` (the generate form, not `quiz validate`)
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-bash fails closed on malformed input

- **WHEN** `codebus hook check-bash` receives stdin that does not parse as JSON
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (fail-closed default — the subcommand SHALL NEVER silently allow on parse failure)

#### Scenario: hook check-bash blocks command with logical-AND shell chaining

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint --format json && rm -rf /tmp/evil`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field mentions a shell metacharacter

#### Scenario: hook check-bash blocks command with semicolon separator

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint; curl evil.example`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-bash blocks command with command substitution even when leading tokens are valid

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint $(whoami)` OR a `codebus lint` invocation followed by a backtick-wrapped `whoami` substitution
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field mentions a shell metacharacter

#### Scenario: hook check-bash blocks metacharacter inside quoted argument

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint --filter "foo;bar"`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (the metacharacter rejection SHALL NOT depend on shell quote parsing)

#### Scenario: hook check-bash blocks quiz validate heredoc with chaining on the first line

- **WHEN** `codebus hook check-bash` receives a command whose first line is `codebus quiz validate - <<'X'; rm -rf ~` (a chained command following the here-document operator on the same line), with body lines and a closing line equal to `X`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (the first line carries a trailing command after the operator, so the heredoc exception SHALL NOT apply)

#### Scenario: hook check-bash blocks command after the heredoc closing delimiter

- **WHEN** `codebus hook check-bash` receives a single-quoted `codebus quiz validate` here-document that is correctly opened and closed, but a non-empty line following the closing delimiter line contains a further command (for example `rm -rf ~`)
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (content after the closing delimiter SHALL disqualify the heredoc exception)

#### Scenario: hook check-bash blocks unquoted quiz validate heredoc

- **WHEN** `codebus hook check-bash` receives a command whose first line is `codebus quiz validate - <<CBQZ` (the marker is NOT single-quoted), with body lines and a closing line equal to `CBQZ`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (an unquoted marker permits shell expansion inside the body, so the heredoc exception SHALL NOT apply)

#### Scenario: hook check-bash blocks non-heredoc input redirection

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus quiz validate < ~/.ssh/id_rsa` OR `codebus lint < /etc/passwd`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (a single less-than input redirection is not a here-document; the metacharacter rejection on less-than SHALL still apply)

#### Scenario: Vault internal gitignore excludes settings.local.json

- **WHEN** `codebus init` runs against `<repo>` and reaches the vault internal `.gitignore` mutation step
- **THEN** the file `<vault_root>/.gitignore` SHALL contain a line equal to `.claude/settings.local.json`


<!-- @trace
source: quiz-heredoc-selfvalidate-unblock
updated: 2026-05-29
code:
  - codebus-cli/src/commands/hook.rs
-->

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

---
### Requirement: PII Image Read Hook Installation

The `codebus init` subcommand SHALL ensure `<vault_root>/.claude/settings.json` contains `hooks.PreToolUse` entries whose `matcher` fields equal `"Read"`, `"Glob"`, AND `"Grep"` respectively, each routing to `codebus hook check-read` as a `command`-type hook, in addition to the Bash matcher entry required by `Fix Bash Hook Installation`. The same write-if-missing semantics from `Fix Bash Hook Installation` SHALL apply at the file level: if `<vault_root>/.claude/settings.json` already exists, init SHALL NOT modify it. Existing vaults predating this requirement SHALL be upgraded via release-note guidance (manual JSON snippet insertion or re-init at a new location), NOT by automatic in-place migration; the `Vault Gate Integrity Check` requirement provides the detection signal for such vaults.

The `codebus hook check-read` subcommand SHALL intercept the `Read`, `Glob`, AND `Grep` tools. The read target path SHALL be resolved by `tool_name`: `tool_input.file_path` for `Read`, AND `tool_input.path` for `Glob` or `Grep`. The `Vault Containment Read Gate` requirement SHALL be evaluated FIRST on this target path; the image / sensitive-path denylist defined below SHALL apply only after containment has allowed the path through, AND serves as in-vault defense-in-depth.

The hook entries SHALL be installed unconditionally — their runtime behavior is gated at hook-invocation time by the `hooks.read_image_block` (denylist) AND `hooks.read_path_containment` (containment) config keys, NOT by conditional install-time logic. This SHALL allow `~/.codebus/config.yaml` to be the single source of truth: changing a config key takes immediate effect for all existing vaults without requiring re-init or per-vault edits to `settings.json`.

The `codebus hook check-read` subcommand SHALL read `~/.codebus/config.yaml` at the start of every invocation AND SHALL consult the boolean configuration key `hooks.read_image_block` for the denylist stage. The key resolution rules SHALL be:

- When the config file does not exist, OR the file fails to parse as YAML, OR the `hooks` section is absent, OR the `read_image_block` key is absent, OR the key is a non-boolean value: the subcommand SHALL behave as if the key were `true` (fail-safe to block).
- When `hooks.read_image_block` is the boolean `false`: the subcommand SHALL NOT execute the image-extension blocklist, the sensitive-path blocklist, or the Read fail-closed stdin checks. The denylist stage is skipped; containment is unaffected (governed independently by `hooks.read_path_containment`).
- When `hooks.read_image_block` is the boolean `true`: the subcommand SHALL execute the denylist stdin/stdout contract defined in the following paragraphs (the image extension blocklist, the sensitive-path blocklist, AND the Read fail-closed branches).

When `hooks.read_image_block` resolves to `true`, the denylist stage of `codebus hook check-read` SHALL implement the following stdin/stdout contract on the resolved target path:

- **Input** (stdin): a single JSON object matching Claude Code's PreToolUse hook input schema. The relevant fields are `tool_name` (`"Read"`, `"Glob"`, or `"Grep"`) AND the corresponding target-path field (`tool_input.file_path` for `Read`, `tool_input.path` for `Glob`/`Grep`).
- **Block (image extension)**: when the target path is a non-empty string whose extension matches any of the following blocklist members (compared ASCII case-insensitively after stripping the directory portion using either `/` or `\` as separator): `png`, `jpg`, `jpeg`, `gif`, `webp`, `bmp`, `tiff`, `tif`, `pdf`, `ico`, `heic`, `heif`, `avif`. The subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` identifies the blocked path.
- **Block (sensitive path)**: when the target path, after path-separator normalization to forward-slash AND after expanding a leading `~` to the running user's home directory, starts with any of the following sensitive directory prefixes (compared ASCII case-insensitively): `<home>/.ssh/`, `<home>/.aws/`, `<home>/.gnupg/`, `<home>/.config/gh/`. OR when the path's basename matches (compared ASCII case-insensitively) any of the following glob patterns: `*id_rsa*`, `*.pem`, `*.key`. The subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` whose reason identifies the sensitive-path rule that fired AND the blocked path.
- **Block (unresolvable home)**: when the running environment provides no resolvable home directory (no usable `HOME` on Unix, no usable `USERPROFILE` on Windows) AND the target path requires home resolution to evaluate the sensitive-prefix rule, the subcommand SHALL exit with status zero AND SHALL print a block decision JSON whose reason identifies that the home directory is unresolvable (fail-closed). Paths the basename-glob rule alone can decide (e.g., `/tmp/random/server.pem`) SHALL still be evaluated independently of home resolution.
- **Allow**: when the target path is a non-empty string whose extension is not in the image blocklist AND whose path does not match the sensitive-path rule AND home resolution is not required (or succeeded), the subcommand SHALL exit with status zero AND SHALL NOT print a denylist decision JSON to stdout.
- **Fail-closed (Read missing path)**: when `tool_name` is `"Read"` AND the stdin lacks `tool_input.file_path`, contains a non-string `file_path`, or contains an empty-string `file_path`, the subcommand SHALL exit with status zero AND SHALL print a block decision JSON. A `Glob` or `Grep` invocation that omits `tool_input.path` SHALL NOT be failed closed by this branch (its implicit search root is the vault root, governed by `Vault Containment Read Gate`).
- **Fail-closed (malformed stdin)**: when stdin is empty OR fails to parse as JSON, the subcommand SHALL exit with status zero AND SHALL print a block decision JSON. The subcommand SHALL NEVER silently allow on parse failure.
- **Cross-platform extension comparison**: the extension match SHALL be ASCII case-insensitive on all platforms. The sensitive-path match SHALL likewise be ASCII case-insensitive on all platforms AND SHALL normalize path separators (both `/` and `\`) to forward-slash before prefix comparison, so `C:\Users\harry\.ssh\config` AND `/home/harry/.ssh/config` both trigger the same rule on their respective OS.
- **Path separator handling**: the implementation SHALL strip the directory portion using either `/` or `\` as a path separator before extracting the extension AND before evaluating the basename glob, so `/repo/img.png`, `C:\repo\img.png`, AND `C:/repo/img.png` all extract the same extension AND basename.

The hook installer SHALL emit the `Read`, `Glob`, AND `Grep` matcher entries as siblings of the existing `Bash` matcher entry in the same `hooks.PreToolUse` array, not as a nested structure.

The `hooks.read_image_block` key SHALL belong to a top-level `hooks` namespace in `~/.codebus/config.yaml`, parallel to existing top-level namespaces (`pii`, `lint`, `quiz`, `goal`, `log`, `app`, `claude_code`) AND to the `hooks.read_path_containment` key. The default value SHALL be `true`. The starter config file written by `codebus init` (when no global config exists) SHALL include a documented `hooks` section with `read_image_block: true` AND `read_path_containment: true` AND inline commentary describing each trade-off.

#### Scenario: Init writes Read, Glob, and Grep matcher entries alongside Bash on fresh vault

- **WHEN** `codebus init` runs against a repository with no existing `<vault_root>/.claude/settings.json`
- **THEN** the system SHALL create `<vault_root>/.claude/settings.json` AND the file content SHALL parse as JSON AND the `hooks.PreToolUse` array SHALL contain a Bash matcher entry invoking `codebus hook check-bash` AND `Read`, `Glob`, AND `Grep` matcher entries each invoking `codebus hook check-read`

#### Scenario: Init does not overwrite existing settings.json for Read hook migration

- **WHEN** `codebus init` runs against a vault where `<vault_root>/.claude/settings.json` already exists with a prior matcher set
- **THEN** the system SHALL NOT modify the existing file AND its byte-content SHALL be identical before and after init

#### Scenario: hook check-read blocks blacklisted image extensions

- **WHEN** `codebus hook check-read` receives stdin JSON whose resolved in-vault target path ends with any image blocklist extension AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field is a non-empty string identifying the blocked path

##### Example: extension blocklist coverage

| Input target path | Decision | Notes |
| --- | --- | --- |
| `wiki/diagrams/flow.png` | block | image |
| `assets/logo.JPG` | block | case-insensitive match |
| `docs/manual.pdf` | block | pdf |
| `art/sprite.avif` | block | newer image format |
| `photo.HEIC` | block | uppercase iOS format |
| `wiki/foo.md` | allow | text |
| `src/main.rs` | allow | source code |
| `wiki/diagram.svg` | allow | xml text, scannable by regex_basic |
| `Makefile` | allow | no extension |
| `script` | allow | no extension |

#### Scenario: hook check-read blocks across path separator styles

- **WHEN** `codebus hook check-read` receives stdin JSON whose resolved in-vault target path is `<vault>/assets/img.png` expressed with `/`, `\`, or mixed separators AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-read allows non-image extensions

- **WHEN** `codebus hook check-read` receives stdin JSON whose resolved in-vault target path is `wiki/modules/uv-lib.md` OR `raw/code/agent/claude_cli.rs` OR `Makefile` AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-read fails closed on missing or invalid Read file_path

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Read","tool_input":{}}` (no `file_path`) OR `{"tool_name":"Read","tool_input":{"file_path":""}}` (empty string) OR `{"tool_name":"Read","tool_input":{"file_path":123}}` (non-string) AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-read fails closed on malformed stdin

- **WHEN** `codebus hook check-read` receives stdin that does not parse as JSON OR stdin that is empty AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (fail-closed default — the subcommand SHALL NEVER silently allow on parse failure)

#### Scenario: hook check-read with read_image_block disabled skips the denylist

- **WHEN** `~/.codebus/config.yaml` contains `hooks.read_image_block: false` AND `codebus hook check-read` receives stdin JSON with any in-vault `tool_input.file_path` value (image extension, sensitive-path hit, text extension, or malformed JSON)
- **THEN** the subcommand SHALL NOT print any denylist `decision` JSON, regardless of the stdin contents (containment, governed independently by `hooks.read_path_containment`, is unaffected by this key)

#### Scenario: Missing config file resolves read_image_block to true (fail-safe block)

- **WHEN** `~/.codebus/config.yaml` does not exist AND `codebus hook check-read` receives stdin JSON whose in-vault `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON to stdout

#### Scenario: Malformed config yaml resolves read_image_block to true (fail-safe block)

- **WHEN** `~/.codebus/config.yaml` contains content that fails to parse as YAML AND `codebus hook check-read` receives stdin JSON whose in-vault `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON to stdout (the hook subcommand SHALL NEVER be made permissive by a config load failure)

#### Scenario: Absent hooks section resolves read_image_block to true

- **WHEN** `~/.codebus/config.yaml` exists AND parses successfully but contains no `hooks` section AND `codebus hook check-read` receives stdin JSON whose in-vault `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON

#### Scenario: hook check-read blocks sensitive key basename inside the vault

- **WHEN** `codebus hook check-read` receives stdin JSON whose resolved in-vault target path has a basename matching `*id_rsa*`, `*.pem`, or `*.key` (e.g., a key file that slipped into `raw/code/`) AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field mentions the basename-glob rule

<!-- @trace
source: agent-hook-hardening
updated: 2026-06-04
code:
  - codebus-cli/src/commands/hook.rs
  - codebus-core/src/vault/settings.rs
  - codebus-core/src/config/hooks.rs
  - codebus-core/src/skill_bundle/mod.rs
tests:
  - codebus-cli/tests/hook_check_read.rs
-->

---
### Requirement: Vault Gate Integrity Check

The lint subsystem SHALL verify that the vault PreToolUse gate configuration at `<vault-root>/.claude/settings.json` still installs the hooks codebus relies on to sandbox the claude-path agent: a `Bash` matcher routing to `codebus hook check-bash`, AND `Read`, `Glob`, AND `Grep` matchers each routing to `codebus hook check-read`. This check SHALL read exactly that single file; it SHALL NOT scan, traverse, or read any other path outside the `wiki/` subtree, AND SHALL NOT broaden lint into a general vault-structure validator. The check is a detection signal only — it SHALL NOT modify, restore, or rewrite the settings file (the Lint Read-Only Invariant continues to hold).

The required hook set (the matcher → command pairs `Bash` → `codebus hook check-bash`, `Read` → `codebus hook check-read`, `Glob` → `codebus hook check-read`, AND `Grep` → `codebus hook check-read`) SHALL be sourced from the same definition that `codebus init` uses to author the default settings file, so the linter AND the installer cannot drift.

The check SHALL emit a lint issue with `severity: error` AND the stable kebab-case rule identifier `vault-gate-integrity` when ANY of the following holds: the settings file is absent; the file does not parse as JSON; `hooks.PreToolUse` is missing or is not an array; OR any one of the four required hook entries (`Bash` → check-bash, `Read` → check-read, `Glob` → check-read, `Grep` → check-read) is absent. The issue `message` SHALL identify which condition failed (which required hook is missing, or that the file is absent / unparseable). When ALL four required hook entries are present, the check SHALL emit NO `vault-gate-integrity` issue, regardless of any additional user-added matcher entries, hook commands, or top-level keys present in the file (preserving the write-if-missing user-customization contract).

The issue path for a `vault-gate-integrity` finding SHALL be the settings file location: in `text` format it SHALL render as the vault-relative path `.claude/settings.json` verbatim, WITHOUT the `wiki/` prefix that the text format applies to wiki-subtree issue paths; in `json` format the issue `path` SHALL be the absolute filesystem path of the settings file. This finding SHALL be counted in the `error_count` totals like any other error-severity issue.

#### Scenario: Intact gate produces no issue

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Bash` → `codebus hook check-bash`, `Read` → `codebus hook check-read`, `Glob` → `codebus hook check-read`, AND `Grep` → `codebus hook check-read` PreToolUse hook entries
- **THEN** the lint result SHALL NOT contain any issue whose `rule` is `vault-gate-integrity`

#### Scenario: Emptied PreToolUse array is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` parses as JSON but whose `hooks.PreToolUse` array has been rewritten to empty
- **THEN** the lint result SHALL contain one `error`-severity issue whose `rule` is `vault-gate-integrity` per missing required hook — i.e., four such issues when all of the `Bash`, `Read`, `Glob`, AND `Grep` gates are absent — AND each issue `message` SHALL identify the specific missing gate

#### Scenario: Missing Bash gate hook is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Read`, `Glob`, AND `Grep` check-read entries but not the `Bash` → `codebus hook check-bash` entry
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue whose `message` identifies the missing `Bash` check-bash gate

#### Scenario: Missing Read gate hook is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Bash`, `Glob`, AND `Grep` entries but not the `Read` → `codebus hook check-read` entry
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue whose `message` identifies the missing `Read` check-read gate

#### Scenario: Missing Glob gate hook is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Bash`, `Read`, AND `Grep` entries but not the `Glob` → `codebus hook check-read` entry
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue whose `message` identifies the missing `Glob` check-read gate

#### Scenario: Missing Grep gate hook is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Bash`, `Read`, AND `Glob` entries but not the `Grep` → `codebus hook check-read` entry
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue whose `message` identifies the missing `Grep` check-read gate

#### Scenario: User-added settings do not cause a false positive

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` retains all four required hook entries AND also contains additional user-added PreToolUse entries or unrelated top-level keys
- **THEN** the lint result SHALL NOT contain any `vault-gate-integrity` issue

#### Scenario: Absent or unparseable settings file is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` is absent, OR whose content does not parse as JSON
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue

#### Scenario: Gate finding path representation per format

- **WHEN** a `vault-gate-integrity` issue is emitted for a vault rooted at `<abs-vault>/`
- **THEN** in `text` format the issue path SHALL render as `.claude/settings.json` with no `wiki/` prefix AND in `json` format the issue `path` SHALL equal `<abs-vault>/.claude/settings.json` (absolute)

#### Scenario: Gate check never modifies the vault

- **WHEN** the system runs lint against any vault, whether or not the gate is intact
- **THEN** `<vault-root>/.claude/settings.json` SHALL be byte-identical before and after the lint invocation

<!-- @trace
source: agent-run-integrity
updated: 2026-06-04
code:
  - codebus-core/src/wiki/lint/rules/vault_gate_integrity.rs
  - codebus-core/src/wiki/lint/rules/mod.rs
  - codebus-core/src/wiki/lint/factory.rs
  - codebus-core/src/wiki/lint/rule.rs
  - codebus-core/src/wiki/lint/mod.rs
  - codebus-core/src/wiki/lint/output.rs
  - codebus-core/src/render/lint_text.rs
  - codebus-core/src/vault/settings.rs
tests:
  - codebus-cli/tests/lint_flow.rs
-->

---
### Requirement: Vault Containment Read Gate

The `codebus hook check-read` subcommand SHALL enforce a vault-root containment boundary on the agent's read target path BEFORE the image / sensitive-path denylist defined in `PII Image Read Hook Installation`. This boundary is the primary read security gate; the denylist is retained only as in-vault defense-in-depth. Containment SHALL apply to the read target path of the `Read`, `Glob`, AND `Grep` tools.

The boundary SHALL be gated by a boolean configuration key `hooks.read_path_containment` in the top-level `hooks` namespace of `~/.codebus/config.yaml`, parallel to `hooks.read_image_block` AND independent of it. The default value SHALL be `true`. Key resolution SHALL be fail-safe: when the config file does not exist, OR fails to parse as YAML, OR the `hooks` section is absent, OR the `read_path_containment` key is absent, OR the key is a non-boolean value, the subcommand SHALL behave as if the key were `true`. The `read_image_block` key SHALL NOT enable or disable containment, AND `read_path_containment` SHALL NOT enable or disable the denylist; the two gates are independent.

The read target path SHALL be resolved from the PreToolUse stdin JSON by `tool_name`: when `tool_name` is `"Read"`, the target path is `tool_input.file_path`; when `tool_name` is `"Glob"` or `"Grep"`, the target path is `tool_input.path`. For `Glob` AND `Grep`, an absent or empty `tool_input.path` SHALL denote the implicit search root (the agent process working directory, which is the vault root) AND SHALL be treated as in-vault — the subcommand SHALL NOT block a `Glob` or `Grep` invocation solely because it omits `tool_input.path`.

When `hooks.read_path_containment` resolves to `true` AND a non-empty target path is present, the subcommand SHALL decide containment by canonical comparison: it SHALL canonicalize the vault root AND canonicalize the target path (resolving a relative target path against the vault root first), applying the same canonicalization to both operands so platform-specific forms (Windows `\\?\` verbatim prefixes, drive-letter case, 8.3 short names, UNC paths, AND `/` versus `\` separators) normalize identically. When the canonicalized target path equals the vault root OR is a descendant of it, the subcommand SHALL allow the path to proceed to the denylist stage. When the canonicalized target path is NOT within the vault root, the subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` whose reason identifies the vault-containment rule AND the blocked path.

When `hooks.read_path_containment` resolves to `false`, the subcommand SHALL NOT perform the containment comparison AND SHALL pass the target path directly to the denylist stage (which remains governed by `hooks.read_image_block`).

The vault root used for containment SHALL be obtained from the PreToolUse stdin `cwd` field (the agent working directory, which codebus sets to the vault root; empirically confirmed to be present AND to equal the vault root), with the hook subprocess working directory as an equivalent fallback when the stdin `cwd` field is absent, AND without introducing a new persistent config field. This requirement constrains the observable containment behavior; the named source is the resolved sourcing mechanism.

#### Scenario: Containment blocks an out-of-vault absolute Read path

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Read","tool_input":{"file_path":"<abs-path-outside-vault>"}}` where the path canonicalizes outside the vault root (e.g., the parent source repository, `~/.kube/config`, or `~/.env`) AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field identifies the vault-containment rule

#### Scenario: Containment blocks an out-of-vault Grep path

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Grep","tool_input":{"pattern":"SECRET","path":"<abs-dir-outside-vault>"}}` where the path canonicalizes outside the vault root AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field identifies the vault-containment rule

#### Scenario: Containment allows an in-vault relative Read path

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Read","tool_input":{"file_path":"raw/code/src/main.rs"}}` resolving under the vault root AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL NOT print a containment block decision AND the path SHALL proceed to the denylist stage

#### Scenario: Containment allows an in-vault absolute path from the fix workflow

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Read","tool_input":{"file_path":"<abs-vault>/wiki/modules/auth.md"}}` where the absolute path lies under the vault root (as produced by the `codebus lint` issue paths the fix workflow consumes verbatim) AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL NOT print a containment block decision AND the path SHALL proceed to the denylist stage

#### Scenario: Glob or Grep omitting path is treated as in-vault

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Grep","tool_input":{"pattern":"foo"}}` OR `{"tool_name":"Glob","tool_input":{"pattern":"**/*.md"}}` with no `path` field AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL NOT print a containment block decision AND SHALL NOT fail closed on the absent `path` (the implicit search root is the vault root)

#### Scenario: Containment disabled passes an out-of-vault path through

- **WHEN** `~/.codebus/config.yaml` contains `hooks.read_path_containment: false` AND `codebus hook check-read` receives stdin JSON whose resolved target path canonicalizes outside the vault root
- **THEN** the subcommand SHALL NOT print a containment block decision (the target path proceeds to the denylist stage, which remains governed by `hooks.read_image_block`)

#### Scenario: Containment key fail-safe resolves to true

- **WHEN** `~/.codebus/config.yaml` does not exist, OR fails to parse as YAML, OR lacks a `hooks` section, OR lacks the `read_path_containment` key, OR sets it to a non-boolean value, AND `codebus hook check-read` receives stdin JSON whose resolved target path canonicalizes outside the vault root
- **THEN** the subcommand SHALL behave as if `hooks.read_path_containment` were `true` AND SHALL print a containment block decision

#### Scenario: In-vault path with Windows separator and drive-case variance is allowed

- **WHEN** on Windows, `codebus hook check-read` receives a target path under the vault root expressed with backslash separators or a differently-cased drive letter than the canonicalized vault root (e.g., target `d:\repo\.codebus\wiki\x.md` against vault root `D:\repo\.codebus`) AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL treat the path as in-vault AND SHALL NOT print a containment block decision (both operands normalize under one canonicalization)

<!-- @trace
source: check-read-vault-containment
updated: 2026-06-04
code:
  - codebus-cli/src/commands/hook.rs
  - codebus-core/src/config/hooks.rs
tests:
  - codebus-cli/tests/hook_check_read.rs
-->
