# qa-agent Specification

## Purpose

TBD - created by archiving change 'module-8-qa-p0'. Update Purpose after archive.

## Requirements

### Requirement: Q&A loop entry point with two-stage RAG-first flow

The sidecar SHALL expose `codebus_agent.agent.qa.run_qa(question, state, *, kb, tools, sanitizer, sanitizer_audit, kb_growth_logger, provider_factory, workspace_root, emitter=None) -> QAAnswer` as the Q&A Agent entry point, per `docs/decisions.md` D-016 and `docs/qa-agent.md §四`. The function SHALL execute exactly three stages in order: (1) **RAG-first probe** — invoke `kb.query(question, top_k=8)` once and pass the hits through `_hits_confident(question, hits)`; (2) **Optional ReAct loop** — entered only when the probe returns `False`, reusing `codebus_agent.agent.explorer._think`, `_execute_tools`, and `_should_stop` from the existing ReAct core, bounded by the budget constants declared by this capability; (3) **Synthesize** — `_synthesize_answer(state, provider)` produces a final `QAAnswer` regardless of whether the loop ran.

`run_qa` MUST NOT instantiate `LLMJudge` or `LLMCoverageChecker`. The Q&A loop's only stop conditions are budget exhaustion (steps / tokens / wall) and explicit cancellation; station-coverage style verdicts are out of scope for Q&A. This isolation is the design surface that prevents Folder-mode prompt vocabulary from leaking into Q&A behavior.

#### Scenario: Confident hits skip the ReAct loop

- **WHEN** `run_qa` calls `kb.query(question, top_k=8)` and `_hits_confident(question, hits)` returns `True`
- **THEN** `run_qa` MUST return a `QAAnswer` produced by `_answer_from_hits(question, hits, provider)` without entering the ReAct loop
- **AND** the `reasoning_log.jsonl` MUST contain zero ReAct `Step` entries for that call

#### Scenario: Non-confident hits enter the ReAct loop

- **WHEN** `_hits_confident(question, hits)` returns `False` for the initial probe
- **THEN** `run_qa` MUST seed `state.messages` with the rendered Q&A prompt and proceed into the ReAct loop until `_should_stop(state)` returns `True`

#### Scenario: Q&A never instantiates Judge or Coverage

- **WHEN** the `run_qa` module is imported
- **THEN** the module MUST NOT contain any reference to `LLMJudge`, `LLMCoverageChecker`, `Judge` Protocol, or `CoverageChecker` Protocol — verified by an import-graph test


<!-- @trace
source: module-8-qa-p0
updated: 2026-04-26
code:
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/types.py
  - docs/sidecar-api.md
  - docs/decisions.md
  - sidecar/src/codebus_agent/agent/qa.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/agent/tools/qa_tools.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/__init__.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/prompts/qa.py
tests:
  - sidecar/tests/agent/tools/test_kb_search.py
  - sidecar/tests/kb/test_upsert_chunk.py
  - sidecar/tests/api/test_qa_sse_events.py
  - sidecar/tests/agent/test_qa_types.py
  - sidecar/tests/api/test_task_id_qa_kind.py
  - sidecar/tests/agent/tools/test_qa_tools.py
  - sidecar/tests/integration/__init__.py
  - sidecar/tests/kb/test_query_filter_stations.py
  - sidecar/tests/agent/test_qa_prompts.py
  - sidecar/tests/agent/test_hits_confident.py
  - sidecar/tests/agent/test_run_qa.py
  - sidecar/tests/api/test_audit_paths_kb_growth.py
  - sidecar/tests/kb/test_growth_logger.py
  - sidecar/tests/api/test_qa_endpoint.py
  - sidecar/tests/integration/test_qa_end_to_end.py
  - sidecar/tests/agent/test_qa_budget_constants.py
  - sidecar/tests/agent/tools/test_add_to_kb.py
  - sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py
-->

---
### Requirement: `_hits_confident` declares three threshold conditions

