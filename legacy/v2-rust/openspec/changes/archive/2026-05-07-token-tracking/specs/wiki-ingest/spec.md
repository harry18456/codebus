## MODIFIED Requirements

### Requirement: Run ingest flow on --goal invocation

When invoked with `--repo <path> --goal "<text>"`, the system SHALL run the full ingest sequence: ensure vault exists, sync raw code, record source version, invoke the LLM agent in ingest mode, post-process pages, and commit the result to the nested git repo.

After the ingest sequence completes (success or failure), the system SHALL build a `RunLog` carrying `mode: "goal"`, the configured `model` and `effort` (if any), accumulated `tokens` from every `StreamEvent::Usage` observed during the run (including all fix-loop iterations when auto-fix is enabled), `started_at` / `finished_at` UTC timestamps, `wiki_changed`, `lint_error_count`, `lint_warn_count`, and call `log_sink.write_run(&run_log)` exactly once. The default sink (`SinkConfig::Jsonl { dir: None }`) writes the entry to `<repo>/.codebus/logs/runs-YYYY-MM-DD.jsonl` automatically — same per-vault auto-tracking precedent as `goals.jsonl`. Users opt out of run logging by setting `log: { sink: null }` in `~/.codebus/config.yaml`.

#### Scenario: First-time goal triggers init then ingest

- **WHEN** the user runs `codebus --repo X --goal "understand checkout"` and `.codebus/` does not exist
- **THEN** the system initializes the vault, then proceeds with the full ingest flow

#### Scenario: Goal on existing vault skips init step

- **WHEN** `.codebus/` already exists
- **AND** the user runs `codebus --repo X --goal "understand checkout"`
- **THEN** the system skips init and proceeds directly with sync + agent invocation

#### Scenario: Goal flow writes a single RunLog after success

- **WHEN** a `--goal` run completes successfully (with or without fix loop)
- **THEN** the system calls `log_sink.write_run(&run_log)` exactly once with `mode: "goal"`, `tokens` summing every `StreamEvent::Usage` observed during the run (including fix-loop iterations), and `wiki_changed` reflecting the post-commit state

#### Scenario: Goal flow writes a RunLog even on failure

- **WHEN** a `--goal` run errors mid-stream after at least one `StreamEvent::Usage` was observed
- **THEN** the system still calls `log_sink.write_run(&run_log)` exactly once with the partial token counts and `wiki_changed: false`; the run still surfaces the error to the user via the existing exit-code path

#### Scenario: Default sink writes RunLog to vault-local logs directory

- **WHEN** the user has no `log:` section in `~/.codebus/config.yaml` (the default) and runs `--goal` against repo `X`
- **THEN** the system writes the `RunLog` as one JSON line to `X/.codebus/logs/runs-YYYY-MM-DD.jsonl` where `YYYY-MM-DD` is the UTC date of the run's `started_at`

#### Scenario: Explicit null sink discards the RunLog write silently

- **WHEN** the user sets `log: { sink: null }` in `~/.codebus/config.yaml` and runs `--goal`
- **THEN** the system still constructs the `RunLog` and calls `log_sink.write_run`, but `NullSink::write_run` returns `Ok(())` without producing any file output (the explicit opt-out)
