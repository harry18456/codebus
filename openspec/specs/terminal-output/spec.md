# terminal-output Specification

## Purpose

Render agent stream events (thoughts, tool_use, tool_result) and lifecycle banners (start, goal, done, hint) to the terminal in a hybrid emoji/symbol style. Emoji mode is resolved via a 5-level priority chain (CLI flag → CLI sugar → env → global config → auto-detect). ANSI color is applied through chalk only when stdout is a TTY and `NO_COLOR` is unset. Global config (`~/.codebus/config.yaml`) is loaded tolerantly so that future phase-2 fields do not break phase-1 parsing.

## Requirements

### Requirement: Render per-event stream output with emoji or symbol prefix

The system SHALL render each StreamEvent (`thought`, `tool_use`, `tool_result`) to a single terminal line whose prefix indicates the event kind, using emoji glyphs when emoji mode is enabled and ASCII/unicode symbols when disabled.

#### Scenario: Thought event with emoji enabled

- **WHEN** the system renders a `thought` event with text `"thinking"` and emoji enabled
- **THEN** the output line begins with the thought emoji glyph and contains the text `"thinking"`

#### Scenario: Thought event with emoji disabled

- **WHEN** the system renders a `thought` event with emoji disabled
- **THEN** the output line begins with the thought symbol `"◆"` and contains the text, with no emoji glyph present

#### Scenario: tool_use Write rendered with write glyph and file path

- **WHEN** the system renders a `tool_use` event with name `"Write"` and input `{file_path: "wiki/concepts/a.md"}` with emoji enabled
- **THEN** the output line begins with the write emoji glyph and contains the path `"wiki/concepts/a.md"`

#### Scenario: tool_use Read rendered with tool glyph and tool name

- **WHEN** the system renders a `tool_use` event with name `"Read"` and emoji enabled
- **THEN** the output line begins with the tool emoji glyph and contains the name `"Read"`

#### Scenario: tool_result error highlights via color, not separate emoji

- **WHEN** the system renders a `tool_result` event with `isError: true` and color enabled
- **THEN** the output line uses the same result emoji glyph as success but applies red color to the result text, and the output contains the error text


<!-- @trace
source: codebus-v2-phase1
updated: 2026-05-04
code:
  - src/infra/fs/raw-sync.ts
  - docs/superpowers/REVIEW_LESSONS.md
  - src/infra/cli-detect.ts
  - src/core/wiki/types.ts
  - README.md
  - src/core/wiki/frontmatter.ts
  - package.json
  - src/core/vault/lock.ts
  - src/core/vault/sanity-check.ts
  - src/core/wiki/stale-detect.ts
  - src/schema/claude-md.ts
  - src/commands/goal.ts
  - src/core/wiki/date.ts
  - LICENSE
  - src/cli.ts
  - tsconfig.json
  - src/commands/query.ts
  - src/infra/git/source-version.ts
  - src/ui/lint-report.ts
  - docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md
  - src/infra/llm/types.ts
  - .spectra.yaml
  - src/core/vault/layout.ts
  - src/infra/git/nested-repo.ts
  - src/commands/check.ts
  - src/core/wiki/page-merge.ts
  - vitest.config.ts
  - src/commands/init.ts
  - src/ui/stream-parser.ts
  - src/core/wiki/lint.ts
  - src/infra/llm/claude-cli.ts
  - src/infra/fs/file-ops.ts
  - src/ui/emoji-mode.ts
  - src/infra/global-config.ts
  - src/core/wiki/frontmatter-repair.ts
  - src/ui/render.ts
