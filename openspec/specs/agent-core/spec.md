# agent-core Specification

## Purpose

TBD - created by archiving change 'explorer-react-loop-p0'. Update Purpose after archive.

## Requirements

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


<!-- @trace
source: agent-defense-depth
updated: 2026-04-27
code:
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/api/scan.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/api/kb.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/kb/payload.py
tests:
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
-->

---
### Requirement: Explorer Think step validates ExplorerAction via Instructor

The `_think(state, provider, tool_specs)` internal function SHALL invoke `await provider.chat(messages, response_model=ExplorerAction)` exactly once per iteration and rely on the provider's Instructor integration to parse + validate the response into an `ExplorerAction` Pydantic instance. The provider passed in MUST be a `TrackedProvider` — the function MUST NOT construct a raw provider or bypass the tracking wrapper. The returned `ExplorerAction` MUST include `thought: str`, `tool_calls: list[ToolCall]`, and `stop: bool` fields.

The `messages` argument to `provider.chat` SHALL consist of the current `state.messages` plus a freshly-rendered user message produced by `render_explorer_prompt(state, tool_specs)`; no messages from prior iterations SHALL be dropped by `_think` itself (any pruning is the responsibility of a future Context-compression change).

#### Scenario: Think returns validated ExplorerAction instance

- **WHEN** `_think` runs against a provider whose `chat()` returns an `ExplorerAction(thought="hi", tool_calls=[], stop=False)`
- **THEN** the returned value MUST be an instance of `ExplorerAction` with the same fields; no raw JSON string MUST leak to the caller

#### Scenario: Think rejects raw (untracked) providers at call-site

- **WHEN** a caller wires `run_explorer` with a provider whose `type(provider)` is not `TrackedProvider`
- **THEN** either the caller's integration test MUST fail at construction time (registry guard), OR `run_explorer` MUST NOT import/use raw provider classes directly — the agent layer's only reachable path to a live LLM goes through a `TrackedProvider` instance supplied by `app.state.llm_reasoning_provider(ws)`

#### Scenario: Prompt version constant is stable across one session

- **WHEN** `_think` runs N times in a single `run_explorer` invocation
- **THEN** every call MUST use the same `EXPLORER_PROMPT_VERSION` constant value as recorded in the reasoning log


<!-- @trace
source: explorer-react-loop-p0
updated: 2026-04-24
code:
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/agent/prompts/judge.py
  - sidecar/src/codebus_agent/agent/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - docs/agent-core.md
  - sidecar/src/codebus_agent/agent/protocols.py
  - sidecar/src/codebus_agent/agent/prompts/explorer.py
  - sidecar/src/codebus_agent/agent/types.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/judge.py
tests:
  - sidecar/tests/agent/test_types.py
  - sidecar/tests/agent/conftest.py
  - sidecar/tests/agent/test_explorer_loop.py
  - sidecar/tests/agent/test_reasoning_logger.py
  - sidecar/tests/agent/test_protocols.py
  - sidecar/tests/agent/test_judge.py
  - sidecar/tests/agent/__init__.py
-->

---
### Requirement: Judge evaluation runs as one-shot call per iteration

The sidecar SHALL implement a `codebus_agent.agent.judge.LLMJudge` class that satisfies the `Judge` Protocol and produces a `JudgeVerdict` by invoking `provider.chat(messages, response_model=JudgeVerdict)` on a `TrackedProvider` obtained via the `llm_judge_provider(workspace_root)` factory (role `ProviderRole.JUDGE`, `default_module="judge"`). Each call to `LLMJudge.evaluate(state, results)` MUST be a one-shot LLM call — it MUST NOT enter a ReAct sub-loop and MUST NOT invoke `ExplorerTools`.

The Judge's returned `JudgeVerdict` SHALL carry `relevance: float` (0.0 ≤ x ≤ 1.0), `should_follow_imports: bool`, `should_add_station: bool`, and `reason: str` fields. The Explorer loop's Update step reads these fields to mutate `state.stations` / `state.pending_queue` but Judge itself MUST NOT mutate `state`.

