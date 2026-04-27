# qa-agent Specification

## Purpose

TBD - created by archiving change 'module-8-qa-p0'. Update Purpose after archive.

## Requirements

### Requirement: Q&A loop entry point with two-stage RAG-first flow

The sidecar SHALL expose `codebus_agent.agent.qa.run_qa(*, question, state, kb, tools, provider, logger=None, emitter=None, cancel_event=None) -> QAAnswer` as the Q&A Agent entry point, per `docs/decisions.md` D-016 and `docs/qa-agent.md §四`. All parameters are keyword-only — the function signature MUST NOT accept any positional argument. The `provider` parameter is the workspace-scoped Q&A `TrackedProvider` instance (constructed via the `app.state.llm_qa_provider` factory with `default_module="qa_agent"`); `kb` carries its own embedding `TrackedProvider` constructed via `app.state.kb_query_provider` (`default_module="kb_query"`) so the two lanes write to `token_usage.jsonl` with distinct `module` values. Sanitizer / sanitizer-audit / kb-growth-logger plumbing MUST be threaded through `tools` (specifically the `QATools.add_to_kb` callee's bound `ToolContext`), NOT exposed as top-level `run_qa` parameters.

The function SHALL execute exactly three stages in order: (1) **RAG-first probe** — invoke `kb.query(question, top_k=8)` once and pass the hits through `_hits_confident(question, hits)`; (2) **Optional ReAct loop** — entered only when the probe returns `False`, reusing `codebus_agent.agent.explorer._think`, `_execute_tools`, and `_should_stop` from the existing ReAct core, bounded by the budget constants declared by this capability; (3) **Synthesize** — `_synthesize_answer(state, provider)` produces a final `QAAnswer` regardless of whether the loop ran.

`run_qa` MUST NOT instantiate `LLMJudge` or `LLMCoverageChecker`. The Q&A loop's only stop conditions are budget exhaustion (steps / tokens / wall) and explicit cancellation (signalled via the optional `cancel_event` keyword argument); station-coverage style verdicts are out of scope for Q&A. This isolation is the design surface that prevents Folder-mode prompt vocabulary from leaking into Q&A behavior.

The optional `logger: ReasoningLogger | None` parameter receives the workspace-scoped reasoning logger (constructed by the caller, typically `api/qa.py`, against `<ws>/.codebus/reasoning_log.jsonl`); when supplied, every ReAct iteration MUST flush one `Step` line through it. The optional `emitter: SSEEmitter | None` parameter receives the SSE emitter (typically `TaskHandleEmitter`) used for `rag_hits` / `agent_thought` / `agent_action_result` / `kb_growth` / `qa_answer` events.

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

#### Scenario: All run_qa parameters are keyword-only

- **WHEN** `inspect.signature(run_qa).parameters` is read
- **THEN** every parameter's `kind` MUST equal `inspect.Parameter.KEYWORD_ONLY`
- **AND** the parameter names MUST equal exactly `{"question", "state", "kb", "tools", "provider", "logger", "emitter", "cancel_event"}` — no extra `sanitizer` / `sanitizer_audit` / `kb_growth_logger` / `provider_factory` / `workspace_root` parameters are accepted

#### Scenario: cancel_event short-circuits the ReAct loop

- **WHEN** `run_qa` is invoked with a `cancel_event` whose `is_set()` becomes `True` mid-loop
- **THEN** `_should_stop(state)` MUST return `True` on the next iteration boundary
- **AND** `run_qa` MUST proceed directly to the Synthesize stage (no further `_think` invocation)
- **AND** the resulting `QAAnswer` MUST NOT raise — cancellation is a clean exit path, not an error


<!-- @trace
source: spec-cleanup-stage-5-batch-a
updated: 2026-04-27
code:
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_station_id_constant.py
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

All five constants MUST be the single source of truth for the entire Python package: any other module that needs to read them MUST `from codebus_agent.agent.qa import _QA_MAX_STEPS, ...` rather than redeclaring its own copy. In particular, `codebus_agent.agent.tools.add_to_kb` MUST NOT contain its own `_QA_MAX_*` declarations, and `codebus_agent.kb.knowledge_base` MUST NOT contain its own `_QA_DEDUP_THRESHOLD` declaration. The single-source contract is enforced by an identity-based defensive test (`assert other_module.<name> is qa.<name>`).

#### Scenario: Step limit honored via _should_stop

- **WHEN** `state.step_count` reaches `_QA_MAX_STEPS` during a Q&A run
- **THEN** the next `_should_stop(state)` call MUST return `True` and `run_qa` MUST exit the loop without further `_think` invocations

#### Scenario: Per-session add_to_kb limit refuses further writes

- **WHEN** `add_to_kb` has been invoked successfully `_QA_MAX_ADD_TO_KB_PER_SESSION` times in the current session
- **THEN** the next `add_to_kb` invocation MUST return a string starting with `"budget exhausted"` and MUST NOT call `kb.upsert_chunk` or `kb_growth_logger.write`

#### Scenario: Oversize chunk rejected without KB write

- **WHEN** a `chunks[*].text` post-Sanitize length is `2001` characters
- **THEN** that chunk MUST be skipped, the response MUST identify the rejection reason, and no `kb_growth.jsonl` line MUST be written for that chunk

#### Scenario: All callsites import constants from agent.qa single source

- **WHEN** any test imports `_QA_MAX_STEPS`, `_QA_MAX_ADD_TO_KB_PER_SESSION`, `_QA_MAX_CHUNK_SIZE_CHARS`, `_QA_MAX_ADD_TO_KB_PER_QUESTION`, or `_QA_DEDUP_THRESHOLD` from any non-`agent.qa` module that uses them
- **THEN** every imported value MUST be the same Python object as the one declared in `codebus_agent.agent.qa` (identity check via `is`)
- **AND** modules `codebus_agent.agent.tools.add_to_kb` and `codebus_agent.kb.knowledge_base` MUST NOT contain their own `_QA_MAX_*` or `_QA_DEDUP_THRESHOLD = ...` declarations
- **AND** when a defensive test grep scans `sidecar/src/codebus_agent/` for `^_QA_(MAX|DEDUP)_` definitions (anchored line-start), the only file containing such declarations MUST be `codebus_agent/agent/qa.py`


<!-- @trace
source: audit-path-unification-stage-2
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/qa.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/kb/payload.py
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/api/scan.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
tests:
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
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

- Five reused read tools delegating to `FolderTools` semantics: `search`, `list_dir`, `read_file`, `trace_import`, `find_callers` — each with `audit_fields` whose value MUST equal the corresponding entry in `codebus_agent.agent.tools.folder_tools._AUDIT_FIELDS` (the canonical FolderTools whitelist). The concrete production values are: `search = []`, `list_dir = ["path"]`, `read_file = ["path", "line_range"]`, `trace_import = []`, `find_callers = []`. The empty-list cases (`search` / `trace_import` / `find_callers`) reflect that those tools' only argument is free-form Agent text (search keyword / symbol name) which would echo secrets if mirrored into `tool_audit.jsonl`; the structured-arg cases (`list_dir` / `read_file`) record `path` (and `line_range` for `read_file`) because those are bounded enums of workspace-relative paths, not free-form user content.
- `kb_search(args: KBSearchArgs, ctx: ToolContext) -> str` with `audit_fields = ["query", "top_k", "station_filter"]`
- `add_to_kb(args: AddToKBArgs, ctx: ToolContext) -> str` with `audit_fields = ["source", "reason", "related_stations"]`

`add_to_kb` `audit_fields` MUST NOT include `chunks` (each chunk's `text` is sanitized and its replacements are recorded in `sanitize_audit.jsonl`; replicating chunk text into `tool_audit.jsonl` would create a parallel audit surface). `kb_search` `audit_fields` MAY include `query` because the search string is part of the Agent's decision trace, not pre-sanitize user content.

#### Scenario: All seven tools declare audit_fields

- **WHEN** `QATools` is registered with `ToolSandbox`
- **THEN** registration MUST succeed without raising — meaning all seven tools have `audit_fields` declared as `list[str]`

#### Scenario: Reused read tools mirror FolderTools audit_fields exactly

- **WHEN** the `audit_fields` attribute of `QATools.search`, `QATools.list_dir`, `QATools.read_file`, `QATools.trace_import`, and `QATools.find_callers` is read
- **THEN** each MUST equal the corresponding entry in `codebus_agent.agent.tools.folder_tools._AUDIT_FIELDS` by value-equality
- **AND** the concrete required values MUST be `QATools.search.audit_fields == []`, `QATools.list_dir.audit_fields == ["path"]`, `QATools.read_file.audit_fields == ["path", "line_range"]`, `QATools.trace_import.audit_fields == []`, `QATools.find_callers.audit_fields == []`
- **AND** none MUST be `None`, missing, or any non-list type

#### Scenario: add_to_kb audit_fields excludes chunks

- **WHEN** the `audit_fields` attribute of `QATools.add_to_kb` is inspected
- **THEN** the list MUST NOT contain the string `"chunks"`
- **AND** the list MUST contain at minimum `"source"`, `"reason"`, and `"related_stations"`


<!-- @trace
source: spec-cleanup-stage-5-batch-a
updated: 2026-04-27
code:
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_station_id_constant.py
-->

---
### Requirement: kb_search invokes KnowledgeBase query with optional station filter

The sidecar SHALL implement `kb_search(args: KBSearchArgs, ctx: ToolContext) -> str` such that it forwards the request to `ctx.kb.query(args.query, top_k=args.top_k, filter_stations=args.station_filter)`. `KBSearchArgs` MUST be a Pydantic model with fields `query: str`, `top_k: int = 5`, `station_filter: list[str] | None = None`. Each entry in `station_filter` MUST match `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$`; non-matching values MUST cause Pydantic validation error before the tool body runs.

The tool's return value MUST be a single multi-line string where each hit is rendered as `<file>:<line> | score=<X.XX> | stations=[<id>,...]\n  <snippet>`. When a hit's `payload.related_stations` is empty, the `stations=` segment MUST be omitted (not rendered as `stations=[]`). The snippet MUST be truncated to at most 200 characters and end with `…` (single Unicode horizontal ellipsis U+2026, NOT three ASCII dots `...`) when truncated. The 200-character ceiling MUST be sourced from `codebus_agent.agent.tools.kb_search._SNIPPET_TRUNCATE_LIMIT` rather than a literal `200` repeated at the call site, so future tweaks to the limit propagate without spec / code drift.

#### Scenario: station_filter forwarded to KB query

- **WHEN** `kb_search(KBSearchArgs(query="x", station_filter=["s02-storage"]))` is invoked
- **THEN** `ctx.kb.query` MUST be called with `filter_stations=["s02-storage"]`

#### Scenario: Invalid station id rejected by Pydantic

- **WHEN** `KBSearchArgs(query="x", station_filter=["invalid-id"])` is constructed
- **THEN** Pydantic MUST raise `ValidationError` referencing the regex constraint, before any KB call

#### Scenario: Hit rendering omits empty station list

- **WHEN** a returned hit has `payload.related_stations == []`
- **THEN** the rendered line MUST NOT contain the substring `stations=`

#### Scenario: Snippet truncates at 200 characters with ellipsis

- **WHEN** a returned hit's `payload.text` is longer than 200 characters
- **THEN** the rendered snippet portion MUST be exactly the first 200 characters of the snippet plus the single Unicode horizontal ellipsis character `…` (U+2026) appended at the end
- **AND** the rendered snippet MUST NOT contain three ASCII dots `...` as the truncation marker (the Unicode ellipsis is the canonical marker per `_SNIPPET_TRUNCATE_LIMIT` site)
- **AND** the truncation MUST source the limit from `codebus_agent.agent.tools.kb_search._SNIPPET_TRUNCATE_LIMIT` (currently `200`); the integer MUST NOT be hard-coded as a separate literal at the comparison site

#### Scenario: Snippet shorter than 200 characters left intact

- **WHEN** a returned hit's `payload.text` length is at most 200 characters
- **THEN** the rendered snippet MUST be the full text with NO trailing `…`
- **AND** no truncation MUST occur (the comparison is `len(snippet) > _SNIPPET_TRUNCATE_LIMIT`, strict greater-than)


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
### Requirement: add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order

The sidecar SHALL implement `add_to_kb(args: AddToKBArgs, ctx: ToolContext) -> str` whose body executes the pipeline in this fixed order, with no reordering permitted: (a) **Per-session and per-question budget check** runs first, before any sanitize, so audit lines do not record a redaction that did not actually happen; budget exhaustion MUST return a string starting with `"budget exhausted:"`. (b) **Pre-validate `related_stations` across ALL chunks in `args.chunks`** runs before any sanitize on any chunk; every id MUST match the canonical regex sourced from `codebus_agent.agent.station_id.STATION_ID_RE` (identity check via `is`); on any non-match, the entire `add_to_kb` invocation MUST return a string starting with `"invalid station_id:"` and naming the offending id, and NO chunk MUST be sanitized, upserted, or growth-logged (fail-fast pre-validate; the call is NOT transactional across chunks because no chunks were processed before the abort). (c) Then for each chunk in `args.chunks` in order, the per-chunk inner loop SHALL execute these steps:

1. **Sanitize** — `ctx.sanitizer.sanitize(chunk.text, source=FileSource(path=chunk.source, pass_="qa_add_to_kb"))`. Each `AuditEntry` returned MUST be appended to `ctx.sanitizer_audit` with `pass_num=3`. If the sanitized text strips to empty, the chunk MUST be skipped without further side effects (no KB upsert, no `kb_growth.jsonl` write); the per-chunk response token MUST be `"skipped_empty"`. If the sanitized text exceeds `_QA_MAX_CHUNK_SIZE_CHARS`, the chunk MUST be skipped with response token starting with `"skipped_oversize:"`.
2. **Upsert via KB layer** — `ctx.kb.upsert_chunk(text=clean, payload=KBPayload(...))` where `payload.added_by="qa_agent"`, `payload.session_id` reflects `ctx.session_id`, and `payload.related_stations` reflects the validated list.
3. **Growth log** — `ctx.kb_growth_logger.write(point_id=..., source=chunk.source, reason=args.reason, related_stations=chunk.related_stations, originating_station_id=ctx.originating_station_id, sanitize_stats=<derived>, chunk_size_chars=len(clean), dedup_skipped=<bool>, session_id=ctx.session_id, question=ctx.question)`. The growth log MUST be written even when `upsert_chunk` returns a dedup-skipped outcome (`outcome.startswith("dedup_")`), with `dedup_skipped=True`.

The Q&A pipeline enforces TWO independent budget ceilings on `add_to_kb`: a **per-session ceiling** (`_QA_MAX_ADD_TO_KB_PER_SESSION = 20`) capping the lifetime number of chunks any single Q&A session may add to the KB, and a **per-question ceiling** (`_QA_MAX_ADD_TO_KB_PER_QUESTION = 5`) capping the per-`question` chunks within that session. Both counters live on `QAState` (`add_to_kb_session_count` / `add_to_kb_question_count`), are sourced from the canonical single source `codebus_agent.agent.qa` module-level constants (identity check via `is`), and are checked together in step (a). Either ceiling exceeded MUST short-circuit with a `"budget exhausted:"` return value naming the violated lane (`per_session` or `per_question`); both ceilings are first-class and neither is a soft hint.

The `Validate` step explicitly does NOT live inside the per-chunk inner loop — it MUST run as a single pre-validate pass over all chunks before any sanitize on any chunk. This is the production behavior since `module-8-qa-p0` archive (2026-04-26) and ensures audit chains never record a redaction for a chunk the call ultimately rejects on a different chunk's invalid station id.

#### Scenario: Order of operations: budget, pre-validate, then per-chunk sanitize-upsert-log

- **WHEN** `add_to_kb` is invoked with three chunks (all with valid `related_stations`), the per-session budget is below the cap, and each chunk triggers a Pass 3 sanitize hit
- **THEN** the test recorder MUST observe these calls in this exact sequence: budget-check (no recorded call; internal counter read), then the three-chunk pre-validate pass (no `sanitizer.sanitize` calls yet), then for each chunk in order: `sanitizer.sanitize(chunk.text)`, `sanitizer_audit.append(... pass_num=3 ...)`, `kb.upsert_chunk(...)`, `kb_growth_logger.write(...)`
- **AND** no chunk MUST observe its sanitize call before any other chunk's pre-validate has completed

#### Scenario: Empty post-sanitize chunk skipped without KB or growth-log writes

- **WHEN** a chunk's post-sanitize text is empty (full redaction)
- **THEN** `kb.upsert_chunk` MUST NOT be called for that chunk
- **AND** `kb_growth_logger.write` MUST NOT be called for that chunk
- **AND** the per-chunk return token MUST be `"skipped_empty"`

#### Scenario: Dedup hit still records growth-log line with dedup_skipped=true

- **WHEN** `kb.upsert_chunk` returns `("dedup_hash", <existing_point_id>)` for a chunk
- **THEN** `kb_growth_logger.write` MUST be called with `dedup_skipped=True` and `point_id=<existing_point_id>` for that chunk
- **AND** the per-chunk return token MUST be `"dedup:hash"` (the legacy text token preserved for backward-compat with prior tests; the underlying `outcome` literal in the tuple is `"dedup_hash"`)

#### Scenario: Invalid station_id aborts the entire invocation before any sanitize

- **WHEN** `add_to_kb` is invoked with three chunks where the second chunk's `related_stations` contains the string `"s2-bad"` (single-digit segment violates the regex)
- **THEN** `add_to_kb` MUST return `"invalid station_id: s2-bad"` (or string starting with that prefix)
- **AND** `sanitizer.sanitize` MUST NOT be called for ANY chunk (not even chunk 0 or chunk 2)
- **AND** `kb.upsert_chunk` MUST NOT be called for ANY chunk
- **AND** `kb_growth_logger.write` MUST NOT be called for ANY chunk
- **AND** zero rows MUST be appended to `<workspace>/.codebus/sanitize_audit.jsonl` for this invocation

#### Scenario: Budget exhausted aborts before any sanitize

- **WHEN** `add_to_kb` is invoked while the per-session counter has already reached `_QA_MAX_ADD_TO_KB_PER_SESSION`
- **THEN** the return value MUST be a string starting with `"budget exhausted:"`
- **AND** `sanitizer.sanitize` MUST NOT be called for ANY chunk
- **AND** `kb.upsert_chunk` MUST NOT be called for ANY chunk
- **AND** zero rows MUST be appended to `<workspace>/.codebus/sanitize_audit.jsonl` for this invocation

#### Scenario: Per-question budget caps add_to_kb at five chunks

- **WHEN** `add_to_kb` is invoked while the per-question counter (`QAState.add_to_kb_question_count`) has already reached `_QA_MAX_ADD_TO_KB_PER_QUESTION = 5`
- **THEN** the return value MUST be a string starting with `"budget exhausted:"` and the wording MUST identify the per-question lane (e.g. naming `per_question` or `_QA_MAX_ADD_TO_KB_PER_QUESTION`) so callers can distinguish per-question from per-session exhaustion
- **AND** `sanitizer.sanitize` MUST NOT be called for ANY chunk
- **AND** `kb.upsert_chunk` MUST NOT be called for ANY chunk
- **AND** `kb_growth_logger.write` MUST NOT be called for ANY chunk
- **AND** zero rows MUST be appended to `<workspace>/.codebus/sanitize_audit.jsonl` for this invocation
- **AND** the per-question counter MUST reset to zero at the start of every new `question` within the same Q&A session, so the per-question ceiling is a per-`question` lane independent of the per-session lane

#### Scenario: Per-question and per-session ceilings sourced from canonical single source

- **WHEN** `add_to_kb`'s budget-check step reads either ceiling
- **THEN** the resolved value MUST be the same Python integer object as `codebus_agent.agent.qa._QA_MAX_ADD_TO_KB_PER_SESSION` / `codebus_agent.agent.qa._QA_MAX_ADD_TO_KB_PER_QUESTION` respectively (identity check via `is` per the existing `agent.qa` single-source convention)
- **AND** the `agent.tools.add_to_kb` module MUST NOT redeclare either constant locally

#### Scenario: Station id regex sourced from canonical leaf module

- **WHEN** `add_to_kb`'s station-id pre-validation runs against the supplied `chunks[*].related_stations`
- **THEN** the `re.Pattern` object used MUST be the same Python object as `codebus_agent.agent.station_id.STATION_ID_RE` (identity check via `is`)
- **AND** the `agent.tools.add_to_kb` module MUST NOT contain its own `re.compile(r"^s\d{2}-...")` call


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
### Requirement: Q&A run emits SSE events on the task channel

The sidecar SHALL emit the following SSE event types, in the listed order, when `run_qa` is driven by `POST /qa`'s background task wrapper:

- `rag_hits` — emitted exactly once after the initial `kb.query`. Payload MUST contain `hits: list[{score, file_path, line_start, line_end, snippet, related_stations}]` reflecting at most the top-8 probe results.
- `agent_thought` / `agent_action_result` — emitted by the reused ReAct core when (and only when) the loop body runs (i.e., `_hits_confident` returned `False`). The `agent_action_result` payload MUST include a `tokens_used: int` field; P0 implementation MAY emit `tokens_used: 0` as a placeholder until per-tool token attribution lands (currently `ToolResult` does not carry a `tokens_used` field; once it does, the emitter MUST forward the per-tool count). This invariant matches the explorer-sse capability's same-named contract — the placeholder convention is uniform across Explorer and Q&A so the Trust Layer R-01 panel renders both event streams with one schema.
- `kb_growth` — emitted exactly once per successful `add_to_kb` chunk that resulted in a new KB point. Payload MUST contain `entry_id`, `source`, `related_stations`, `originating_station_id`. Dedup-skipped chunks MUST NOT emit `kb_growth`.
- `qa_answer` — emitted exactly once at the end of `run_qa`. Payload MUST contain `answer: str` (the synthesized final answer text) and `citations: list[{file_path, line_start, line_end, related_stations}]`. P0 MUST treat `qa_answer` as a single-shot non-streaming event; field-level streaming is reserved for a later P1 change and MUST NOT alter the schema shape introduced here.
- `usage_delta` / `llm_call` — emitted by `TrackedProvider` per existing contracts when chat / embed calls fire. The Q&A run uses **two distinct lanes** that write to `token_usage.jsonl` with different `module` values: the **chat lane** carries `module="qa_agent"` (the `app.state.llm_qa_provider` factory wraps the provider with `default_module="qa_agent"`), and the **KB embedding lane** carries `module="kb_query"` (the `app.state.kb_query_provider` factory wraps the embedding provider with `default_module="kb_query"`). Both lanes are part of the Q&A run's accounting surface, but they MUST be distinguishable on `token_usage.jsonl` and `usage_delta` events by their `module` field — downstream cost-attribution consumers depend on the distinction.

Failures during the Q&A run MUST surface via the `error` event defined by `sidecar-runtime` (with `code="QA_FAILED"`); they MUST NOT emit a partial `qa_answer`.

#### Scenario: rag_hits event precedes any agent_thought

- **WHEN** `run_qa` is driven by `POST /qa` against an emitter
- **THEN** the sequence of emitted events MUST contain `rag_hits` strictly before any `agent_thought` event (if any `agent_thought` events are emitted)

#### Scenario: qa_answer is emitted exactly once at the end

- **WHEN** a Q&A run completes successfully (whether via cheap path or ReAct loop)
- **THEN** exactly one `qa_answer` event MUST be emitted, and it MUST be the last event before `done`

#### Scenario: Chat lane writes module="qa_agent" to token_usage.jsonl

- **WHEN** the Q&A run invokes `provider.chat(...)` through the `app.state.llm_qa_provider` factory's TrackedProvider
- **THEN** the resulting `<workspace>/.codebus/token_usage.jsonl` line MUST contain `"module": "qa_agent"`
- **AND** the corresponding `usage_delta` SSE event payload MUST carry `module="qa_agent"`

#### Scenario: KB embedding lane writes module="kb_query" to token_usage.jsonl

- **WHEN** the Q&A run invokes `kb.query(...)` which internally calls the embedding provider through the `app.state.kb_query_provider` factory's TrackedProvider
- **THEN** the resulting `<workspace>/.codebus/token_usage.jsonl` line MUST contain `"module": "kb_query"` (NOT `"qa_agent"`)
- **AND** the corresponding `usage_delta` SSE event payload MUST carry `module="kb_query"`

#### Scenario: Both lanes co-occur within one Q&A run

- **WHEN** a Q&A run executes both an embedding call (RAG probe) and at least one chat call (synthesize or ReAct loop)
- **THEN** `<workspace>/.codebus/token_usage.jsonl` MUST contain at least one line with `"module": "kb_query"` AND at least one line with `"module": "qa_agent"`
- **AND** no Q&A-run line MUST carry any other `module` value

#### Scenario: tokens_used field accepts P0 placeholder zero

- **WHEN** `agent_action_result` events are inspected from a P0-stage Q&A run (no `tokens_used` field on `ToolResult` yet)
- **THEN** the `tokens_used` field on each emitted event MUST be a non-negative integer
- **AND** consumers MUST treat the value `0` as valid placeholder semantics (`0` does NOT mean "no tokens used", it signals "attribution not yet wired") — uniform with the explorer-sse capability's same-named convention so Q&A and Explorer event consumers share one schema


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
