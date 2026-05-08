# wiki-query Specification

## Purpose

`--query` flow — read-only path that lets the LLM agent answer questions by reading existing wiki pages and citing them via `[[wikilink]]`. The agent runs with `Write` and `Edit` removed from the toolset, so the vault is never mutated during a query (filing-back is deferred to phase 1.5).

## Requirements

### Requirement: Run query flow on --query invocation

When invoked with `--repo <path> --query "<text>"`, the system SHALL run a read-only flow that lets the agent read existing wiki pages and produce an answer with citations, without writing any files or modifying the vault.

After the query stream completes (success or failure), the system SHALL build a `RunLog` carrying `mode: "query"`, the configured `model` and `effort` (if any), accumulated `tokens` from every `StreamEvent::Usage` observed during the run, `started_at` / `finished_at` UTC timestamps, `wiki_changed: false` (query never mutates), `lint_error_count: 0`, `lint_warn_count: 0`, and call `log_sink.write_run(&run_log)` exactly once. The default sink (`SinkConfig::Jsonl { dir: None }`) writes the entry to `<repo>/.codebus/logs/runs-YYYY-MM-DD.jsonl` automatically. Users opt out of run logging by setting `log: { sink: null }` in `~/.codebus/config.yaml`.

#### Scenario: Query with non-empty wiki succeeds

- **WHEN** the user runs `codebus --repo X --query "how does checkout work?"` and at least one of `.codebus/wiki/{concepts,entities,modules,processes,synthesis}/` contains a `.md` file
- **THEN** the system spawns the LLM agent in query mode and streams the agent's reasoning and answer to the terminal

#### Scenario: Query flow writes a single RunLog after success

- **WHEN** a `--query` run completes successfully
- **THEN** the system calls `log_sink.write_run(&run_log)` exactly once with `mode: "query"`, `tokens` reflecting the single LLM invocation, and `wiki_changed: false`

#### Scenario: Query flow writes a RunLog even on failure

- **WHEN** a `--query` run errors mid-stream after at least one `StreamEvent::Usage` was observed
- **THEN** the system still calls `log_sink.write_run(&run_log)` exactly once with the partial token counts; the run still surfaces the error via the existing exit-code path

#### Scenario: Default sink writes RunLog to vault-local logs directory

- **WHEN** the user has no `log:` section in `~/.codebus/config.yaml` (the default) and runs `--query` against repo `X`
- **THEN** the system writes the `RunLog` as one JSON line to `X/.codebus/logs/runs-YYYY-MM-DD.jsonl`

#### Scenario: Explicit null sink discards the RunLog write silently

- **WHEN** the user sets `log: { sink: null }` in `~/.codebus/config.yaml` and runs `--query`
- **THEN** the system still constructs the `RunLog` and calls `log_sink.write_run`, but `NullSink::write_run` returns `Ok(())` without producing any file output (the explicit opt-out)


<!-- @trace
source: token-tracking
updated: 2026-05-07
code:
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/render/renderers/terminal.rs
  - codebus-cli/src/commands/query.rs
  - codebus-core/src/log/sinks/null_sink.rs
  - codebus-core/src/stream/parser.rs
  - codebus-core/src/log/mod.rs
  - codebus-core/src/config/loader.rs
  - codebus-cli/src/main.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/log/factory.rs
  - codebus-core/src/log/sinks/jsonl_sink.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/log/sink.rs
  - codebus-core/src/wiki/fix/mod.rs
-->

---
### Requirement: Reject query when wiki is empty

The system SHALL fail fast with a user-facing error pointing to `--goal` when none of the 5 type folders under `.codebus/wiki/` contain any `.md` file.

#### Scenario: All type folders empty or missing aborts with hint

- **WHEN** the user runs `codebus --repo X --query "..."` and none of `.codebus/wiki/{concepts,entities,modules,processes,synthesis}/` contains a `.md` file (folders may be missing or present-but-empty)
- **THEN** the system throws an error whose message instructs the user to run `--goal` first


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
### Requirement: Spawn agent in query mode with Write/Edit excluded from toolset

