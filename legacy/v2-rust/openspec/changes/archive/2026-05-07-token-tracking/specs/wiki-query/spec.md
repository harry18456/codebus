## MODIFIED Requirements

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
