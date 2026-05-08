# token-tracking Specification

## Purpose

TBD - created by archiving change 'token-tracking'. Update Purpose after archive.

## Requirements

### Requirement: Normalized TokenUsage schema

The system SHALL define a single `TokenUsage` struct that all LLM providers use to report per-invocation token counts. The struct SHALL contain:

- `input_tokens: u64` — required. Tokens fed into the model as prompt content. Every LLM API exposes this concept.
- `output_tokens: u64` — required. Tokens produced by the model. Every LLM API exposes this concept.
- `cache_read_tokens: Option<u64>` — optional. `None` SHALL mean "the provider has no notion of prompt caching" (e.g., OpenAI legacy, Ollama). `Some(n)` SHALL mean cached tokens read from the provider's cache during this invocation.
- `cache_write_tokens: Option<u64>` — optional. Same `None` semantic. `Some(n)` SHALL mean tokens written into the provider's cache during this invocation.
- `reasoning_tokens: Option<u64>` — optional. `None` for providers without separate reasoning accounting; `Some(n)` for o-series-style or extended-thinking-style providers that bill reasoning separately.
- `extras: serde_json::Value` — vendor-specific raw JSON escape hatch. Providers SHALL place the original wire-format `usage` object here so downstream tools can recover full fidelity when the normalized fields lose detail.

The struct SHALL serialize via serde with `Option<u64>` fields skipped when `None` (`#[serde(skip_serializing_if = "Option::is_none")]`) so jsonl entries do not carry empty cache fields for providers that do not support them.

#### Scenario: Claude CLI invocation populates all four anthropic fields

- **WHEN** a `claude -p` invocation completes and the stream-json `result` event reports `usage: { input_tokens: 1234, output_tokens: 567, cache_creation_input_tokens: 100, cache_read_input_tokens: 8900 }`
- **THEN** the resulting `TokenUsage` has `input_tokens: 1234`, `output_tokens: 567`, `cache_read_tokens: Some(8900)`, `cache_write_tokens: Some(100)`, `reasoning_tokens: None`, and `extras` containing the full original `usage` object as a `serde_json::Value`

#### Scenario: Provider without cache concept produces None for cache fields

- **WHEN** a future provider (placeholder example: a plain text-completion endpoint with no caching) reports only `prompt_tokens` and `completion_tokens` and emits a `TokenUsage`
- **THEN** the `TokenUsage` has the input and output counts mapped, `cache_read_tokens: None`, `cache_write_tokens: None`, `reasoning_tokens: None`, and `extras` carrying the provider's own usage payload

#### Scenario: TokenUsage with all None cache fields serializes without those keys

- **WHEN** the system serializes `TokenUsage { input_tokens: 100, output_tokens: 50, cache_read_tokens: None, cache_write_tokens: None, reasoning_tokens: None, extras: serde_json::Value::Null }` to JSON
- **THEN** the resulting JSON object contains exactly `input_tokens`, `output_tokens`, and `extras` keys; `cache_read_tokens` / `cache_write_tokens` / `reasoning_tokens` are absent (skipped by serde)


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
### Requirement: Provider stream parser emits StreamEvent::Usage

The system SHALL expose a `StreamEvent::Usage(TokenUsage)` variant. Each `LlmProvider` implementation's stream parser SHALL emit at most one `StreamEvent::Usage` per invocation, immediately before `StreamEvent::Done`, carrying the normalized `TokenUsage` for that invocation. Providers that cannot report usage (e.g., a stub provider used in tests) SHALL omit the event.

The variant placement is provider-internal: callers receive a uniform stream and need not know which provider produced the usage data. This keeps the consumer-side accumulation logic provider-agnostic and lets future provider implementations (Anthropic API direct / OpenAI / Ollama) plug in without touching consumer code.

#### Scenario: Claude CLI provider emits Usage before Done

- **WHEN** `ClaudeCliProvider` parses a stream-json line containing `{"type": "result", "result": {...}, "usage": {...}}`
- **THEN** the parser yields a `StreamEvent::Usage(token_usage)` followed by `StreamEvent::Done`

#### Scenario: Stream without usage data omits Usage event

- **WHEN** a provider stream completes without a `usage` payload (e.g., a mock provider in tests)
- **THEN** the stream contains no `StreamEvent::Usage` and the consumer SHALL treat the run as having zero usage (default `TokenUsage`)


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
### Requirement: RunLog carries mode model effort fields

The system SHALL extend `RunLog` with three optional metadata fields so jsonl entries are self-describing:

- `mode: String` — `"goal"` or `"query"`. Required because runs.jsonl mixes both modes.
- `model: Option<String>` — the model alias or full model name passed to the provider for this run. `None` when no model was configured (provider used its default).
- `effort: Option<String>` — the reasoning effort level passed to the provider. `None` when no effort was configured.

These fields SHALL be populated by the run flow (goal / query) from the `InvokeOptions` actually used.

#### Scenario: Goal run with claude_cli + haiku writes mode goal model haiku

- **WHEN** the user runs `codebus --goal "explain X"` against a config with `provider: claude_cli, model: haiku`
- **THEN** the resulting `RunLog` has `mode: "goal"`, `model: Some("haiku")`, `effort: None`

