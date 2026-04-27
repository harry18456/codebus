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
- **THEN** the exception MUST be captured into the corresponding `ToolResult.error` field, the loop MUST continue to the Judge / Log / Update sub-steps, and the `Step` line written for that iteration MUST record the failed `ToolResult` verbatim
- **AND** the `ToolResult.output` populated for the error path MUST be Pass 2 sanitized: the loop MUST invoke `ctx.sanitizer.sanitize(error_text, source=MessageSource(message_id=f"explorer_step_{state.step_count}_tool_error"))` before assigning the result string into `ToolResult.output`, and any sanitize hits MUST append one line to `<workspace>/.codebus/sanitize_audit.jsonl` with `pass_num=2`
- **AND** the sanitized error string assigned to `ToolResult.output` MUST NOT contain raw secrets or PII even when the original exception message embeds user input (e.g. file path, search keyword, symbol name)

#### Scenario: Tool error string sanitized through Pass 2

- **WHEN** a tool invocation raises an exception whose string representation contains a literal that the Sanitizer's built-in rules detect as a secret (e.g. `ValueError("api_key=sk-AKIAIOSFODNN7EXAMPLE invalid")`)
- **THEN** the resulting `ToolResult.output` string MUST contain a `<REDACTED:` placeholder
- **AND** the `ToolResult.output` MUST NOT contain the raw `sk-AKIAIOSFODNN7EXAMPLE` literal
- **AND** `<workspace>/.codebus/sanitize_audit.jsonl` MUST have at least one new line with `pass_num=2`
- **AND** the audit line's `source` MUST reflect a `MessageSource` shape (NOT a `FileSource` — the error string is message-channel content, not file content)

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