tests:
  - tests/e2e/init-smoke.test.ts
  - tests/infra/fs/file-ops.test.ts
  - tests/commands/goal.test.ts
  - tests/cli.test.ts
  - tests/commands/query.test.ts
  - tests/commands/check.test.ts
  - tests/core/wiki/date.test.ts
  - tests/core/wiki/page-merge.test.ts
  - tests/core/wiki/stale-detect.test.ts
  - tests/infra/cli-detect.test.ts
  - tests/ui/emoji-mode.test.ts
  - tests/core/wiki/frontmatter-repair.test.ts
  - tests/infra/git/source-version.test.ts
  - tests/commands/init.test.ts
  - tests/infra/global-config.test.ts
  - tests/ui/stream-parser.test.ts
  - tests/core/vault/sanity-check.test.ts
  - tests/core/wiki/lint.test.ts
  - tests/infra/git/nested-repo.test.ts
  - tests/infra/fs/raw-sync.test.ts
  - tests/core/vault/layout.test.ts
  - tests/core/vault/lock.test.ts
  - tests/infra/llm/claude-cli.test.ts
  - tests/schema/claude-md.test.ts
  - tests/core/wiki/frontmatter.test.ts
  - tests/ui/render.test.ts
-->

---
### Requirement: Render lifecycle banners

The system SHALL render four banner messages (`start`, `goal`, `done`, `hint`) at appropriate lifecycle moments, with emoji or symbol prefix matching the current emoji mode.

#### Scenario: Start banner shows repo path

- **WHEN** the system renders the `start` banner with `path: "/tmp/myrepo"` and emoji enabled
- **THEN** the output contains the start emoji glyph and the path `"/tmp/myrepo"`

#### Scenario: Done banner with emoji disabled uses symbol

- **WHEN** the system renders the `done` banner with `wikiPath: ".codebus/wiki"` and emoji disabled
- **THEN** the output begins with the done symbol `"✓"` and contains the wiki path, with no emoji glyph present


<!-- @trace
source: codebus-v2-phase1
updated: 2026-05-04
code:
  - src/infra/fs/raw-sync.ts
  - docs/superpowers/REVIEW_LESSONS.md
  - src/infra/cli-detect.ts
  - src/core/wiki/types.ts
  - README.md
  - src/core/wiki/frontmatter.ts
  - package.json
  - src/core/vault/lock.ts
  - src/core/vault/sanity-check.ts
  - src/core/wiki/stale-detect.ts
  - src/schema/claude-md.ts
  - src/commands/goal.ts
  - src/core/wiki/date.ts
  - LICENSE
  - src/cli.ts
  - tsconfig.json
  - src/commands/query.ts
  - src/infra/git/source-version.ts
  - src/ui/lint-report.ts
  - docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md
  - src/infra/llm/types.ts
  - .spectra.yaml
  - src/core/vault/layout.ts
  - src/infra/git/nested-repo.ts
  - src/commands/check.ts
  - src/core/wiki/page-merge.ts
  - vitest.config.ts
  - src/commands/init.ts
  - src/ui/stream-parser.ts
  - src/core/wiki/lint.ts
  - src/infra/llm/claude-cli.ts
  - src/infra/fs/file-ops.ts
  - src/ui/emoji-mode.ts
  - src/infra/global-config.ts
  - src/core/wiki/frontmatter-repair.ts
  - src/ui/render.ts
tests:
  - tests/e2e/init-smoke.test.ts
  - tests/infra/fs/file-ops.test.ts
  - tests/commands/goal.test.ts
  - tests/cli.test.ts
  - tests/commands/query.test.ts
  - tests/commands/check.test.ts
  - tests/core/wiki/date.test.ts
  - tests/core/wiki/page-merge.test.ts
  - tests/core/wiki/stale-detect.test.ts
  - tests/infra/cli-detect.test.ts
  - tests/ui/emoji-mode.test.ts
  - tests/core/wiki/frontmatter-repair.test.ts
  - tests/infra/git/source-version.test.ts
  - tests/commands/init.test.ts
  - tests/infra/global-config.test.ts
  - tests/ui/stream-parser.test.ts
  - tests/core/vault/sanity-check.test.ts
  - tests/core/wiki/lint.test.ts
  - tests/infra/git/nested-repo.test.ts
  - tests/infra/fs/raw-sync.test.ts
  - tests/core/vault/layout.test.ts
  - tests/core/vault/lock.test.ts
  - tests/infra/llm/claude-cli.test.ts
  - tests/schema/claude-md.test.ts
  - tests/core/wiki/frontmatter.test.ts
  - tests/ui/render.test.ts