#### Scenario: Query run without explicit model defaults to None

- **WHEN** the user runs `codebus --query "what is X?"` against a config that has no `model` field set under `llm.claude_cli`
- **THEN** the resulting `RunLog` has `mode: "query"`, `model: None`, `effort: None`


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
### Requirement: Goal and query flows accumulate Usage events into RunLog

The system SHALL accumulate every `StreamEvent::Usage` event observed during a run into a single `TokenUsage` value placed in the resulting `RunLog`. When the run encompasses multiple LLM invocations (e.g., goal flow with auto-fix loop), the final `RunLog.tokens` SHALL be the sum of every Usage event observed across all invocations within that run.

After the run completes (success or failure), the system SHALL call `log_sink.write_run(&run_log)` exactly once. The default sink (`SinkConfig::Null {}`) SHALL silently discard the write so 0.x-era users see no behavioral change.

#### Scenario: Goal flow with no fix loop writes one RunLog containing one invocation's tokens

- **WHEN** a `--goal` run with `--no-fix` produces a single LLM invocation whose Usage event reports input 100 / output 50
- **THEN** the `RunLog` written to the sink has `tokens.input_tokens == 100` and `tokens.output_tokens == 50`

#### Scenario: Goal flow with fix loop sums tokens across iterations

- **WHEN** a `--goal` run completes one ingest invocation (input 100 / output 50) followed by two fix-loop iterations (each input 80 / output 30)
- **THEN** the single `RunLog` written to the sink has `tokens.input_tokens == 260` (100 + 80 + 80) and `tokens.output_tokens == 110` (50 + 30 + 30)

#### Scenario: Run failure still writes RunLog

- **WHEN** a `--goal` invocation errors mid-stream after one Usage event (input 50 / output 20) was observed
- **THEN** the system writes a `RunLog` with the partial token counts and `wiki_changed: false`; the run still surfaces the error to the user via the existing exit-code path


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
### Requirement: Default jsonl directory falls back to vault-local logs folder

When `SinkConfig::Jsonl { dir: None }` is resolved (because the user wrote `log: { sink: jsonl }` without specifying `dir`), the system SHALL fall back to `<repo>/.codebus/logs/` as the directory for run logs. When `SinkConfig::Jsonl { dir: Some(p) }` is resolved, the system SHALL use `p` as written.

The vault-local default keeps logs adjacent to the wiki, raw mirror, and goals.jsonl — the same `.codebus/` boundary that already isolates per-project codebus state from the user's source repo.

#### Scenario: Jsonl sink without dir defaults to vault-local logs

- **WHEN** `~/.codebus/config.yaml` contains `log: { sink: jsonl }` and the user runs `codebus --repo /home/u/myproj --goal "..."`
- **THEN** run log entries are written under `/home/u/myproj/.codebus/logs/`

#### Scenario: Jsonl sink with explicit dir uses it verbatim

- **WHEN** `~/.codebus/config.yaml` contains `log: { sink: jsonl, dir: /var/log/codebus }` and the user runs codebus
- **THEN** run log entries are written under `/var/log/codebus/`


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
### Requirement: Jsonl files rotate by UTC date

The system SHALL name jsonl files `runs-YYYY-MM-DD.jsonl` where `YYYY-MM-DD` is the UTC date at the moment of write. A new file SHALL be created when the first run of a UTC day occurs. Existing entries on the same UTC date SHALL be appended to the same file.

UTC is used (rather than local time) so that the file name is stable across timezone-shifting machines and so cross-machine jsonl analysis never has to reason about local-tz collation.

#### Scenario: First run of a UTC day creates a new file

- **WHEN** the system writes a `RunLog` whose `started_at` is `2026-05-07T23:30:00Z` and the directory contains no `runs-2026-05-07.jsonl`
- **THEN** a new file `runs-2026-05-07.jsonl` is created with one line

#### Scenario: Subsequent run on same UTC date appends to existing file

- **WHEN** a second `RunLog` is written 30 minutes later (`2026-05-08T00:00:00Z` is UTC the next day; this scenario uses two events at `2026-05-07T23:30:00Z` and `2026-05-07T23:55:00Z`)
- **THEN** both lines are in the same `runs-2026-05-07.jsonl` file

#### Scenario: Run crossing UTC midnight writes to file matching started_at

- **WHEN** a `RunLog` has `started_at: 2026-05-07T23:55:00Z` and `finished_at: 2026-05-08T00:10:00Z`
- **THEN** the entry is written to `runs-2026-05-07.jsonl` (file is selected by `started_at` UTC date, not `finished_at`)


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
### Requirement: Logs directory is excluded from nested vault git

The system SHALL ensure that `<repo>/.codebus/logs/` is excluded from the nested vault git repository so that `codebus` runs do not produce a stream of "modified" or "untracked" file noise in `git status` for the nested vault. Implementation may use `<repo>/.codebus/.gitignore` listing `logs/`, or `<repo>/.codebus/.git/info/exclude` listing `logs/`.

#### Scenario: After init the logs directory is gitignored

- **WHEN** the system runs `codebus --repo X` for the first time (vault init) and a subsequent goal writes a run log
- **THEN** `git -C X/.codebus status --porcelain` does not include any path under `logs/`

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