The sidecar SHALL implement `_hits_confident(question: str, hits: list[KBHit]) -> bool` in `codebus_agent.agent.qa` whose return value is `True` if and only if all three conditions hold: (1) `hits[0].score > 0.75`; (2) the arithmetic mean of `hits[0..3].score` (top-3) is strictly greater than `0.65`; (3) the union of significant tokens in `hits[0..5].text` (top-5) covers at least one significant token from `question`, where significant tokens are produced by a deterministic helper `_significant_tokens(text)` that lowercases the input, splits on non-alphanumeric boundaries, and drops a fixed stop-word set.

When `len(hits) < 3` the function MUST return `False` regardless of individual scores. When `len(hits) == 0` the function MUST return `False`.

#### Scenario: All three conditions met returns True

- **WHEN** `hits` is `[KBHit(score=0.82, text="storage adapter")...]` and the question is `"how does the storage adapter work"` and at least 3 hits exist
- **THEN** `_hits_confident` MUST return `True` provided top-3 mean > 0.65 and the entity word `"storage"` appears among the top-5 hit texts

#### Scenario: Insufficient hits returns False

- **WHEN** `hits` is `[KBHit(score=0.99)]` (length 1)
- **THEN** `_hits_confident` MUST return `False`

#### Scenario: High top-1 but no entity coverage returns False

- **WHEN** `hits[0].score == 0.90` and `hits[0..5]` contain none of the significant tokens from the question
- **THEN** `_hits_confident` MUST return `False`


<!-- @trace
source: module-8-qa-p0
updated: 2026-04-26
code:
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/types.py
  - docs/sidecar-api.md
  - docs/decisions.md
  - sidecar/src/codebus_agent/agent/qa.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/agent/tools/qa_tools.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/__init__.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/prompts/qa.py
tests:
  - sidecar/tests/agent/tools/test_kb_search.py
  - sidecar/tests/kb/test_upsert_chunk.py
  - sidecar/tests/api/test_qa_sse_events.py
  - sidecar/tests/agent/test_qa_types.py
  - sidecar/tests/api/test_task_id_qa_kind.py
  - sidecar/tests/agent/tools/test_qa_tools.py
  - sidecar/tests/integration/__init__.py
  - sidecar/tests/kb/test_query_filter_stations.py
  - sidecar/tests/agent/test_qa_prompts.py
  - sidecar/tests/agent/test_hits_confident.py
  - sidecar/tests/agent/test_run_qa.py
  - sidecar/tests/api/test_audit_paths_kb_growth.py
  - sidecar/tests/kb/test_growth_logger.py
  - sidecar/tests/api/test_qa_endpoint.py
  - sidecar/tests/integration/test_qa_end_to_end.py
  - sidecar/tests/agent/test_qa_budget_constants.py
  - sidecar/tests/agent/tools/test_add_to_kb.py
  - sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py
-->

---
### Requirement: Q&A budget constants are module-level

The sidecar SHALL declare the following module-level constants in `codebus_agent.agent.qa` and use them as the only source of truth for Q&A safety guards:

- `_QA_MAX_STEPS` = `10`
- `_QA_MAX_ADD_TO_KB_PER_SESSION` = `20`
- `_QA_MAX_CHUNK_SIZE_CHARS` = `2000`
- `_QA_MAX_ADD_TO_KB_PER_QUESTION` = `5`
- `_QA_DEDUP_THRESHOLD` = `0.95`

`run_qa` MUST pass `_QA_MAX_STEPS` to `_should_stop` as the step ceiling. `add_to_kb` invocations MUST consult `_QA_MAX_ADD_TO_KB_PER_SESSION` and `_QA_MAX_ADD_TO_KB_PER_QUESTION` and refuse further writes by returning a string error to the calling Agent when the limit is reached. Each `chunks[*].text` whose post-Sanitize length exceeds `_QA_MAX_CHUNK_SIZE_CHARS` MUST be rejected without writing to KB or `kb_growth.jsonl`.

#### Scenario: Step limit honored via _should_stop