#### Scenario: Judge produces validated verdict per iteration

- **WHEN** `LLMJudge.evaluate(state, results)` is called where `results` is a list of `ToolResult` objects
- **THEN** the result MUST be an instance of `JudgeVerdict` with `0.0 <= relevance <= 1.0` after Pydantic validation

#### Scenario: Judge provider is distinct from reasoning provider

- **WHEN** `run_explorer` is invoked with its `provider` (reasoning) and its `judge` (backed by `llm_judge_provider`)
- **THEN** the `TrackedProvider` used by `_think` MUST have `role == ProviderRole.REASONING` and the one used by `LLMJudge.evaluate` MUST have `role == ProviderRole.JUDGE`; both MUST resolve to distinct `TrackedProvider` instances (audit records split by role)

#### Scenario: Judge is stateless with respect to ExplorerState

- **WHEN** `LLMJudge.evaluate(state, results)` returns
- **THEN** `state.stations`, `state.visited_files`, `state.pending_queue`, and `state.step_count` MUST be unchanged relative to their values at call entry — only the Explorer loop's Update step is allowed to mutate them


<!-- @trace
source: explorer-react-loop-p0
updated: 2026-04-24
code:
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/agent/prompts/judge.py
  - sidecar/src/codebus_agent/agent/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - docs/agent-core.md
  - sidecar/src/codebus_agent/agent/protocols.py
  - sidecar/src/codebus_agent/agent/prompts/explorer.py
  - sidecar/src/codebus_agent/agent/types.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/judge.py
tests:
  - sidecar/tests/agent/test_types.py
  - sidecar/tests/agent/conftest.py
  - sidecar/tests/agent/test_explorer_loop.py
  - sidecar/tests/agent/test_reasoning_logger.py
  - sidecar/tests/agent/test_protocols.py
  - sidecar/tests/agent/test_judge.py
  - sidecar/tests/agent/__init__.py
-->

---
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

---
### Requirement: Explorer loop stops on budget exhaustion, empty queue, or cancel signal

The sidecar SHALL implement a `_should_stop(state, cancel_event)` predicate (internal to `codebus_agent.agent.explorer`) that returns `True` when any of four convergence conditions fires: (a) `cancel_event.is_set()` is True; (b) a caller-supplied `TokenBudgetProbe` (see separate Requirement) reports `total() >= state.budget_tokens_left`; (c) `state.budget_steps_left <= 0`; (d) `state.pending_queue == []` **and** `len(state.stations) >= _MIN_STATIONS_FOR_CONVERGENCE` (a module-level constant with a sensible P0 default, e.g. 3).

The predicate MUST be evaluated at the top of each loop iteration (before `_think`) so cancel signals abort cleanly without issuing an LLM call. When the predicate returns True, `run_explorer` MUST populate `ExplorerResult.stopped_reason` with exactly one of the documented string values: `"budget_exhausted"`, `"queue_empty"`, `"cancelled"`, or `"budget_tokens_exhausted"`.

The four conditions MUST be evaluated in this precedence order: cancel > token budget > step budget > queue empty. When more than one condition holds simultaneously, the reason string MUST reflect the first one matched, so operators receive the most actionable signal.

Token budget enforcement MUST NOT fire when the caller passes no `TokenBudgetProbe` (the parameter defaults to `None` for backward compatibility with in-process tests and golden-sample replay). Under that path the loop behaves identically to the pre-Requirement revision.

#### Scenario: Budget exhaustion terminates loop

- **WHEN** `run_explorer` is called with `state.budget_steps_left == 0`
- **THEN** the loop body MUST NOT execute even once — no `_think` call MUST fire — and the returned `ExplorerResult.stopped_reason` MUST equal `"budget_exhausted"`

#### Scenario: Cancel event short-circuits mid-run