-->

---
### Requirement: Resolve emoji mode via 5-level priority

The system SHALL resolve the effective emoji setting by checking, in order: explicit CLI enum (`--emoji on|off|auto`), `--no-emoji` sugar (equivalent to `--emoji off`), `NO_EMOJI` env var, `~/.codebus/config.yaml emoji:` field, and finally automatic detection. Auto-detection SHALL enable emoji only when stdout is a TTY, `CI` env is unset, `NO_EMOJI` env is unset, and `TERM` is not `"dumb"`.

#### Scenario: --emoji on overrides CI auto-detect

- **WHEN** the user passes `--emoji on` while running in a CI environment
- **THEN** emoji rendering is enabled regardless of CI / TTY state

#### Scenario: --no-emoji overrides global config emoji=on

- **WHEN** `~/.codebus/config.yaml` contains `emoji: on`
- **AND** the user passes `--no-emoji`
- **THEN** emoji rendering is disabled

#### Scenario: NO_EMOJI env disables emoji when no CLI flag set

- **WHEN** no `--emoji` or `--no-emoji` flag is passed
- **AND** `NO_EMOJI=1` is set
- **THEN** emoji rendering is disabled

#### Scenario: Auto mode respects TTY and CI signals

- **WHEN** no flag, env, or config setting is present, and the resolved mode is `"auto"`
- **THEN** emoji is enabled when `process.stdout.isTTY` is truthy AND `CI` is unset AND `TERM !== "dumb"`, and disabled otherwise


<!-- @trace
source: codebus-v2-phase1
updated: 2026-05-04
code:
  - src/infra/fs/raw-sync.ts
  - docs/superpowers/REVIEW_LESSONS.md
  - src/infra/cli-detect.ts
  - src/core/wiki/types.ts
  - README.md
  - src/core/wiki/frontmatter.ts
  - package.json
  - src/core/vault/lock.ts
  - src/core/vault/sanity-check.ts
  - src/core/wiki/stale-detect.ts
  - src/schema/claude-md.ts
  - src/commands/goal.ts
  - src/core/wiki/date.ts
  - LICENSE
  - src/cli.ts
  - tsconfig.json
  - src/commands/query.ts
  - src/infra/git/source-version.ts
  - src/ui/lint-report.ts
  - docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md
  - src/infra/llm/types.ts
  - .spectra.yaml
  - src/core/vault/layout.ts
  - src/infra/git/nested-repo.ts
  - src/commands/check.ts
  - src/core/wiki/page-merge.ts
  - vitest.config.ts
  - src/commands/init.ts
  - src/ui/stream-parser.ts
  - src/core/wiki/lint.ts
  - src/infra/llm/claude-cli.ts
  - src/infra/fs/file-ops.ts
  - src/ui/emoji-mode.ts
  - src/infra/global-config.ts
  - src/core/wiki/frontmatter-repair.ts
  - src/ui/render.ts
tests:
  - tests/e2e/init-smoke.test.ts
  - tests/infra/fs/file-ops.test.ts
  - tests/commands/goal.test.ts
  - tests/cli.test.ts
  - tests/commands/query.test.ts
  - tests/commands/check.test.ts
  - tests/core/wiki/date.test.ts
  - tests/core/wiki/page-merge.test.ts
  - tests/core/wiki/stale-detect.test.ts
  - tests/infra/cli-detect.test.ts
  - tests/ui/emoji-mode.test.ts
  - tests/core/wiki/frontmatter-repair.test.ts
  - tests/infra/git/source-version.test.ts
  - tests/commands/init.test.ts
  - tests/infra/global-config.test.ts
  - tests/ui/stream-parser.test.ts
  - tests/core/vault/sanity-check.test.ts
  - tests/core/wiki/lint.test.ts
  - tests/infra/git/nested-repo.test.ts
  - tests/infra/fs/raw-sync.test.ts
  - tests/core/vault/layout.test.ts
  - tests/core/vault/lock.test.ts
  - tests/infra/llm/claude-cli.test.ts
  - tests/schema/claude-md.test.ts
  - tests/core/wiki/frontmatter.test.ts
  - tests/ui/render.test.ts
