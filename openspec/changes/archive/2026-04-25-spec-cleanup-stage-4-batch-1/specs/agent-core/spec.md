## MODIFIED Requirements

### Requirement: ReAct loop executes think-act-observe-judge-log-update each iteration

The sidecar SHALL implement an async `run_explorer(state, provider, tools, judge, coverage, logger)` function in `codebus_agent.agent.explorer` that drives Explorer Agent execution through a ReAct control flow. Each iteration MUST perform the six sub-steps in order: (1) **Think** — invoke `provider.chat(messages, response_model=ExplorerAction)` to produce an `ExplorerAction`; (2) **Act** — dispatch each `tool_calls[*]` through the `ExplorerTools` Protocol implementation and collect `ToolResult`s (empty list when the action contains no tool calls); (3) **Observe** — append Tool results as `Message(role="tool", ...)` onto `state.messages` so the next iteration sees them; (4) **Judge** — call `judge.evaluate(state, results)` to produce a `JudgeVerdict`; (5) **Log** — build a `Step` aggregating thought + tool_calls + tool_results + verdict and pass it to `logger.write(step)`; (6) **Update state** — advance `state.step_count`, decrement `state.budget_steps_left`, and fold the verdict into `state.stations` / `state.visited_files` / `state.pending_queue` per the orchestrator's update rules.

The loop terminates via the `_should_stop(state)` predicate (see separate Requirement). On termination, `run_explorer` MUST invoke `coverage.check(state)` exactly once and MAY recurse into a new `run_explorer` call per the rules defined in Requirement `Coverage-gap recursion runs after main loop convergence`. When recursion does not fire (empty gaps / budget exhausted / max depth reached), `run_explorer` returns an `ExplorerResult` containing the accumulated `state.stations`, the reasoning-log path, and a `stopped_reason` string classifying which convergence branch fired in the innermost iteration.

#### Scenario: Each iteration writes exactly one Step line

- **WHEN** `run_explorer` runs with a budget that permits N iterations and no coverage recursion fires
- **THEN** `logger.write(step)` MUST be invoked exactly N times for regular iterations, each with a `Step.step` value equal to the iteration index (0-based)
- **AND** every iteration `Step` written MUST contain a non-None `judge_verdict` (Judge ran in the same iteration)

#### Scenario: Observations feed forward into next Think call

- **WHEN** iteration K emits tool calls that return results `R1, R2`
- **THEN** iteration K+1's `_think` invocation MUST include messages with `role="tool"` whose `content` reflects `R1.output` / `R2.output` so the LLM can react to observations

#### Scenario: Tool errors do not crash the loop

- **WHEN** a tool invocation inside `_execute_tools` raises any exception
- **THEN** the exception MUST be captured into the corresponding `ToolResult.error` field (and `ToolResult.output` MUST hold a sanitized error string), the loop MUST continue to the Judge / Log / Update sub-steps, and the `Step` line written for that iteration MUST record the failed `ToolResult` verbatim

#### Scenario: Coverage recursion hook activates after main loop convergence

- **WHEN** `_should_stop` returns true and `run_explorer` is about to return
- **THEN** `coverage.check(state)` MUST be invoked exactly once before return
- **AND** when `coverage.check` returns one or more `Gap` entries and the recursion preconditions defined in Requirement `Coverage-gap recursion runs after main loop convergence` are met, `run_explorer` MUST recurse into a new call with `_depth + 1`
- **AND** when the preconditions are not met (empty gaps / budget exhausted / `_depth` has reached `_COVERAGE_MAX_DEPTH`), `run_explorer` MUST return without recursing

#### Scenario: Update step uses tool_name as P0 pending_queue placeholder

- **WHEN** an iteration's `JudgeVerdict` carries `should_follow_imports=True` and the iteration produced one or more non-error `ToolResult` entries
- **THEN** the Update step MUST append `r.tool_name` (the tool's name string, e.g. `"echo"` / `"search"`) onto `state.pending_queue` for each such ToolResult — this is a P0 placeholder whose sole purpose is keeping `pending_queue` non-empty across iterations so the `_should_stop` predicate's `queue_empty` branch (which fires on `pending_queue == [] and len(stations) >= _MIN_STATIONS_FOR_CONVERGENCE`) does not terminate the run prematurely
- **AND** consumers of `pending_queue` content MUST treat the P0 string as opaque (the value carries no symbolic / path semantics in P0)
- **AND** real symbol or path enqueue lands when `explorer-tools-p2` introduces `follow_reference` tool semantics; that change will MODIFY this Requirement to specify the richer queue payload

---

### Requirement: ReasoningLogger appends one JSONL line per Step to workspace path

The sidecar SHALL implement a `codebus_agent.agent.reasoning_logger.ReasoningLogger` class whose `write(step: Step)` method appends exactly one UTF-8 encoded JSON line per call to `{workspace_root}/reasoning_log.jsonl`. The JSON payload MUST be `step.model_dump_json()` output (Pydantic v2 canonical form) so every field of `Step` (including `thought`, `tool_calls`, `tool_results`, `judge_verdict`, `tokens_used`, `ts`) round-trips via `Step.model_validate_json`.

Every written line SHALL additionally include the `explorer_prompt_version` and `judge_prompt_version` constant strings so golden-sample replays can pin prompt revisions. The logger MUST NOT emit any SSE events in P0 (wiring deferred to the follow-up SSE change) and MUST NOT write to any path outside `workspace_root`.

Write failures (disk full, permission denied) MUST propagate as exceptions so the Explorer loop's error-handling path can log and terminate gracefully — silent drops are forbidden.

#### Scenario: Each write appends exactly one JSONL line

- **WHEN** `ReasoningLogger(path).write(step)` is called K times in sequence
- **THEN** the file at `path` MUST contain exactly K lines, each terminated by `\n`, and each line MUST parse as a `Step` via `Step.model_validate_json`

#### Scenario: Prompt version columns are present on every line

- **WHEN** any line is parsed out of `reasoning_log.jsonl`
- **THEN** the JSON object MUST contain string fields `explorer_prompt_version` and `judge_prompt_version` matching the module-level constants at write time

#### Scenario: Path stays under workspace

- **WHEN** `ReasoningLogger(path)` is constructed where `path` resolves above `workspace_root`
- **THEN** the caller's integration site (e.g., `run_explorer`'s setup) MUST have rejected the path via `ensure_in_workspace` before construction; `ReasoningLogger` itself MAY rely on that precondition and perform no additional path check, but it MUST NOT silently create parent directories outside the workspace

#### Scenario: Logger is the single source of truth for prompt version stamping

- **WHEN** any caller of `ReasoningLogger.write(step)` constructs a `Step` whose `explorer_prompt_version` / `judge_prompt_version` fields are set to caller-supplied values (including the current module-level constants)
- **THEN** `ReasoningLogger.write` MUST overwrite those two fields via `model_copy(update={...})` with the LIVE module-level `EXPLORER_PROMPT_VERSION` / `JUDGE_PROMPT_VERSION` constants before writing, so the on-disk JSONL line carries the version values at write time regardless of caller-supplied values
- **AND** callers (including `run_explorer`'s coverage-gap Step path) MUST NOT rely on pre-stamping the Step — the logger is the single source of truth for prompt-version stamping
- **AND** the rationale is: any future prompt-version bump becomes a single-point edit (`prompts/explorer.py` / `prompts/judge.py` constants); double-stamping creates drift risk where the constant moves but a caller's stale literal value lands in the audit log