The system SHALL spawn the LLM provider with cwd = `.codebus/` (same isolation as ingest) and SHALL omit `Write` and `Edit` from the `--tools` toolset whitelist so the agent cannot write files even within the vault. (See §3.2.4 of the design spec for why `--tools` is the toolset gate, not `--allowedTools`.)

When the resolved `ProviderConfig::ClaudeCli` carries a non-empty `model` value, the system SHALL append `--model <value>` to the spawned argv. When the resolved `ProviderConfig::ClaudeCli` carries a non-empty `effort` value, the system SHALL append `--effort <value>` to the spawned argv. When either field is `None`, the corresponding flag SHALL NOT appear in argv.

The system SHALL NOT pass any of the following sandbox-breaking flags under any combination of mode, model, or effort: `--add-dir`, `--allow-dangerously-skip-permissions`, `--dangerously-skip-permissions`.

#### Scenario: Required argv flags are present in query mode

- **WHEN** the system builds argv for query mode
- **THEN** argv contains `-p`, `--output-format stream-json`, `--input-format stream-json`, `--verbose`, `--permission-mode acceptEdits`, `--tools Read,Glob,Grep`, and `--allowedTools Read,Glob,Grep` (auto-approval safety net mirroring `--tools`)

#### Scenario: Provider spawn cwd matches vault root

- **WHEN** the system invokes the LLM provider for query mode against repo X
- **THEN** the spawn cwd equals `<X>/.codebus/`

#### Scenario: Model flag is injected in query mode when ClaudeCli config sets model

- **WHEN** the system builds argv for query mode with `ProviderConfig::ClaudeCli { model: Some("haiku"), ... }`
- **THEN** argv contains `--model haiku`

#### Scenario: Effort flag is injected in query mode when ClaudeCli config sets effort

- **WHEN** the system builds argv for query mode with `ProviderConfig::ClaudeCli { effort: Some("low"), ... }`
- **THEN** argv contains `--effort low`

#### Scenario: Model and effort flags are absent in query mode when config leaves them None

- **WHEN** the system builds argv for query mode with `ProviderConfig::ClaudeCli { model: None, effort: None, ... }`
- **THEN** argv contains neither `--model` nor `--effort`

#### Scenario: Forbidden sandbox-breaking flags never appear in query argv

- **WHEN** the system builds argv for query mode under any combination of `model` and `effort` values (set or unset)
- **THEN** argv contains none of `--add-dir`, `--allow-dangerously-skip-permissions`, `--dangerously-skip-permissions`


<!-- @trace
source: llm-claude-cli-params
updated: 2026-05-07
code:
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-core/src/llm/providers/claude_cli.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/llm/factory.rs
  - codebus-core/src/llm/provider.rs
  - codebus-core/src/config/loader.rs
  - codebus-cli/src/commands/query.rs
-->

---
### Requirement: Compose system prompt from schema and wiki index

The system SHALL build the agent's system prompt by concatenating the built-in schema, the current `wiki/index.md` content (or `(empty)` placeholder), and a query-mode instruction directing the agent to cite via `[[wikilink]]` and not write any files.

#### Scenario: System prompt includes schema and index

- **WHEN** query runs and `.codebus/CLAUDE.md` and `.codebus/wiki/index.md` both exist
- **THEN** the agent's system prompt contains the schema content followed by the index content followed by the query-mode instruction

#### Scenario: Missing index falls back to placeholder

- **WHEN** `.codebus/wiki/index.md` does not exist
- **THEN** the system uses `(empty)` as the index portion of the prompt


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
### Requirement: Query flow does not mutate the vault

The system SHALL NOT sync raw, append to `goals.jsonl`, run stale-detect, or commit to the nested git repo during query execution.

#### Scenario: Query leaves goals.jsonl unchanged

- **WHEN** query runs
- **THEN** `.codebus/goals.jsonl` content and modification time are unchanged

#### Scenario: Query leaves nested git unchanged

- **WHEN** query runs
- **THEN** `.codebus/.git` HEAD commit is unchanged

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