-->

---
### Requirement: Apply chalk color when stdout is a TTY

The system SHALL apply ANSI color via chalk only when stdout is a TTY and `NO_COLOR` env is unset; otherwise output SHALL be plain text without escape codes.

#### Scenario: Non-TTY stdout produces plain text

- **WHEN** the system renders any event with stdout redirected to a file or pipe
- **THEN** the output contains no ANSI color escape codes

#### Scenario: NO_COLOR env disables color even on TTY

- **WHEN** stdout is a TTY but `NO_COLOR` is set
- **THEN** the output contains no ANSI color escape codes


<!-- @trace
source: codebus-v2-phase1
updated: 2026-05-04
code:
  - src/infra/fs/raw-sync.ts
  - docs/superpowers/REVIEW_LESSONS.md
  - src/infra/cli-detect.ts
  - src/core/wiki/types.ts
  - README.md
  - src/core/wiki/frontmatter.ts
  - package.json
  - src/core/vault/lock.ts
  - src/core/vault/sanity-check.ts
  - src/core/wiki/stale-detect.ts
  - src/schema/claude-md.ts
  - src/commands/goal.ts
  - src/core/wiki/date.ts
  - LICENSE
  - src/cli.ts
  - tsconfig.json
  - src/commands/query.ts
  - src/infra/git/source-version.ts
  - src/ui/lint-report.ts
  - docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md
  - src/infra/llm/types.ts
  - .spectra.yaml
  - src/core/vault/layout.ts
  - src/infra/git/nested-repo.ts
  - src/commands/check.ts
  - src/core/wiki/page-merge.ts
  - vitest.config.ts
  - src/commands/init.ts
  - src/ui/stream-parser.ts
  - src/core/wiki/lint.ts
  - src/infra/llm/claude-cli.ts
  - src/infra/fs/file-ops.ts
  - src/ui/emoji-mode.ts
  - src/infra/global-config.ts
  - src/core/wiki/frontmatter-repair.ts
  - src/ui/render.ts
tests:
  - tests/e2e/init-smoke.test.ts
  - tests/infra/fs/file-ops.test.ts
  - tests/commands/goal.test.ts
  - tests/cli.test.ts
  - tests/commands/query.test.ts
  - tests/commands/check.test.ts
  - tests/core/wiki/date.test.ts
  - tests/core/wiki/page-merge.test.ts
  - tests/core/wiki/stale-detect.test.ts
  - tests/infra/cli-detect.test.ts
  - tests/ui/emoji-mode.test.ts
  - tests/core/wiki/frontmatter-repair.test.ts
  - tests/infra/git/source-version.test.ts
  - tests/commands/init.test.ts
  - tests/infra/global-config.test.ts
  - tests/ui/stream-parser.test.ts
  - tests/core/vault/sanity-check.test.ts
  - tests/core/wiki/lint.test.ts
  - tests/infra/git/nested-repo.test.ts
  - tests/infra/fs/raw-sync.test.ts
  - tests/core/vault/layout.test.ts
  - tests/core/vault/lock.test.ts
  - tests/infra/llm/claude-cli.test.ts
  - tests/schema/claude-md.test.ts
  - tests/core/wiki/frontmatter.test.ts
  - tests/ui/render.test.ts
-->

---
### Requirement: Load global config tolerantly

The system SHALL load `~/.codebus/config.yaml` if present, ignore unknown fields silently (forward-compat for future schema growth), warn but not abort on parse errors, and warn on unknown values for known fields.

The config schema SHALL recognize the following top-level keys, each optional and tolerantly parsed:

- `emoji`: emoji mode preference (one of `auto` | `on` | `off`)
- `llm`: LLM provider configuration block (provider kind plus provider-specific sub-fields)
- `pii`: PII scanner configuration block (scanner kind, on-hit behavior, extra patterns)
- `lint`: lint rule configuration block (rule overrides, disabled rules, custom rules path)
- `render`: output renderer configuration block (renderer format)
- `log`: log sink configuration block (sink kind, retention)