- **WHEN** `state.step_count` reaches `_QA_MAX_STEPS` during a Q&A run
- **THEN** the next `_should_stop(state)` call MUST return `True` and `run_qa` MUST exit the loop without further `_think` invocations

#### Scenario: Per-session add_to_kb limit refuses further writes

- **WHEN** `add_to_kb` has been invoked successfully `_QA_MAX_ADD_TO_KB_PER_SESSION` times in the current session
- **THEN** the next `add_to_kb` invocation MUST return a string starting with `"budget exhausted"` and MUST NOT call `kb.upsert_chunk` or `kb_growth_logger.write`

#### Scenario: Oversize chunk rejected without KB write

- **WHEN** a `chunks[*].text` post-Sanitize length is `2001` characters
- **THEN** that chunk MUST be skipped, the response MUST identify the rejection reason, and no `kb_growth.jsonl` line MUST be written for that chunk


<!-- @trace
source: module-8-qa-p0
updated: 2026-04-26
code:
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/types.py
  - docs/sidecar-api.md
  - docs/decisions.md
  - sidecar/src/codebus_agent/agent/qa.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/agent/tools/qa_tools.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/__init__.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/prompts/qa.py
tests:
  - sidecar/tests/agent/tools/test_kb_search.py
  - sidecar/tests/kb/test_upsert_chunk.py
  - sidecar/tests/api/test_qa_sse_events.py
  - sidecar/tests/agent/test_qa_types.py
  - sidecar/tests/api/test_task_id_qa_kind.py
  - sidecar/tests/agent/tools/test_qa_tools.py
  - sidecar/tests/integration/__init__.py
  - sidecar/tests/kb/test_query_filter_stations.py
  - sidecar/tests/agent/test_qa_prompts.py
  - sidecar/tests/agent/test_hits_confident.py
  - sidecar/tests/agent/test_run_qa.py
  - sidecar/tests/api/test_audit_paths_kb_growth.py
  - sidecar/tests/kb/test_growth_logger.py
  - sidecar/tests/api/test_qa_endpoint.py
  - sidecar/tests/integration/test_qa_end_to_end.py
  - sidecar/tests/agent/test_qa_budget_constants.py
  - sidecar/tests/agent/tools/test_add_to_kb.py
  - sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py
-->

---
### Requirement: Q&A system prompt module is isolated from Explorer prompts

The sidecar SHALL provide `codebus_agent.agent.prompts.qa` exposing `QA_SYSTEM: str`, `render_qa_prompt(state: QAState, question: str, initial_hits: list[KBHit]) -> str`, and `QA_PROMPT_VERSION: str` whose value follows the `YYYY-MM-DD-N` date-version format used by other prompt modules. The system prompt MUST encode the three "worth persisting" rules (reusable, stable fact, non-duplicative) verbatim and MUST include the stable station id format constraint `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$` for `related_stations` validation guidance.

`prompts/qa.py` MUST NOT import from `prompts/explorer.py`, `prompts/judge.py`, or `prompts/coverage.py`. `agent/qa.py` MUST NOT import any symbol from `prompts/explorer.py`, `prompts/judge.py`, or `prompts/coverage.py`.

`ReasoningLogger` MUST stamp `qa_prompt_version` on every `Step` written from `run_qa` (analogous to how Explorer stamps `explorer_prompt_version` and `judge_prompt_version`), drawn from `QA_PROMPT_VERSION`.

#### Scenario: Prompt module exposes required symbols

- **WHEN** `from codebus_agent.agent.prompts import qa as qa_prompts` is executed
- **THEN** `hasattr(qa_prompts, "QA_SYSTEM")` AND `hasattr(qa_prompts, "render_qa_prompt")` AND `hasattr(qa_prompts, "QA_PROMPT_VERSION")` MUST all be `True`
- **AND** `re.match(r"^\d{4}-\d{2}-\d{2}-\d+$", qa_prompts.QA_PROMPT_VERSION)` MUST return a non-None match

