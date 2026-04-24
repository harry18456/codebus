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


## ADDED Requirements

### Requirement: Coverage-gap recursion runs after main loop convergence

The sidecar SHALL guard coverage-gap recursion in `codebus_agent.agent.explorer.run_explorer` with a keyword-only `_depth: int = 0` parameter and a module-level constant `_COVERAGE_MAX_DEPTH: int = 3`. After the main while loop exits, `run_explorer` MUST call `await coverage.check(state)` exactly once. Recursion into a nested `run_explorer(..., _depth=_depth + 1)` call MUST fire if and only if all three preconditions hold simultaneously: (1) `coverage.check` returned at least one `Gap`; (2) `state.budget_steps_left > 0`; (3) `_depth < _COVERAGE_MAX_DEPTH`.

When recursion fires, the sidecar SHALL call an internal `_enqueue_gap_investigation(state, gaps)` helper that MUST perform two mutations on the shared `ExplorerState` instance before the recursive call: (a) append each gap's `suggested_target` (or a `f"gap:{description[:80]}"` placeholder when `suggested_target is None`) onto `state.pending_queue`; (b) append a single `Message(role="user", content=...)` onto `state.messages` summarising the gap count and the first up to three gap descriptions. The shared state MUST NOT be deep-copied — gap rounds accumulate stations / visited files / messages / step counts on the same `ExplorerState`.

Regardless of whether recursion fires, when `gaps` is non-empty the sidecar SHALL write exactly one extra `Step` line to `reasoning_log.jsonl` via the existing `logger.write(...)` path. The Step MUST carry `thought=f"[coverage] round-{_depth + 1} gaps={len(gaps)} will_recurse={will_recurse}"`, empty `tool_calls`, empty `tool_results`, and `judge_verdict=None`. When `gaps` is empty (no-op coverage check) the sidecar MUST NOT write an extra Step line.

The innermost `run_explorer` call's `stopped_reason` MUST propagate unchanged through the recursive return chain so callers see the convergence branch that fired in the deepest frame.

#### Scenario: Empty gaps terminate without recursion

- **WHEN** `run_explorer` completes its main while loop and `coverage.check(state)` returns `[]`
- **THEN** `run_explorer` MUST return without recursing
- **AND** no extra `Step` line MUST be written for this coverage round
- **AND** `_enqueue_gap_investigation` MUST NOT be invoked

#### Scenario: Gaps with budget trigger one recursion round

- **WHEN** `run_explorer(_depth=0)` completes and `coverage.check(state)` returns two `Gap` entries, `state.budget_steps_left == 5`, and `_COVERAGE_MAX_DEPTH == 3`
- **THEN** `_enqueue_gap_investigation(state, gaps)` MUST be invoked before recursion, growing `state.pending_queue` by exactly two entries and `state.messages` by exactly one `role="user"` message
- **AND** one extra `Step` line MUST be written with `thought` starting `[coverage] round-1 gaps=2 will_recurse=True`
- **AND** `run_explorer` MUST recurse with `_depth=1` using the same `state` / `provider` / `tools` / `judge` / `coverage` / `logger` / `emitter` instances

#### Scenario: Max depth halts further recursion

- **WHEN** `run_explorer(_depth=2)` completes and `coverage.check(state)` returns one `Gap`, `state.budget_steps_left == 10`, and `_COVERAGE_MAX_DEPTH == 3`
- **THEN** `run_explorer` MUST NOT recurse (since `_depth == 2` would recurse into `_depth=3` which equals the cap; precondition `_depth < _COVERAGE_MAX_DEPTH` fails at the next level)
- **AND** one extra `Step` line MUST be written with `thought` starting `[coverage] round-3 gaps=1 will_recurse=False`
- **AND** `run_explorer` MUST return the accumulated `ExplorerResult`

#### Scenario: Budget exhaustion halts recursion even with gaps

- **WHEN** `run_explorer` completes with `state.budget_steps_left == 0` and `coverage.check(state)` returns one `Gap`
- **THEN** `run_explorer` MUST NOT recurse even though gaps are present
- **AND** one extra `Step` line MUST be written with `thought` starting `[coverage] round-1 gaps=1 will_recurse=False`
- **AND** `_enqueue_gap_investigation` MUST NOT be invoked (no mutation on `state.pending_queue` / `state.messages`)


### Requirement: LLMCoverageChecker produces one-shot CoverageResult

The sidecar SHALL expose a `codebus_agent.agent.coverage.LLMCoverageChecker` class whose constructor accepts `provider_factory: Callable[[Path], TrackedProvider]` and `workspace_root: Path`, materialising a single workspace-scoped `TrackedProvider` at construction time. The class MUST structurally satisfy the existing `codebus_agent.agent.protocols.CoverageChecker` Protocol via an `async check(state: ExplorerState) -> list[Gap]` coroutine.

`check` MUST issue exactly one `provider.chat(messages, response_model=CoverageResult)` call per invocation, MUST NOT invoke any `ExplorerTools` method, MUST NOT mutate `ExplorerState`, and MUST return `result.gaps` unchanged. The rendered prompt SHALL carry the task, the current stations list, and a bounded visited-files window (at most 20 entries plus a `... (N more)` footer, mirroring the Judge prompt's bounded rendering) so the LLM receives context without blowing the token budget.

The class SHALL expose a `set_emitter(emitter: SSEEmitter | None) -> None` method that forwards `emitter` to the wrapped `TrackedProvider.set_emitter`, so coverage-side `usage_delta` / `llm_call` events surface on the same SSE channel as the Explorer loop.

The prompt module `codebus_agent.agent.prompts.coverage` SHALL expose `COVERAGE_SYSTEM: str`, `render_coverage_prompt(state: ExplorerState) -> str`, and `COVERAGE_PROMPT_VERSION: str` (date-version format matching `JUDGE_PROMPT_VERSION`, e.g. `"2026-04-26-1"`) so `reasoning_log.jsonl` replay can pin prompt revisions for drift detection.

#### Scenario: check issues one-shot structured call

- **WHEN** `LLMCoverageChecker.check(state)` is invoked with a populated state
- **THEN** the wrapped `TrackedProvider.chat` MUST be called exactly once with `response_model=CoverageResult`
- **AND** the coroutine MUST return the `result.gaps` list from the validated CoverageResult

#### Scenario: check does not mutate ExplorerState

- **WHEN** `LLMCoverageChecker.check(state)` runs to completion
- **THEN** `state.stations`, `state.visited_files`, `state.pending_queue`, `state.messages`, `state.step_count`, and `state.budget_steps_left` MUST all be unchanged relative to their values at call entry

#### Scenario: set_emitter propagates to TrackedProvider

- **WHEN** `LLMCoverageChecker.set_emitter(emitter)` is called with a non-None emitter
- **THEN** the wrapped `TrackedProvider.set_emitter` MUST receive the same emitter instance so subsequent `chat` calls emit `usage_delta` / `llm_call` through that channel

#### Scenario: Prompt module exposes version constant

- **WHEN** `codebus_agent.agent.prompts.coverage.COVERAGE_PROMPT_VERSION` is imported
- **THEN** the value MUST be a non-empty string in date-version format `YYYY-MM-DD-N` so reasoning_log replay can compare prompt revisions across runs