For each of the five plugin section keys (`llm`, `pii`, `lint`, `render`, `log`), the loader SHALL:

- Parse the section if present and value is a YAML mapping; produce an empty section if absent or null
- Within each section, recognize provider/scanner/rule/renderer/sink kind via a `provider` / `scanner` / `format` / `sink` discriminator field as appropriate to the section
- Silently ignore sub-fields under a section that the loader does not recognize (forward-compat for future plugin additions)
- Warn but not abort when a discriminator field has an unknown value, treating that section as unset (factory falls through to default)
- Warn but not abort when a sub-field has a type-incompatible value (e.g., `timeout_secs: "thirty"` where number expected), treating that sub-field as unset

#### Scenario: Missing config returns empty config

- **WHEN** `~/.codebus/config.yaml` does not exist
- **THEN** the system returns an empty config object without error

#### Scenario: Invalid YAML warns and falls back to empty

- **WHEN** `~/.codebus/config.yaml` contains malformed YAML
- **THEN** the system writes a warning to stderr and returns an empty config object

#### Scenario: Unknown emoji value warns and is ignored

- **WHEN** `~/.codebus/config.yaml` contains `emoji: maybe`
- **THEN** the system writes a warning, and `emoji` is treated as unset (falling through to next priority level)

#### Scenario: Future top-level fields are silently ignored

- **WHEN** `~/.codebus/config.yaml` contains a top-level field not in the recognized set (`emoji`, `llm`, `pii`, `lint`, `render`, `log`)
- **THEN** the system silently ignores the field without warning

#### Scenario: LLM section selects provider via discriminator

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: claude_cli, binary_path: /usr/local/bin/claude }`
- **THEN** the loader returns an LLM config with provider kind `claude_cli` and the `binary_path` sub-field populated

#### Scenario: Unknown LLM provider warns and is treated as unset

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: gibberish, api_key: x }`
- **THEN** the system writes a warning identifying the unknown provider, and the LLM section is treated as unset (factory uses default `claude_cli` provider)

#### Scenario: Unknown sub-field within a known section is silently ignored

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: claude_cli, future_unknown_field: 1 }`
- **THEN** the loader honors `provider: claude_cli` and silently ignores `future_unknown_field`

#### Scenario: PII section selects scanner via discriminator

- **WHEN** `~/.codebus/config.yaml` contains `pii: { scanner: regex_basic, on_hit: warn, patterns_extra: ["INTERNAL-\\d{6}"] }`
- **THEN** the loader returns a PII config with scanner kind `regex_basic`, on-hit `warn`, and one extra pattern

#### Scenario: Lint section overrides recognized

- **WHEN** `~/.codebus/config.yaml` contains `lint: { disabled_rules: [oversize-page] }`
- **THEN** the loader returns a lint config with the rule `oversize-page` listed in disabled rules

#### Scenario: Render section selects renderer

- **WHEN** `~/.codebus/config.yaml` contains `render: { format: terminal }`
- **THEN** the loader returns a render config with format `terminal`

#### Scenario: Log section selects sink

- **WHEN** `~/.codebus/config.yaml` contains `log: { sink: jsonl, retention_days: 30 }`
- **THEN** the loader returns a log config with sink kind `jsonl` and retention 30 days

#### Scenario: Empty plugin section parses as defaults

- **WHEN** `~/.codebus/config.yaml` contains `pii: {}`
- **THEN** the loader returns a PII config with all fields at their defaults (scanner unset, factory uses `null` scanner)

#### Scenario: Type-mismatched sub-field warns and is treated as unset

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: claude_cli, timeout_secs: "thirty" }` (string where number expected)
- **THEN** the system writes a warning, the `timeout_secs` sub-field is treated as unset, and the rest of the LLM section is honored

---
### Requirement: Render stage banners during goal flow

The system SHALL emit a stage banner before and after each non-LLM stage of the `--goal` flow so the user can identify which stage is currently executing. The required stages are: raw sync, PII summary, lint, each fix iteration, and auto-commit. Each "done" banner SHALL carry the elapsed milliseconds for that stage. Banner emoji vs symbol prefix SHALL follow the existing emoji-mode resolution chain — this requirement does not introduce a parallel mode-resolution path.