- **WHEN** a caller sets the `asyncio.Event` passed as `cancel_event` between iterations K and K+1
- **THEN** iteration K+1 MUST NOT invoke `_think`, `_execute_tools`, or `judge.evaluate`; `run_explorer` MUST return `ExplorerResult` with `stopped_reason == "cancelled"` and the stations accumulated through iteration K intact

#### Scenario: Queue empty + enough stations terminates cleanly

- **WHEN** an iteration completes such that `state.pending_queue == []` and `len(state.stations) >= _MIN_STATIONS_FOR_CONVERGENCE`
- **THEN** the next iteration's `_should_stop` check MUST return True with `stopped_reason == "queue_empty"`

#### Scenario: Token budget exhaustion terminates loop

- **WHEN** `run_explorer` is called with a non-None `token_probe` whose `total()` returns a value `>= state.budget_tokens_left` at the start of iteration K+1
- **THEN** iteration K+1 MUST NOT invoke `_think` and `run_explorer` MUST return `ExplorerResult` with `stopped_reason == "budget_tokens_exhausted"`
- **AND** the stations accumulated through iteration K MUST be preserved intact on the returned result

#### Scenario: Missing token probe leaves token budget unenforced

- **WHEN** `run_explorer` is called with `token_probe=None` (the backward-compatible default)
- **THEN** the `budget_tokens_exhausted` branch MUST NOT fire regardless of `state.budget_tokens_left` value
- **AND** every previously-passing test in `sidecar/tests/agent/` that calls `run_explorer` without a `token_probe` MUST continue to pass with identical terminal behaviour


<!-- @trace
source: explorer-react-loop-p0
updated: 2026-04-25
extended_by: context-compression-token-budget
code:
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/budget.py
  - sidecar/src/codebus_agent/agent/types.py
tests:
  - sidecar/tests/agent/test_explorer_loop.py
  - sidecar/tests/agent/test_token_budget_enforcement.py
  - sidecar/tests/agent/test_budget_probe.py
docs:
  - docs/agent-core.md
-->

---
### Requirement: ExplorerTools, Judge, and CoverageChecker are structural Protocols

The sidecar SHALL expose three `typing.Protocol` types in `codebus_agent.agent.protocols` that define the boundary between Explorer core and pluggable implementations: `ExplorerTools` (with `primary_search`, `fetch`, `follow_reference` coroutines), `Judge` (with `evaluate`), and `CoverageChecker` (with `check`). These Protocols MUST be `runtime_checkable` so tests can assert duck-typing conformance, but `run_explorer` MUST NOT perform `isinstance` checks in its hot path — type checking is enforced statically and at test boundaries.

The Protocol surface is the day-1 abstraction that unlocks future reuse: Q&A Agent (Module 8) and Topic-mode Explorer (Phase 2) supply their own implementations without touching the core loop. Therefore the P0 shape MUST NOT leak Folder-mode-specific assumptions (e.g. file paths) into the Protocol signatures — use abstract types like `SearchHit`, `Content`, `Target` defined alongside the Protocols.

`ExplorerTools` SHALL additionally declare an OPTIONAL `tool_specs() -> list[dict]` method that returns the tool-spec list consumed by `render_explorer_prompt(state, tool_specs)`. The method is OPTIONAL at the Protocol level (implementors are permitted to omit it) and `run_explorer` MUST provide a fallback empty list when absent. Concrete Folder-mode `FolderTools` (landed by `explorer-tools-p0` and extended by `explorer-tools-p1`) SHALL implement `tool_specs()` to return one dict per exposed tool with keys `name` / `description` / `parameters` so the Explorer Think-step prompt advertises its real tool surface instead of the empty `[]` default supplied in P0. The full P0+P1 tool surface is six entries: the four P0 tools (`search` / `list_dir` / `read_file` / `mark_station`) plus the two P1 differentiated weapons (`trace_import` / `find_callers`).

#### Scenario: MockTools satisfies ExplorerTools structurally

