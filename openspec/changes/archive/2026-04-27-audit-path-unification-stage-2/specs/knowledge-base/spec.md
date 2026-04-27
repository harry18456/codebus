## MODIFIED Requirements

### Requirement: KnowledgeBase query and find_similar API

The sidecar SHALL expose `KnowledgeBase.query(text: str, *, top_k: int = 8, filter_path: str | None = None, filter_source_kind: list[str] | None = None, filter_stations: list[str] | None = None) -> list[KBHit]` and `KnowledgeBase.find_similar(text: str, *, threshold: float = 0.95) -> KBHit | None`. `query` MUST embed `text` via the bound provider, search the workspace collection, and return hits ordered by descending score. `find_similar` MUST call `query(text, top_k=1)` internally and MUST return `None` when the top hit's score is strictly less than `threshold`.

`filter_stations`, when not `None`, SHALL restrict results to chunks whose `payload.related_stations` contains at least one of the supplied stable station ids. The Qdrant filter expression MUST be a `should` clause that matches any element in the supplied list against the indexed `related_stations` keyword field — i.e., the filter is logically OR over the supplied ids, not AND. Each entry in `filter_stations` MUST match the canonical regex sourced from `codebus_agent.agent.station_id.STATION_ID_RE`; any non-matching value MUST cause `query` to raise `ValueError` before any embedding or Qdrant call. The `kb.knowledge_base` module MUST NOT redeclare its own copy of the regex literal — the `re.Pattern` object used for validation MUST be the same Python object as the canonical one (identity check via `is`). An empty list (`filter_stations=[]`) MUST be treated identically to `filter_stations=None` — no station restriction is applied — so callers can normalize a missing-vs-empty distinction without conditional branches.

#### Scenario: Query returns top_k hits ordered by score

- **WHEN** `query("Storage", top_k=3)` is called against a populated collection
- **THEN** the returned list MUST contain at most 3 `KBHit` entries and scores MUST be monotonically non-increasing

#### Scenario: filter_path restricts results

- **WHEN** `query("Storage", filter_path="src/storage/types.ts")` is called
- **THEN** every returned hit's `payload.file_path` MUST equal `"src/storage/types.ts"`

#### Scenario: filter_source_kind restricts results

- **WHEN** `query("x", filter_source_kind=["code"])` is called against a collection containing both `code` and `skeleton` payloads
- **THEN** no returned hit's `payload.source_kind` MUST be `"skeleton"`

#### Scenario: filter_stations restricts results to chunks tagged with any supplied id

- **WHEN** `query("x", filter_stations=["s02-storage-contract"])` is called against a collection where some chunks have `payload.related_stations` containing `"s02-storage-contract"` and others do not
- **THEN** every returned hit's `payload.related_stations` MUST contain `"s02-storage-contract"`

#### Scenario: filter_stations OR semantics across multiple ids

- **WHEN** `query("x", filter_stations=["s02-storage-contract", "s03-payment-flow"])` is called
- **THEN** every returned hit's `payload.related_stations` MUST contain `"s02-storage-contract"` OR `"s03-payment-flow"` (or both) — chunks tagged with either id MUST be eligible

#### Scenario: Empty filter_stations equivalent to None

- **WHEN** `query("x", filter_stations=[])` is called against the same collection where `query("x")` returns N hits
- **THEN** the returned hit set MUST be identical (same ids, same order) to the result of `query("x")` with no station filter

#### Scenario: Invalid station id raises before query

- **WHEN** `query("x", filter_stations=["bad-id"])` is called
- **THEN** the call MUST raise `ValueError` referencing the regex constraint
- **AND** no embedding API call MUST occur and no Qdrant search MUST be issued

#### Scenario: find_similar returns None below threshold

- **WHEN** `find_similar("rare query", threshold=0.95)` is called and the top hit's score is `0.80`
- **THEN** the return value MUST be `None`

#### Scenario: find_similar returns hit at or above threshold

- **WHEN** `find_similar("known text", threshold=0.95)` is called and the top hit's score is `0.96`
- **THEN** the return value MUST be a `KBHit` whose `score >= 0.95`

#### Scenario: Station id regex sourced from canonical leaf module

- **WHEN** `KnowledgeBase._validate_station_filter` (or equivalent internal helper) validates `filter_stations` entries
- **THEN** the `re.Pattern` object used MUST be the same Python object as `codebus_agent.agent.station_id.STATION_ID_RE` (identity check via `is`)
- **AND** the `kb.knowledge_base` module MUST NOT contain its own `re.compile(r"^s\d{2}-...")` call

---
### Requirement: KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path