#### Scenario: No cross-module prompt import

- **WHEN** an import-graph test inspects `codebus_agent.agent.qa` and `codebus_agent.agent.prompts.qa`
- **THEN** neither module MUST import any symbol from `codebus_agent.agent.prompts.explorer`, `codebus_agent.agent.prompts.judge`, or `codebus_agent.agent.prompts.coverage`

#### Scenario: ReasoningLogger stamps qa_prompt_version on every Q&A step

- **WHEN** `run_qa` writes a `Step` via `ReasoningLogger.write(step)` during a Q&A run
- **THEN** the resulting JSONL line MUST contain a `qa_prompt_version` field equal to `QA_PROMPT_VERSION`
- **AND** the line MUST NOT contain `explorer_prompt_version` or `judge_prompt_version`


<!-- @trace
source: module-8-qa-p0
updated: 2026-04-26
code:
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/types.py
  - docs/sidecar-api.md
  - docs/decisions.md
  - sidecar/src/codebus_agent/agent/qa.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/agent/tools/qa_tools.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/__init__.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/prompts/qa.py
tests:
  - sidecar/tests/agent/tools/test_kb_search.py
  - sidecar/tests/kb/test_upsert_chunk.py
  - sidecar/tests/api/test_qa_sse_events.py
  - sidecar/tests/agent/test_qa_types.py
  - sidecar/tests/api/test_task_id_qa_kind.py
  - sidecar/tests/agent/tools/test_qa_tools.py
  - sidecar/tests/integration/__init__.py
  - sidecar/tests/kb/test_query_filter_stations.py
  - sidecar/tests/agent/test_qa_prompts.py
  - sidecar/tests/agent/test_hits_confident.py
  - sidecar/tests/agent/test_run_qa.py
  - sidecar/tests/api/test_audit_paths_kb_growth.py
  - sidecar/tests/kb/test_growth_logger.py
  - sidecar/tests/api/test_qa_endpoint.py
  - sidecar/tests/integration/test_qa_end_to_end.py
  - sidecar/tests/agent/test_qa_budget_constants.py
  - sidecar/tests/agent/tools/test_add_to_kb.py
  - sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py
-->

---
### Requirement: QATools exposes seven tools with audit_fields declared

The sidecar SHALL provide `codebus_agent.agent.tools.qa_tools.QATools` whose instance methods declare `audit_fields: list[str]` per `tool-sandbox` Requirement `Tools declare audit_fields whitelist`. The class SHALL expose exactly seven tool methods, all reachable via `getattr(tools, call.name)` from the Q&A ReAct loop:

- Five reused read tools delegating to `FolderTools` semantics: `search`, `list_dir`, `read_file`, `trace_import`, `find_callers` — each with `audit_fields` matching `FolderTools` of the same name
- `kb_search(args: KBSearchArgs, ctx: ToolContext) -> str` with `audit_fields = ["query", "top_k", "station_filter"]`
- `add_to_kb(args: AddToKBArgs, ctx: ToolContext) -> str` with `audit_fields = ["source", "reason", "related_stations"]`