- **WHEN** a test class implements `primary_search` / `fetch` / `follow_reference` with correct coroutine signatures (no `ExplorerTools` inheritance)
- **THEN** `isinstance(mock_tools, ExplorerTools)` MUST return True via `runtime_checkable`, and `run_explorer` MUST accept it as the `tools` argument without type error

#### Scenario: Protocols do not bind Folder-mode types

- **WHEN** `ExplorerTools.primary_search`'s signature is inspected
- **THEN** its parameters and return type MUST be abstract (`query: str` → `list[SearchHit]`) rather than Folder-specific types, so a `TopicTools` implementation (Phase 2) can satisfy the same Protocol without core-loop changes

#### Scenario: tool_specs method is optional on ExplorerTools

- **WHEN** a minimal `_MockTools` class implements only `primary_search` / `fetch` / `follow_reference` and omits `tool_specs`
- **THEN** `isinstance(mock_tools, ExplorerTools)` MUST still return True
- **AND** `run_explorer` MUST fall back to an empty `tool_specs=[]` for prompt rendering without raising `AttributeError`

#### Scenario: FolderTools advertises its tool surface via tool_specs

- **WHEN** a `FolderTools` instance is passed to `run_explorer`
- **AND** `tool_specs()` is invoked on that instance
- **THEN** the return value MUST be a `list[dict]` containing at least one entry for each of the six P0+P1 tools: `search` / `list_dir` / `read_file` / `mark_station` / `trace_import` / `find_callers`
- **AND** each entry MUST carry `name` / `description` / `parameters` keys so the prompt render can advertise them to the LLM
- **AND** the list MUST NOT silently drop the two P1 tools (`trace_import` / `find_callers`) added by `explorer-tools-p1` — both MUST be present alongside the four P0 tools so the LLM sees the full Folder-mode tool surface


<!-- @trace
source: spec-cleanup-stage-5-batch-b
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - docs/sidecar-api.md
  - CLAUDE.md
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
-->

---
### Requirement: Agent-core types are Pydantic BaseModels with stable JSON serialization

All ReAct data structures exposed in `codebus_agent.agent.types` (`Message`, `ToolCall`, `ToolResult`, `Step`, `JudgeVerdict`, `CoverageResult`, `Station`, `ExplorerState`, `ExplorerAction`, `ExplorerResult`) SHALL be Pydantic `BaseModel` subclasses. Each type MUST round-trip via `model_dump_json` / `model_validate_json` without data loss so that `reasoning_log.jsonl` replay, golden-sample fixtures, and future Generator-side consumers (Module 5) can rely on the on-disk schema.

Numeric fields with natural bounds (e.g., `JudgeVerdict.relevance`) MUST carry Pydantic validators (e.g., `Field(ge=0, le=1)`) so out-of-range LLM outputs are rejected at parse time rather than polluting state.

#### Scenario: ExplorerAction round-trips through Instructor parse path

- **WHEN** a raw JSON payload `{"thought": "...", "tool_calls": [], "stop": false}` is validated via `ExplorerAction.model_validate_json`
- **THEN** the resulting instance's `model_dump_json()` MUST yield a string that `json.loads` equal to the original object (key order may differ)

#### Scenario: JudgeVerdict rejects out-of-range relevance

- **WHEN** a JSON payload with `relevance=1.5` is validated via `JudgeVerdict.model_validate_json`
- **THEN** Pydantic MUST raise a `ValidationError`; Instructor's retry machinery will either re-prompt the LLM or surface the error — the agent layer MUST NOT silently accept out-of-range values

<!-- @trace
source: explorer-react-loop-p0
updated: 2026-04-24
code:
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/agent/prompts/judge.py
  - sidecar/src/codebus_agent/agent/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - docs/agent-core.md
  - sidecar/src/codebus_agent/agent/protocols.py
  - sidecar/src/codebus_agent/agent/prompts/explorer.py
  - sidecar/src/codebus_agent/agent/types.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/judge.py
