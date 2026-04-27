# kb-growth Specification

## Purpose

TBD - created by archiving change 'module-8-qa-p0'. Update Purpose after archive.

## Requirements

### Requirement: KBGrowthLogger writes kb_growth.jsonl

The sidecar SHALL implement a `KBGrowthLogger` class in `codebus_agent.kb.growth_logger` that appends one JSON line per Q&A `add_to_kb` outcome to `<workspace>/.codebus/kb_growth.jsonl`, per `docs/decisions.md` D-016 and `docs/qa-agent.md §六`. The path lives under the `.codebus/` subdirectory of the workspace root, consistent with the workspace-level audit chain convention shared by `<workspace>/.codebus/sanitize_audit.jsonl`, `<workspace>/.codebus/tool_audit.jsonl`, `<workspace>/.codebus/reasoning_log.jsonl`, `<workspace>/.codebus/token_usage.jsonl`, and `<workspace>/.codebus/llm_calls.jsonl`. The constructor MUST auto-create the parent `.codebus/` directory if absent so callers do not have to pre-mkdir.

The class SHALL expose exactly one public method `write(*, point_id: str, source: str, reason: str, related_stations: list[str], originating_station_id: str | None, sanitize_stats: dict[str, int], chunk_size_chars: int, dedup_skipped: bool, session_id: str, question: str | None) -> None`. Every keyword argument MUST be required at the type-checking level; positional arguments MUST be rejected at runtime by the function signature.

`KBGrowthLogger` MUST be the only writer to `<workspace>/.codebus/kb_growth.jsonl` in the production codebase. No other module SHALL open this path for writing.

#### Scenario: Constructor auto-creates .codebus parent

- **WHEN** `KBGrowthLogger(<workspace>/.codebus/kb_growth.jsonl)` is constructed against a workspace where `.codebus/` does not yet exist
- **THEN** the directory MUST be created automatically
- **AND** the file itself MUST NOT be created until the first `write(...)` call

#### Scenario: One line per write

- **WHEN** `write(...)` is called once with a valid kwarg set
- **THEN** exactly one new line MUST be appended to `<workspace>/.codebus/kb_growth.jsonl`
- **AND** the line MUST be valid JSON terminated by `\n`

#### Scenario: Single source of truth for the path

- **WHEN** the codebase is grepped for `kb_growth.jsonl` in `sidecar/src/`
- **THEN** the only writing site (open with mode `"a"` / `"ab"` or equivalent) MUST be inside `codebus_agent.kb.growth_logger`


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
### Requirement: Required fields on every kb_growth.jsonl line

