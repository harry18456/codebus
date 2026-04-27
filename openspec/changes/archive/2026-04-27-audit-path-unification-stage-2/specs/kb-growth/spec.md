## MODIFIED Requirements

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
