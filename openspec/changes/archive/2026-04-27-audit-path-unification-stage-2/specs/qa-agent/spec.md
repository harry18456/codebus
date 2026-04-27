## MODIFIED Requirements

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

---
### Requirement: add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order

The sidecar SHALL implement `add_to_kb(args: AddToKBArgs, ctx: ToolContext) -> str` whose body executes the following steps for each chunk in `args.chunks`, in this order, with no reordering permitted:

1. **Sanitize** — `ctx.sanitizer.sanitize(chunk.text, source=FileSource(path=chunk.source, pass_="qa_add_to_kb"))`. Each `AuditEntry` returned MUST be appended to `ctx.sanitizer_audit` with `pass_num=3`. If the sanitized text strips to empty, the chunk MUST be skipped without further side effects (no KB upsert, no `kb_growth.jsonl` write); the per-chunk response token MUST be `"skipped_empty"`.
2. **Validate `related_stations`** — every id in `chunk.related_stations` MUST match the canonical regex sourced from `codebus_agent.agent.station_id.STATION_ID_RE`. The `agent.tools.add_to_kb` module MUST NOT redeclare its own copy of the regex literal — the `re.Pattern` object used for validation MUST be the same Python object as the canonical one (identity check via `is`). On any non-match, the entire `add_to_kb` invocation MUST return a string starting with `"invalid station_id:"` and naming the offending id; previously-processed chunks in the same invocation MUST remain committed (the call is not transactional across chunks).
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

#### Scenario: Station id regex sourced from canonical leaf module

- **WHEN** `add_to_kb`'s station-id pre-validation runs against the supplied `chunks[*].related_stations`
- **THEN** the `re.Pattern` object used MUST be the same Python object as `codebus_agent.agent.station_id.STATION_ID_RE` (identity check via `is`)
- **AND** the `agent.tools.add_to_kb` module MUST NOT contain its own `re.compile(r"^s\d{2}-...")` call