Each line written by `KBGrowthLogger.write` SHALL contain the following keys with non-null values: `ts` (ISO 8601 UTC timestamp), `session_id` (string), `question` (string or `null`), `originating_station_id` (string or `null`), `entry_id` (string — the **real** Qdrant `point_id` returned by `KnowledgeBase.upsert_chunk`; for both new writes and dedup-skipped writes, the value MUST be the real existing point id reported by `upsert_chunk`'s tuple return — see `knowledge-base` capability `KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path`. The `entry_id` MUST NOT carry sentinel prefixes such as `"dedup:hash"` or `"dedup:sim"`), `source` (string in `path:line_start-line_end` form), `related_stations` (list of strings, possibly empty), `reason` (string), `sanitize_stats` (mapping of string to non-negative integer), `chunk_size_chars` (non-negative integer reflecting post-Sanitize length), `dedup_skipped` (boolean — `true` when caller observed `outcome ∈ {"dedup_hash", "dedup_sim"}` from `upsert_chunk`, `false` when `outcome == "new"`), and `event_type` (literal string — see Requirement `Event type field defaults to "add" with rollback reserved for P1`).

Stable station ids in `related_stations` MUST match `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$`. Invalid ids MUST cause `KBGrowthLogger.write` to raise `ValueError` before any disk write, so the audit chain never persists malformed station references. The regex pattern MUST be sourced from the canonical leaf module `codebus_agent.agent.station_id.STATION_ID_RE`; `KBGrowthLogger` MUST NOT redeclare its own copy of the regex literal. This rule extends the single-source contract that `audit-path-unification-stage-2` establishes across all six station-id-validating modules (`agent.tools.add_to_kb`, `agent.tools.kb_search`, `kb.growth_logger`, `kb.knowledge_base`, `kb.payload`, `api.qa`).

#### Scenario: All required keys present

- **WHEN** any line from `<workspace>/.codebus/kb_growth.jsonl` is parsed as JSON
- **THEN** the parsed object MUST contain all of: `ts`, `session_id`, `question`, `originating_station_id`, `entry_id`, `source`, `related_stations`, `reason`, `sanitize_stats`, `chunk_size_chars`, `dedup_skipped`, `event_type`

#### Scenario: ts is ISO 8601 with UTC suffix

- **WHEN** any line is parsed
- **THEN** the `ts` field MUST match the regex `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$`

#### Scenario: Invalid station id rejected pre-write

- **WHEN** `write(... related_stations=["s9-bad"], ...)` is invoked (single-digit segment violates the regex)
- **THEN** the call MUST raise `ValueError` referencing the offending id
- **AND** no line MUST be appended to disk

#### Scenario: Dedup-skipped write records existing point id

- **WHEN** `KBGrowthLogger.write(...)` is invoked for a chunk whose `KnowledgeBase.upsert_chunk` returned `("dedup_hash", <existing_point_id>)` or `("dedup_sim", <existing_point_id>)`
- **THEN** the resulting `kb_growth.jsonl` line's `entry_id` field MUST equal `<existing_point_id>` (the real Qdrant point id reported by the dedup match)
- **AND** `entry_id` MUST NOT start with the literal string `"dedup:"`
- **AND** `dedup_skipped` MUST be `true`
- **AND** the line MUST still be appended to disk (dedup-skipped writes are still audited; only the `dedup_skipped=true` flag distinguishes them from new writes)

#### Scenario: New write records new point id

- **WHEN** `KBGrowthLogger.write(...)` is invoked for a chunk whose `KnowledgeBase.upsert_chunk` returned `("new", <new_point_id>)`
- **THEN** the resulting `kb_growth.jsonl` line's `entry_id` field MUST equal `<new_point_id>`
- **AND** `dedup_skipped` MUST be `false`
- **AND** `entry_id` MUST be a syntactically valid Qdrant point id (UUID-formatted string)

#### Scenario: Station id regex sourced from canonical leaf module

- **WHEN** `KBGrowthLogger`'s station-id pre-validation runs
- **THEN** the `re.Pattern` object used MUST be the same Python object as `codebus_agent.agent.station_id.STATION_ID_RE` (identity check via `is`)
- **AND** the `kb.growth_logger` module MUST NOT contain its own `re.compile(r"^s\d{2}-...")` call
- **AND** any defensive test that imports both `codebus_agent.kb.growth_logger.STATION_ID_RE` (if exposed as alias) and `codebus_agent.agent.station_id.STATION_ID_RE` MUST observe `is`-identity equality


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
### Requirement: Event type field defaults to "add" with rollback reserved for P1

Each line written by the P0 `KBGrowthLogger.write` method SHALL set `event_type` to the literal string `"add"`. The schema MUST reserve the value `"rollback"` for a future P1 change that introduces user-driven KB rollback (per `docs/qa-agent.md §六` rollback section), but the P0 writer MUST NOT expose a parameter or code path that produces `event_type="rollback"`.

A future change introducing rollback MUST extend `write(...)` with an additional keyword-only `event_type: Literal["add", "rollback"] = "add"` parameter; the schema deliberately uses an explicit field (not a discriminator-by-shape) so back-compatible reads of P0 lines remain trivial.

#### Scenario: P0 always writes event_type "add"

- **WHEN** `KBGrowthLogger.write(...)` is invoked through any P0 code path
- **THEN** the resulting line's `event_type` field MUST equal the literal string `"add"`

#### Scenario: P0 writer does not accept event_type kwarg

- **WHEN** `inspect.signature(KBGrowthLogger.write).parameters` is inspected in the P0 codebase
- **THEN** `event_type` MUST NOT appear among the parameter names — the field is internally hard-coded so a caller cannot drift the audit semantic


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
### Requirement: kb_growth_logger_factory wired into app.state

The sidecar SHALL expose `app.state.kb_growth_logger_factory: Callable[[Path], KBGrowthLogger]` populated by `wire_kb_dependencies(...)` whenever `openai_api_key` is set (the same precondition that populates `kb_provider`, `kb_query_provider`, and `kb_usage_tracker`). When `openai_api_key` is absent, `app.state.kb_growth_logger_factory` MUST be `None`, mirroring the other workspace-scoped KB factories.

The factory return value MUST resolve `<workspace_root>/.codebus/kb_growth.jsonl` and pass it to `KBGrowthLogger(...)`. The factory MUST NOT cache instances across workspaces — each call returns a fresh `KBGrowthLogger` for the supplied workspace path.

#### Scenario: Factory wired alongside other KB factories

- **WHEN** `wire_kb_dependencies(app, openai_api_key="sk-...", qdrant_url="http://...")` runs
- **THEN** `app.state.kb_growth_logger_factory` MUST be a callable
- **AND** `app.state.kb_growth_logger_factory(<ws>)` MUST return an instance of `KBGrowthLogger` whose internal path equals `<ws>/.codebus/kb_growth.jsonl`

#### Scenario: Factory absent when KB unconfigured

- **WHEN** `wire_kb_dependencies(app, openai_api_key=None, qdrant_url=None)` runs
- **THEN** `app.state.kb_growth_logger_factory` MUST be `None`


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
### Requirement: kb_growth.jsonl path constant lives alongside other audit filenames

The sidecar SHALL declare the module-level constant `_KB_GROWTH_FILENAME = "kb_growth.jsonl"` at exactly one canonical location: `codebus_agent/_audit_paths.py` (the **package-root leaf module**, established by `audit-path-unification` archive 2026-04-25 to break a three-way circular import among `api/__init__.py` ↔ `api/generate.py` ↔ `generator/runner.py`). The legacy location `codebus_agent/api/_audit_paths.py` MUST remain as a backward-compat re-export shim that imports the seven path constants from `codebus_agent._audit_paths` and re-exports them under the same names; existing call sites MAY import from either path and MUST receive the same Python string object via identity (`is`) check.

The constant MUST be importable as `from codebus_agent._audit_paths import _KB_GROWTH_FILENAME` (canonical) or `from codebus_agent.api._audit_paths import _KB_GROWTH_FILENAME` (backward-compat). Any factory that constructs a `KBGrowthLogger` MUST resolve its path using this constant rather than embedding the literal string `"kb_growth.jsonl"`.

#### Scenario: Constant exported from canonical leaf module

- **WHEN** `from codebus_agent._audit_paths import _KB_GROWTH_FILENAME` is executed
- **THEN** the import MUST succeed and the value MUST equal the literal string `"kb_growth.jsonl"`

#### Scenario: Backward-compat shim re-exports same object

- **WHEN** `from codebus_agent.api._audit_paths import _KB_GROWTH_FILENAME` is executed
- **THEN** the import MUST succeed
- **AND** the imported value MUST be the same Python string object as the one imported from `codebus_agent._audit_paths` (identity check via `is`)

#### Scenario: No literal "kb_growth.jsonl" outside the canonical leaf module

- **WHEN** `sidecar/src/codebus_agent/` is grepped for the quoted-string literal `"kb_growth.jsonl"` (the string surrounded by double or single quotes)
- **THEN** the only match MUST be inside `codebus_agent/_audit_paths.py` (the canonical leaf module)
- **AND** `codebus_agent/api/_audit_paths.py` MUST contain only an `import` line for `_KB_GROWTH_FILENAME` and an `__all__` listing — it MUST NOT contain a freshly declared `_KB_GROWTH_FILENAME = "kb_growth.jsonl"` literal of its own

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