The sidecar SHALL expose `KnowledgeBase.upsert_chunk(text: str, *, payload: KBPayload) -> tuple[str, str]` as a public coroutine method on `KnowledgeBase`. The first element of the returned tuple is an `outcome` literal drawn from the closed set `{"new", "dedup_hash", "dedup_sim"}`; the second element is the Qdrant `point_id` (real UUID string for both new writes and dedup-skipped writes — the existing point's id when dedup matches). The method MUST execute the following steps in order:

1. **Layer 1 hash dedup** — call `backend.exists_by_hash(self._collection_name, payload.text_hash)`. When the hash already exists in Qdrant, the method MUST look up the existing point's id (e.g. via `backend.search_points` filtered on `text_hash`) and return `("dedup_hash", <existing_point_id>)`. The method MUST NOT call `provider.embed`, MUST NOT issue a Qdrant upsert, and MUST NOT append a `token_usage.jsonl` line for the skipped embedding.
2. **Embed once** — call `provider.embed([text])` exactly once. The bound provider's `default_module` SHALL be the value used for the surrounding query path (e.g. `"qa_agent"` when the `KnowledgeBase` instance is constructed with the Q&A query provider) so cost accounting flows through the existing `TrackedProvider` chain without per-call plumbing.
3. **Layer 2 similarity dedup** — invoke `find_similar(text, threshold=_QA_DEDUP_THRESHOLD)` and inspect the result. The threshold value MUST be sourced from the canonical single source `codebus_agent.agent.qa._QA_DEDUP_THRESHOLD` (identity check via `is`); the `kb.knowledge_base` module MUST NOT redeclare a local `_QA_DEDUP_THRESHOLD` constant. When the returned `KBHit` is non-`None` and its `score >= _QA_DEDUP_THRESHOLD`, the method MUST return `("dedup_sim", <hit.point_id>)` without issuing a Qdrant upsert.
4. **Upsert** — when neither dedup layer matches, persist the chunk as a single Qdrant point with the supplied `payload` and the just-computed embedding. The method MUST return `("new", <new_point_id>)`.

The `outcome` literal is the canonical discriminator. Callers (notably `add_to_kb`) MUST destructure the tuple and rely on `outcome` to distinguish dedup-skipped writes from new writes. The `point_id` value MUST always be a non-empty UUID-formatted string and MUST NOT carry sentinel prefixes (e.g. it MUST NOT be `"dedup:hash"` or `"dedup:sim"`); both the new-point and dedup-skipped paths return the real Qdrant point id so downstream audit consumers (Trust Layer R-01 panel, `kb_growth.jsonl.entry_id`) can join back to a Qdrant point unambiguously.

`upsert_chunk` MUST NOT bypass the `payload` validation that `KBPayload` already enforces; in particular, an invalid `related_stations` id MUST surface as the same `pydantic.ValidationError` raised by `KBPayload` construction at the call site, rather than being silently suppressed inside `upsert_chunk`.

#### Scenario: Hash dedup short-circuits before embed

- **WHEN** `upsert_chunk("hello", payload=<payload with already-present text_hash>)` is invoked
- **THEN** the return value MUST be a tuple `(outcome, point_id)` where `outcome == "dedup_hash"`
- **AND** `point_id` MUST equal the existing Qdrant point's id (the same id as the originally-stored chunk that produced the matching hash)
- **AND** `provider.embed` MUST NOT be called
- **AND** no Qdrant upsert MUST be issued

#### Scenario: Similarity dedup short-circuits after embed

- **WHEN** `upsert_chunk("hello rephrased", payload=<payload>)` is invoked, the hash is novel, but the freshly-embedded vector matches an existing point with score `0.97`
- **THEN** the return value MUST be a tuple `(outcome, point_id)` where `outcome == "dedup_sim"`
- **AND** `point_id` MUST equal the matched point's `point_id` (i.e. `KBHit.point_id`)
- **AND** `provider.embed` MUST be called exactly once
- **AND** no Qdrant upsert MUST be issued

#### Scenario: New chunk yields outcome "new" with new point id

- **WHEN** `upsert_chunk(text, payload=<payload>)` is invoked with novel hash AND no similar existing chunk
- **THEN** the return value MUST be a tuple `(outcome, point_id)` where `outcome == "new"`
- **AND** `point_id` MUST be a non-empty UUID-formatted string
- **AND** the same `point_id` MUST be retrievable as the Qdrant point id of the persisted point

#### Scenario: Outcome literal closed set

- **WHEN** any test reads the `outcome` element of `upsert_chunk`'s return value
- **THEN** `outcome` MUST be drawn from the closed set `{"new", "dedup_hash", "dedup_sim"}` — no other variant MUST be returned by P0 production code

#### Scenario: point_id never carries dedup sentinel prefix

- **WHEN** any test reads the `point_id` element of `upsert_chunk`'s return value (across new / dedup_hash / dedup_sim outcomes)
- **THEN** `point_id` MUST NOT start with the literal string `"dedup:"`
- **AND** `point_id` MUST be a syntactically valid Qdrant point id (UUID-formatted string)

#### Scenario: Dedup threshold sourced from canonical single source

- **WHEN** any code path inside `kb.knowledge_base.upsert_chunk` references the dedup threshold
- **THEN** the resolved value MUST be the same Python object as `codebus_agent.agent.qa._QA_DEDUP_THRESHOLD` (identity check via `is`)
- **AND** the `kb.knowledge_base` module MUST NOT contain a local `_QA_DEDUP_THRESHOLD = 0.95` declaration