tests:
  - sidecar/tests/agent/test_types.py
  - sidecar/tests/agent/conftest.py
  - sidecar/tests/agent/test_explorer_loop.py
  - sidecar/tests/agent/test_reasoning_logger.py
  - sidecar/tests/agent/test_protocols.py
  - sidecar/tests/agent/test_judge.py
  - sidecar/tests/agent/__init__.py
-->

---
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


<!-- @trace
source: coverage-gap-recurse
updated: 2026-04-24
code:
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/coverage.py
  - sidecar/src/codebus_agent/agent/types.py
tests:
  - sidecar/tests/agent/test_coverage_recursion.py
  - sidecar/tests/agent/test_coverage.py
docs:
  - docs/agent-core.md
-->

---
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


<!-- @trace
source: coverage-gap-recurse
updated: 2026-04-24
code:
  - sidecar/src/codebus_agent/agent/coverage.py
  - sidecar/src/codebus_agent/agent/prompts/coverage.py
  - sidecar/src/codebus_agent/agent/types.py
tests:
  - sidecar/tests/agent/test_coverage.py
  - sidecar/tests/agent/prompts/test_coverage_prompt.py
docs:
  - docs/agent-core.md
-->

---
### Requirement: Explorer applies rolling message window before each Think call

The sidecar SHALL keep a module-level constant `_MESSAGE_ROLLING_WINDOW: int` in `codebus_agent.agent.explorer` (default value 16) that bounds the number of trailing `state.messages` entries forwarded to `TrackedProvider.chat` during the `_think` substep. The `_think` implementation MUST compose the provider wire prompt as `[system_message, *normalized_history, user_prompt]` in that exact order — `system_message` MUST be the first element of the messages array passed to `provider.chat`. Earlier entries beyond the window MUST be dropped from the wire payload only (not from `state.messages`).

`normalized_history` is `state.messages[-_MESSAGE_ROLLING_WINDOW:]` after rewriting orphan `role == "tool"` entries to `role == "user"` notes. Each `tool` role message in OpenAI Chat Completions MUST follow an `assistant` message containing matching `tool_calls` per the OpenAI API ordering contract; window slicing may strip the preceding `assistant tool_calls`, and the current Explorer architecture never emits `assistant tool_calls` into `state.messages` at all (Instructor consumes the assistant response and only the resulting `ToolResult`s are appended via `_append_observations`). Both situations leave orphan `tool` messages whose immediately preceding entry is neither an `assistant` with non-empty `tool_calls` nor another non-orphan `tool` chained from the same assistant. To remain compatible with OpenAI Chat Completions ordering AND preserve cross-iteration observation visibility, `_think` MUST rewrite each orphan `tool` entry as a `role == "user"` message whose content embeds the original tool name and observation text (so the LLM still sees what the previous iteration observed). Paired `tool` messages (preceded by an `assistant` with `tool_calls`, or by another already-paired `tool` chained from one) MUST pass through unchanged. Sending an orphan `role == "tool"` message in the wire payload causes `400 invalid_request_error` ("messages with role 'tool' must be a response to a preceding message with 'tool_calls'") — the rewrite is the mitigation.

The rolling window MUST NOT mutate `state.messages`, `state.visited_files`, `state.stations`, `state.pending_queue`, or any other field of `ExplorerState`. Reasoning-log audit (`reasoning_log.jsonl`) MUST continue to capture the full per-iteration Step record and MUST NOT be abbreviated by the window or by the orphan-tool rewrite.

The window MUST apply uniformly across main-loop iterations and across coverage-gap recursion frames (i.e., the recursive `run_explorer` call on `_depth=_depth+1` receives the same slicing AND the same orphan-tool rewrite).

Judge and Coverage Checker one-shot calls MUST NOT apply the window: their `render_judge_prompt(state, results)` and `render_coverage_prompt(state)` helpers already bound their own context (visited-files window 20 + `... (N more)` footer, stations tail, ToolResult 800-char truncation). The rolling window is strictly for the cross-iteration Explorer wire path.

