## MODIFIED Requirements

### Requirement: ReasoningLogger appends one JSONL line per Step to workspace path

The sidecar SHALL implement a `codebus_agent.agent.reasoning_logger.ReasoningLogger` class whose `write(step: Step)` method appends exactly one UTF-8 encoded JSON line per call to `{workspace_root}/.codebus/reasoning_log.jsonl`. The JSON payload MUST be `step.model_dump_json()` output (Pydantic v2 canonical form) so every field of `Step` (including `thought`, `tool_calls`, `tool_results`, `judge_verdict`, `tokens_used`, `ts`) round-trips via `Step.model_validate_json`.

The path lives under the `.codebus/` subdirectory of the workspace root, consistent with the workspace-level audit chain convention shared by `<workspace>/.codebus/sanitize_audit.jsonl` / `<workspace>/.codebus/tool_audit.jsonl` / `<workspace>/.codebus/token_usage.jsonl` / `<workspace>/.codebus/llm_calls.jsonl`. ReasoningLogger's constructor MUST NOT silently create parent directories (caller-side path-safety convention preserves single-source-of-truth for path validation); callers (e.g. `api/explore.py`) MUST `mkdir(parents=True, exist_ok=True)` the `.codebus/` parent BEFORE constructing the logger, mirroring how `UsageTracker` and `LLMCallLogger` constructors auto-handle the same directory but ReasoningLogger defers to caller per its existing path-safety contract.

Every written line SHALL additionally include the `explorer_prompt_version` and `judge_prompt_version` constant strings so golden-sample replays can pin prompt revisions. The logger MUST NOT emit any SSE events in P0 (wiring deferred to the follow-up SSE change) and MUST NOT write to any path outside `workspace_root`.

Write failures (disk full, permission denied) MUST propagate as exceptions so the Explorer loop's error-handling path can log and terminate gracefully — silent drops are forbidden.

#### Scenario: Each write appends exactly one JSONL line

- **WHEN** `ReasoningLogger(path).write(step)` is called K times in sequence
- **THEN** the file at `path` MUST contain exactly K lines, each terminated by `\n`, and each line MUST parse as a `Step` via `Step.model_validate_json`

#### Scenario: Prompt version columns are present on every line

- **WHEN** any line is parsed out of `<workspace>/.codebus/reasoning_log.jsonl`
- **THEN** the JSON object MUST contain string fields `explorer_prompt_version` and `judge_prompt_version` matching the module-level constants at write time

#### Scenario: Path stays under workspace and caller mkdirs .codebus parent

- **WHEN** `ReasoningLogger(path)` is constructed where `path` resolves above `workspace_root`
- **THEN** the caller's integration site (e.g., `run_explorer`'s setup in `api/explore.py`) MUST have rejected the path via `ensure_in_workspace` before construction; `ReasoningLogger` itself MAY rely on that precondition and perform no additional path check, but it MUST NOT silently create parent directories outside the workspace
- **AND** the caller MUST also `mkdir(parents=True, exist_ok=True)` the `<workspace_root>/.codebus/` parent directory BEFORE constructing `ReasoningLogger(<workspace_root> / ".codebus" / "reasoning_log.jsonl")`, because ReasoningLogger's constructor does not auto-mkdir (asymmetry vs. `UsageTracker` / `LLMCallLogger` is intentional — caller-side mkdir keeps path-safety logic in one place per the spec's existing precondition)

#### Scenario: Logger is the single source of truth for prompt version stamping

- **WHEN** any caller of `ReasoningLogger.write(step)` constructs a `Step` whose `explorer_prompt_version` / `judge_prompt_version` fields are set to caller-supplied values (including the current module-level constants)
- **THEN** `ReasoningLogger.write` MUST overwrite those two fields via `model_copy(update={...})` with the LIVE module-level `EXPLORER_PROMPT_VERSION` / `JUDGE_PROMPT_VERSION` constants before writing, so the on-disk JSONL line carries the version values at write time regardless of caller-supplied values
- **AND** callers (including `run_explorer`'s coverage-gap Step path) MUST NOT rely on pre-stamping the Step — the logger is the single source of truth for prompt-version stamping
- **AND** the rationale is: any future prompt-version bump becomes a single-point edit (`prompts/explorer.py` / `prompts/judge.py` constants); double-stamping creates drift risk where the constant moves but a caller's stale literal value lands in the audit log
