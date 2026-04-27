## MODIFIED Requirements

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

---
### Requirement: add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order

The sidecar SHALL implement `add_to_kb(args: AddToKBArgs, ctx: ToolContext) -> str` whose body executes the pipeline in this fixed order, with no reordering permitted: (a) **Per-session and per-question budget check** runs first, before any sanitize, so audit lines do not record a redaction that did not actually happen; budget exhaustion MUST return a string starting with `"budget exhausted:"`. (b) **Pre-validate `related_stations` across ALL chunks in `args.chunks`** runs before any sanitize on any chunk; every id MUST match the canonical regex sourced from `codebus_agent.agent.station_id.STATION_ID_RE` (identity check via `is`); on any non-match, the entire `add_to_kb` invocation MUST return a string starting with `"invalid station_id:"` and naming the offending id, and NO chunk MUST be sanitized, upserted, or growth-logged (fail-fast pre-validate; the call is NOT transactional across chunks because no chunks were processed before the abort). (c) Then for each chunk in `args.chunks` in order, the per-chunk inner loop SHALL execute these steps:

1. **Sanitize** — `ctx.sanitizer.sanitize(chunk.text, source=FileSource(path=chunk.source, pass_="qa_add_to_kb"))`. Each `AuditEntry` returned MUST be appended to `ctx.sanitizer_audit` with `pass_num=3`. If the sanitized text strips to empty, the chunk MUST be skipped without further side effects (no KB upsert, no `kb_growth.jsonl` write); the per-chunk response token MUST be `"skipped_empty"`. If the sanitized text exceeds `_QA_MAX_CHUNK_SIZE_CHARS`, the chunk MUST be skipped with response token starting with `"skipped_oversize:"`.
2. **Upsert via KB layer** — `ctx.kb.upsert_chunk(text=clean, payload=KBPayload(...))` where `payload.added_by="qa_agent"`, `payload.session_id` reflects `ctx.session_id`, and `payload.related_stations` reflects the validated list.
3. **Growth log** — `ctx.kb_growth_logger.write(point_id=..., source=chunk.source, reason=args.reason, related_stations=chunk.related_stations, originating_station_id=ctx.originating_station_id, sanitize_stats=<derived>, chunk_size_chars=len(clean), dedup_skipped=<bool>, session_id=ctx.session_id, question=ctx.question)`. The growth log MUST be written even when `upsert_chunk` returns a dedup-skipped outcome (`outcome.startswith("dedup_")`), with `dedup_skipped=True`.

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

#### Scenario: Station id regex sourced from canonical leaf module

- **WHEN** `add_to_kb`'s station-id pre-validation runs against the supplied `chunks[*].related_stations`
- **THEN** the `re.Pattern` object used MUST be the same Python object as `codebus_agent.agent.station_id.STATION_ID_RE` (identity check via `is`)
- **AND** the `agent.tools.add_to_kb` module MUST NOT contain its own `re.compile(r"^s\d{2}-...")` call

---
### Requirement: Q&A run emits SSE events on the task channel

The sidecar SHALL emit the following SSE event types, in the listed order, when `run_qa` is driven by `POST /qa`'s background task wrapper:

- `rag_hits` — emitted exactly once after the initial `kb.query`. Payload MUST contain `hits: list[{score, file_path, line_start, line_end, snippet, related_stations}]` reflecting at most the top-8 probe results.
- `agent_thought` / `agent_action_result` — emitted by the reused ReAct core when (and only when) the loop body runs (i.e., `_hits_confident` returned `False`).
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