`add_to_kb` `audit_fields` MUST NOT include `chunks` (each chunk's `text` is sanitized and its replacements are recorded in `sanitize_audit.jsonl`; replicating chunk text into `tool_audit.jsonl` would create a parallel audit surface). `kb_search` `audit_fields` MAY include `query` because the search string is part of the Agent's decision trace, not pre-sanitize user content.

#### Scenario: All seven tools declare audit_fields

- **WHEN** `QATools` is registered with `ToolSandbox`
- **THEN** registration MUST succeed without raising — meaning all seven tools have `audit_fields` declared as `list[str]`

#### Scenario: add_to_kb audit_fields excludes chunks

- **WHEN** the `audit_fields` attribute of `QATools.add_to_kb` is inspected
- **THEN** the list MUST NOT contain the string `"chunks"`
- **AND** the list MUST contain at minimum `"source"`, `"reason"`, and `"related_stations"`


<!-- @trace
source: module-8-qa-p0
updated: 2026-04-26
code:
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/types.py
  - docs/sidecar-api.md
  - docs/decisions.md
  - sidecar/src/codebus_agent/agent/qa.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/agent/tools/qa_tools.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/__init__.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/prompts/qa.py
tests:
  - sidecar/tests/agent/tools/test_kb_search.py
  - sidecar/tests/kb/test_upsert_chunk.py
  - sidecar/tests/api/test_qa_sse_events.py
  - sidecar/tests/agent/test_qa_types.py
  - sidecar/tests/api/test_task_id_qa_kind.py
  - sidecar/tests/agent/tools/test_qa_tools.py
  - sidecar/tests/integration/__init__.py
  - sidecar/tests/kb/test_query_filter_stations.py
  - sidecar/tests/agent/test_qa_prompts.py
  - sidecar/tests/agent/test_hits_confident.py
  - sidecar/tests/agent/test_run_qa.py
  - sidecar/tests/api/test_audit_paths_kb_growth.py
  - sidecar/tests/kb/test_growth_logger.py
  - sidecar/tests/api/test_qa_endpoint.py
  - sidecar/tests/integration/test_qa_end_to_end.py
  - sidecar/tests/agent/test_qa_budget_constants.py
  - sidecar/tests/agent/tools/test_add_to_kb.py
  - sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py
-->

---
### Requirement: kb_search invokes KnowledgeBase query with optional station filter

The sidecar SHALL implement `kb_search(args: KBSearchArgs, ctx: ToolContext) -> str` such that it forwards the request to `ctx.kb.query(args.query, top_k=args.top_k, filter_stations=args.station_filter)`. `KBSearchArgs` MUST be a Pydantic model with fields `query: str`, `top_k: int = 5`, `station_filter: list[str] | None = None`. Each entry in `station_filter` MUST match `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$`; non-matching values MUST cause Pydantic validation error before the tool body runs.

The tool's return value MUST be a single multi-line string where each hit is rendered as `<file>:<line> | score=<X.XX> | stations=[<id>,...]\n  <snippet>`. When a hit's `payload.related_stations` is empty, the `stations=` segment MUST be omitted (not rendered as `stations=[]`). The snippet MUST be truncated to at most 200 characters and end with `…` when truncated.

#### Scenario: station_filter forwarded to KB query

- **WHEN** `kb_search(KBSearchArgs(query="x", station_filter=["s02-storage"]))` is invoked
- **THEN** `ctx.kb.query` MUST be called with `filter_stations=["s02-storage"]`

#### Scenario: Invalid station id rejected by Pydantic

- **WHEN** `KBSearchArgs(query="x", station_filter=["invalid-id"])` is constructed
- **THEN** Pydantic MUST raise `ValidationError` referencing the regex constraint, before any KB call

#### Scenario: Hit rendering omits empty station list

- **WHEN** a returned hit has `payload.related_stations == []`
- **THEN** the rendered line MUST NOT contain the substring `stations=`


<!-- @trace
source: module-8-qa-p0
updated: 2026-04-26
code:
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/types.py
  - docs/sidecar-api.md
  - docs/decisions.md
  - sidecar/src/codebus_agent/agent/qa.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/agent/tools/qa_tools.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/__init__.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/prompts/qa.py
tests:
  - sidecar/tests/agent/tools/test_kb_search.py
  - sidecar/tests/kb/test_upsert_chunk.py
  - sidecar/tests/api/test_qa_sse_events.py
  - sidecar/tests/agent/test_qa_types.py
  - sidecar/tests/api/test_task_id_qa_kind.py
  - sidecar/tests/agent/tools/test_qa_tools.py
  - sidecar/tests/integration/__init__.py
  - sidecar/tests/kb/test_query_filter_stations.py
  - sidecar/tests/agent/test_qa_prompts.py
  - sidecar/tests/agent/test_hits_confident.py
  - sidecar/tests/agent/test_run_qa.py
  - sidecar/tests/api/test_audit_paths_kb_growth.py
  - sidecar/tests/kb/test_growth_logger.py
  - sidecar/tests/api/test_qa_endpoint.py
  - sidecar/tests/integration/test_qa_end_to_end.py
  - sidecar/tests/agent/test_qa_budget_constants.py
  - sidecar/tests/agent/tools/test_add_to_kb.py
  - sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py
-->

---
### Requirement: add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order

The sidecar SHALL implement `add_to_kb(args: AddToKBArgs, ctx: ToolContext) -> str` whose body executes the following steps for each chunk in `args.chunks`, in this order, with no reordering permitted:

1. **Sanitize** — `ctx.sanitizer.sanitize(chunk.text, source=FileSource(path=chunk.source, pass_="qa_add_to_kb"))`. Each `AuditEntry` returned MUST be appended to `ctx.sanitizer_audit` with `pass_num=3`. If the sanitized text strips to empty, the chunk MUST be skipped without further side effects (no KB upsert, no `kb_growth.jsonl` write); the per-chunk response token MUST be `"skipped_empty"`.
2. **Validate `related_stations`** — every id in `chunk.related_stations` MUST match `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$`. On any non-match, the entire `add_to_kb` invocation MUST return a string starting with `"invalid station_id:"` and naming the offending id; previously-processed chunks in the same invocation MUST remain committed (the call is not transactional across chunks).
3. **Upsert via KB layer** — `ctx.kb.upsert_chunk(text=clean, payload=KBPayload(...))` where `payload.added_by="qa_agent"`, `payload.session_id` reflects `ctx.session_id`, and `payload.related_stations` reflects the validated list.
4. **Growth log** — `ctx.kb_growth_logger.write(point_id=..., source=chunk.source, reason=args.reason, related_stations=chunk.related_stations, originating_station_id=ctx.originating_station_id, sanitize_stats=<derived>, chunk_size_chars=len(clean), dedup_skipped=<bool>, session_id=ctx.session_id, question=ctx.question)`. The growth log MUST be written even when `upsert_chunk` returns a `"dedup:..."` token, with `dedup_skipped=True`.

When the per-session or per-question `add_to_kb` budget has been exhausted, the tool MUST return a string starting with `"budget exhausted:"` before sanitize is invoked, so audit lines do not record a redaction that did not actually happen.

#### Scenario: Order of operations sanitize → validate → upsert → log

- **WHEN** `add_to_kb` is invoked with a single chunk that triggers a Pass 3 sanitize hit, validates, and upserts
- **THEN** the test recorder MUST observe these calls in this exact sequence: `sanitizer.sanitize(...)`, then `sanitizer_audit.append(... pass_num=3 ...)`, then `kb.upsert_chunk(...)`, then `kb_growth_logger.write(...)`

#### Scenario: Empty post-sanitize chunk skipped without KB or growth-log writes

- **WHEN** a chunk's post-sanitize text is empty (full redaction)
- **THEN** `kb.upsert_chunk` MUST NOT be called for that chunk
- **AND** `kb_growth_logger.write` MUST NOT be called for that chunk
- **AND** the per-chunk return token MUST be `"skipped_empty"`

#### Scenario: Dedup hit still records growth-log line with dedup_skipped=true

- **WHEN** `kb.upsert_chunk` returns `"dedup:hash"` for a chunk
- **THEN** `kb_growth_logger.write` MUST be called with `dedup_skipped=True` for that chunk
- **AND** the per-chunk return token MUST equal the `"dedup:hash"` string returned by `upsert_chunk`

#### Scenario: Invalid station_id aborts before upsert

- **WHEN** a chunk's `related_stations` contains the string `"s2-bad"` (missing two-digit segment)
- **THEN** `add_to_kb` MUST return `"invalid station_id: s2-bad"` (or string starting with that prefix)
- **AND** `kb.upsert_chunk` MUST NOT be called for that chunk
- **AND** earlier successful chunks in the same invocation MUST remain committed


<!-- @trace
source: module-8-qa-p0
updated: 2026-04-26
code:
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/types.py
  - docs/sidecar-api.md
  - docs/decisions.md
  - sidecar/src/codebus_agent/agent/qa.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/agent/tools/qa_tools.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/__init__.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/prompts/qa.py
tests:
  - sidecar/tests/agent/tools/test_kb_search.py
  - sidecar/tests/kb/test_upsert_chunk.py
  - sidecar/tests/api/test_qa_sse_events.py
  - sidecar/tests/agent/test_qa_types.py
  - sidecar/tests/api/test_task_id_qa_kind.py
  - sidecar/tests/agent/tools/test_qa_tools.py
  - sidecar/tests/integration/__init__.py
  - sidecar/tests/kb/test_query_filter_stations.py
  - sidecar/tests/agent/test_qa_prompts.py
  - sidecar/tests/agent/test_hits_confident.py
  - sidecar/tests/agent/test_run_qa.py
  - sidecar/tests/api/test_audit_paths_kb_growth.py
  - sidecar/tests/kb/test_growth_logger.py
  - sidecar/tests/api/test_qa_endpoint.py
  - sidecar/tests/integration/test_qa_end_to_end.py
  - sidecar/tests/agent/test_qa_budget_constants.py
  - sidecar/tests/agent/tools/test_add_to_kb.py
  - sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py
-->

---
### Requirement: Q&A run emits SSE events on the task channel

The sidecar SHALL emit the following SSE event types, in the listed order, when `run_qa` is driven by `POST /qa`'s background task wrapper:

- `rag_hits` — emitted exactly once after the initial `kb.query`. Payload MUST contain `hits: list[{score, file_path, line_start, line_end, snippet, related_stations}]` reflecting at most the top-8 probe results.
- `agent_thought` / `agent_action_result` — emitted by the reused ReAct core when (and only when) the loop body runs (i.e., `_hits_confident` returned `False`).
- `kb_growth` — emitted exactly once per successful `add_to_kb` chunk that resulted in a new KB point. Payload MUST contain `entry_id`, `source`, `related_stations`, `originating_station_id`. Dedup-skipped chunks MUST NOT emit `kb_growth`.
- `qa_answer` — emitted exactly once at the end of `run_qa`. Payload MUST contain `answer: str` (the synthesized final answer text) and `citations: list[{file_path, line_start, line_end, related_stations}]`. P0 MUST treat `qa_answer` as a single-shot non-streaming event; field-level streaming is reserved for a later P1 change and MUST NOT alter the schema shape introduced here.
- `usage_delta` / `llm_call` — emitted by `TrackedProvider` per existing contracts when chat / embed calls fire; the Q&A `default_module="qa_agent"` MUST appear on every record from this run.

Failures during the Q&A run MUST surface via the `error` event defined by `sidecar-runtime` (with `code="QA_FAILED"`); they MUST NOT emit a partial `qa_answer`.

#### Scenario: rag_hits event precedes any agent_thought

- **WHEN** `run_qa` is driven by `POST /qa` against an emitter
- **THEN** the sequence of emitted events MUST contain `rag_hits` strictly before any `agent_thought` event (if any `agent_thought` events are emitted)

#### Scenario: kb_growth event omitted on dedup skip

- **WHEN** `add_to_kb` records a chunk that gets dedup-skipped via `upsert_chunk` returning `"dedup:..."`
- **THEN** no `kb_growth` SSE event MUST be emitted for that chunk
- **AND** the corresponding `kb_growth.jsonl` line MUST still be written with `dedup_skipped=True`

#### Scenario: qa_answer payload schema

- **WHEN** the `qa_answer` event is emitted
- **THEN** the payload MUST contain string field `answer` and list field `citations` whose entries each contain `file_path`, `line_start`, `line_end`, and `related_stations`


<!-- @trace
source: module-8-qa-p0
updated: 2026-04-26
code:
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/types.py
  - docs/sidecar-api.md
  - docs/decisions.md
  - sidecar/src/codebus_agent/agent/qa.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/agent/tools/qa_tools.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/__init__.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/prompts/qa.py
tests:
  - sidecar/tests/agent/tools/test_kb_search.py
  - sidecar/tests/kb/test_upsert_chunk.py
  - sidecar/tests/api/test_qa_sse_events.py
  - sidecar/tests/agent/test_qa_types.py
  - sidecar/tests/api/test_task_id_qa_kind.py
  - sidecar/tests/agent/tools/test_qa_tools.py
  - sidecar/tests/integration/__init__.py
  - sidecar/tests/kb/test_query_filter_stations.py
  - sidecar/tests/agent/test_qa_prompts.py
  - sidecar/tests/agent/test_hits_confident.py
  - sidecar/tests/agent/test_run_qa.py
  - sidecar/tests/api/test_audit_paths_kb_growth.py
  - sidecar/tests/kb/test_growth_logger.py
  - sidecar/tests/api/test_qa_endpoint.py
  - sidecar/tests/integration/test_qa_end_to_end.py
  - sidecar/tests/agent/test_qa_budget_constants.py
  - sidecar/tests/agent/tools/test_add_to_kb.py
  - sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py
-->

---
### Requirement: QAState, QAAnswer, and QAAction are Pydantic models

The sidecar SHALL define `QAState`, `QAAction`, and `QAAnswer` as Pydantic `BaseModel` subclasses in `codebus_agent.agent.types` (or a sibling module). `QAState` MUST contain at minimum: `question: str`, `originating_station_id: str | None`, `session_id: str`, `messages: list[Message]`, `step_count: int`, `add_to_kb_session_count: int`, `add_to_kb_question_count: int`. `QAAction` MUST mirror the existing `ExplorerAction` shape (`thought: str`, `tool_calls: list[ToolCall]`) so it can flow through `_think` without changes. `QAAnswer` MUST contain `answer: str` and `citations: list[KBCitation]` where `KBCitation` carries `file_path`, `line_start`, `line_end`, `related_stations`.

All three models MUST round-trip through `model_dump_json` / `model_validate_json` without data loss.

#### Scenario: QAState round-trip

- **WHEN** a populated `QAState` is dumped and re-validated
- **THEN** every scalar and list field MUST be byte-equal after the round-trip

#### Scenario: QAAction is structurally compatible with ExplorerAction

- **WHEN** `_think(state, provider, response_model=QAAction)` is invoked
- **THEN** the call MUST succeed without raising — verified by the same test seam used for Explorer

<!-- @trace
source: module-8-qa-p0
updated: 2026-04-26
code:
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/types.py
  - docs/sidecar-api.md
  - docs/decisions.md
  - sidecar/src/codebus_agent/agent/qa.py
  - sidecar/src/codebus_agent/agent/prompts/__init__.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/agent/reasoning_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/agent/tools/qa_tools.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/__init__.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/prompts/qa.py
tests:
  - sidecar/tests/agent/tools/test_kb_search.py
  - sidecar/tests/kb/test_upsert_chunk.py
  - sidecar/tests/api/test_qa_sse_events.py
  - sidecar/tests/agent/test_qa_types.py
  - sidecar/tests/api/test_task_id_qa_kind.py
  - sidecar/tests/agent/tools/test_qa_tools.py
  - sidecar/tests/integration/__init__.py
  - sidecar/tests/kb/test_query_filter_stations.py
  - sidecar/tests/agent/test_qa_prompts.py
  - sidecar/tests/agent/test_hits_confident.py
  - sidecar/tests/agent/test_run_qa.py
  - sidecar/tests/api/test_audit_paths_kb_growth.py
  - sidecar/tests/kb/test_growth_logger.py
  - sidecar/tests/api/test_qa_endpoint.py
  - sidecar/tests/integration/test_qa_end_to_end.py
  - sidecar/tests/agent/test_qa_budget_constants.py
  - sidecar/tests/agent/tools/test_add_to_kb.py
  - sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py
-->
