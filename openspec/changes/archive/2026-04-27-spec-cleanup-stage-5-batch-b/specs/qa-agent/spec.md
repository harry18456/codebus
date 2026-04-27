## MODIFIED Requirements

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