#### Scenario: System message is first element of provider.chat payload

- **WHEN** `_think` is invoked with any `state.messages` length (zero or more)
- **THEN** the `messages` argument passed to `provider.chat` MUST have `messages[0].role == "system"`
- **AND** the system message content MUST equal `EXPLORER_SYSTEM`
- **AND** the user prompt MUST appear as the last element (`messages[-1].role == "user"`)

#### Scenario: Orphan tool messages are converted to user notes

- **WHEN** `state.messages[-_MESSAGE_ROLLING_WINDOW:]` slice contains one or more entries whose `role == "tool"` and whose immediately preceding entry (in the slice, walking left-to-right) is neither `role == "assistant"` with non-empty `tool_calls` nor another non-orphan `tool`
- **THEN** `_think` MUST rewrite each such orphan entry as a `role == "user"` message in the wire payload
- **AND** the rewritten user-note's content MUST embed the original `tool` message's content (and `tool_name` when available) so the LLM still sees the observation
- **AND** the messages array passed to `provider.chat` MUST NOT contain any `role == "tool"` entry whose immediately preceding entry has neither `role == "assistant"` with non-empty `tool_calls` nor another (non-orphan) `role == "tool"` chained from the same assistant

#### Scenario: Assistant tool_calls and matching tool messages stay paired inside the window

- **WHEN** the slice `state.messages[-_MESSAGE_ROLLING_WINDOW:]` contains an `assistant` message with `tool_calls` followed by one or more `tool` messages responding to those calls (the assistant is not orphaned)
- **THEN** `_think` MUST NOT rewrite the `assistant` or any of its trailing `tool` messages
- **AND** all of these messages MUST be forwarded to `provider.chat` in their original order with their original roles preserved

#### Scenario: Think receives at most window-size messages when state grew larger

- **WHEN** `run_explorer` completes an iteration that leaves `len(state.messages) > _MESSAGE_ROLLING_WINDOW`
- **THEN** the next iteration's `_think` call MUST pass at most `_MESSAGE_ROLLING_WINDOW` history messages (after orphan-tool rewrite — rewrite preserves length, only changes role / content) plus the prepended `system` message and the appended `user` message into `provider.chat`
- **AND** the dropped messages (`state.messages[:-_MESSAGE_ROLLING_WINDOW]`) MUST remain on `state.messages` unchanged

#### Scenario: Think preserves all state when message count is below window

- **WHEN** `run_explorer` invokes `_think` with `len(state.messages) <= _MESSAGE_ROLLING_WINDOW`
- **THEN** `provider.chat` MUST receive a payload of length `len(state.messages) + 2` (every entry of `state.messages` after orphan-tool rewrite, plus the prepended `system` message and the appended `user` message)
- **AND** no slicing MUST be observable at the provider boundary
- **AND** the orphan-tool rewrite MUST still apply when relevant (rewriting only changes role / content; it does not change the wire payload's length)

#### Scenario: Reasoning log records full iteration history despite windowing

- **WHEN** `run_explorer` writes the Step for an iteration whose wire prompt was windowed or had orphan tool messages rewritten
- **THEN** the Step's `tool_results` field MUST contain every `ToolResult` emitted in that iteration in full
- **AND** no Step field MUST reflect the windowed wire prompt or the rewrite (the log is faithful to the iteration, not to the prompt)

#### Scenario: Coverage-gap recursion frame respects the same window and rewrite

- **WHEN** `run_explorer` recurses into a coverage-gap frame (`_depth=_depth+1`) and that frame's first `_think` call is invoked
- **THEN** the windowing AND the orphan-tool rewrite MUST apply identically — `provider.chat` MUST receive a payload where `messages[0].role == "system"`, the windowed history (after rewrite) is in the middle, and the user prompt is last
- **AND** the `_enqueue_gap_investigation` user-summary message MUST be visible in the windowed slice (because it is the most recent entry appended to `state.messages` before recursion)
