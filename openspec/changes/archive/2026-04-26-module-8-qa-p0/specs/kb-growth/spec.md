## ADDED Requirements

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

### Requirement: Required fields on every kb_growth.jsonl line

Each line written by `KBGrowthLogger.write` SHALL contain the following keys with non-null values: `ts` (ISO 8601 UTC timestamp), `session_id` (string), `question` (string or `null`), `originating_station_id` (string or `null`), `entry_id` (string — the Qdrant `point_id` returned by `KnowledgeBase.upsert_chunk` for non-dedup writes; for dedup-skipped writes, the existing point id reported by the dedup match), `source` (string in `path:line_start-line_end` form), `related_stations` (list of strings, possibly empty), `reason` (string), `sanitize_stats` (mapping of string to non-negative integer), `chunk_size_chars` (non-negative integer reflecting post-Sanitize length), `dedup_skipped` (boolean), and `event_type` (literal string — see Requirement `Event type field defaults to "add" with rollback reserved for P1`).

Stable station ids in `related_stations` MUST match `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$`. Invalid ids MUST cause `KBGrowthLogger.write` to raise `ValueError` before any disk write, so the audit chain never persists malformed station references.

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

### Requirement: Event type field defaults to "add" with rollback reserved for P1

Each line written by the P0 `KBGrowthLogger.write` method SHALL set `event_type` to the literal string `"add"`. The schema MUST reserve the value `"rollback"` for a future P1 change that introduces user-driven KB rollback (per `docs/qa-agent.md §六` rollback section), but the P0 writer MUST NOT expose a parameter or code path that produces `event_type="rollback"`.

A future change introducing rollback MUST extend `write(...)` with an additional keyword-only `event_type: Literal["add", "rollback"] = "add"` parameter; the schema deliberately uses an explicit field (not a discriminator-by-shape) so back-compatible reads of P0 lines remain trivial.

#### Scenario: P0 always writes event_type "add"

- **WHEN** `KBGrowthLogger.write(...)` is invoked through any P0 code path
- **THEN** the resulting line's `event_type` field MUST equal the literal string `"add"`

#### Scenario: P0 writer does not accept event_type kwarg

- **WHEN** `inspect.signature(KBGrowthLogger.write).parameters` is inspected in the P0 codebase
- **THEN** `event_type` MUST NOT appear among the parameter names — the field is internally hard-coded so a caller cannot drift the audit semantic

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

### Requirement: kb_growth.jsonl path constant lives alongside other audit filenames

The sidecar SHALL declare a module-level constant `_KB_GROWTH_FILENAME = "kb_growth.jsonl"` in `codebus_agent.api._audit_paths` so the path string is not duplicated across factories. The constant MUST be importable as `from codebus_agent.api._audit_paths import _KB_GROWTH_FILENAME`. Any factory that constructs a `KBGrowthLogger` MUST resolve its path using this constant rather than embedding the literal string.

#### Scenario: Constant exported from leaf module

- **WHEN** `from codebus_agent.api._audit_paths import _KB_GROWTH_FILENAME` is executed
- **THEN** the import MUST succeed and the value MUST equal the literal string `"kb_growth.jsonl"`

#### Scenario: No literal "kb_growth.jsonl" outside the leaf module

- **WHEN** `sidecar/src/codebus_agent/` is grepped for the literal string `"kb_growth.jsonl"`
- **THEN** the only match MUST be inside `codebus_agent/api/_audit_paths.py`