#### Scenario: Sync start banner before raw_sync

- **WHEN** the goal flow is about to invoke raw_sync
- **THEN** the renderer emits a `SyncStart` banner whose body identifies the sync stage

#### Scenario: Sync done banner after raw_sync

- **WHEN** raw_sync returns successfully
- **THEN** the renderer emits a `SyncDone` banner carrying the number of files copied, total mebibytes copied, and elapsed milliseconds

#### Scenario: Lint start banner before lint_wiki

- **WHEN** the goal flow is about to invoke lint_wiki
- **THEN** the renderer emits a `LintStart` banner

#### Scenario: Lint done banner after lint_wiki

- **WHEN** lint_wiki returns a result
- **THEN** the renderer emits a `LintDone` banner carrying the error count, warning count, and elapsed milliseconds

#### Scenario: Fix iteration start banner

- **WHEN** the fix loop is about to start iteration `i` out of a maximum of `max`
- **THEN** the renderer emits a `FixIterStart { i, max }` banner

#### Scenario: Fix iteration done banner

- **WHEN** fix iteration `i` returns
- **THEN** the renderer emits a `FixIterDone` banner carrying the iteration index, number of issues fixed in this iteration, number of issues remaining, and elapsed milliseconds for this iteration

#### Scenario: Commit done banner after auto_commit

- **WHEN** auto_commit succeeds with sha `abc1234567...`
- **THEN** the renderer emits a `CommitDone` banner whose body contains the short (7-char) prefix of that sha

#### Scenario: Stage banners follow existing emoji mode

- **WHEN** the renderer emits any stage banner with emoji enabled
- **THEN** the line is prefixed with the same emoji-mode glyph the existing lifecycle banners use, not a new symbol set


<!-- @trace
source: goal-stage-banners
updated: 2026-05-07
code:
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/fs/raw_sync.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/examples/bench_sync.rs
  - codebus-core/src/render/event_renderer.rs
  - codebus-core/src/render/renderers/terminal.rs
-->

---
### Requirement: Render PII summary banner

The system SHALL emit one PII summary banner after the raw sync stage completes, regardless of which scanner was selected (including `null`). The banner SHALL carry the scanner name, the count of files scanned, the count of files with PII matches, and the on-hit action that was applied. This requirement makes PII activity visible without requiring the user to enable verbose logging.

#### Scenario: PII summary with NullScanner

- **WHEN** the raw sync stage completes with the default `null` scanner over 1289 files
- **THEN** the renderer emits a `PiiSummary` banner reporting scanner `null`, scanned 1289, hits 0, action `warn`

#### Scenario: PII summary with regex_basic scanner and skip action

- **WHEN** the raw sync stage completes with `regex_basic` scanner that matched 3 files and on-hit `skip`
- **THEN** the renderer emits a `PiiSummary` banner reporting scanner `regex_basic`, scanned 1289, hits 3, action `skip`


<!-- @trace
source: goal-stage-banners
updated: 2026-05-07
code:
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/fs/raw_sync.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/examples/bench_sync.rs
  - codebus-core/src/render/event_renderer.rs
  - codebus-core/src/render/renderers/terminal.rs
-->

---
### Requirement: Stage banners do not block on stdout failures

The system SHALL treat banner rendering as best-effort. If the underlying `println!` fails (e.g., closed pipe), the goal flow SHALL continue to completion and the run result SHALL NOT be marked as failed solely because a banner could not be written. This matches the behavior already in place for the existing four lifecycle banners.

#### Scenario: Goal flow completes when stdout pipe is closed

- **WHEN** stdout is closed mid-run and a stage banner cannot be written
- **THEN** the goal flow continues, the wiki is still updated, and the process exit code reflects the goal outcome — not the banner I/O error

<!-- @trace
source: goal-stage-banners
updated: 2026-05-07
code:
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/fs/raw_sync.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/examples/bench_sync.rs
  - codebus-core/src/render/event_renderer.rs
  - codebus-core/src/render/renderers/terminal.rs
-->