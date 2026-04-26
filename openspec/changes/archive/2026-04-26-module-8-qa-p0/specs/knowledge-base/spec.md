## MODIFIED Requirements

### Requirement: KnowledgeBase query and find_similar API

The sidecar SHALL expose `KnowledgeBase.query(text: str, *, top_k: int = 8, filter_path: str | None = None, filter_source_kind: list[str] | None = None, filter_stations: list[str] | None = None) -> list[KBHit]` and `KnowledgeBase.find_similar(text: str, *, threshold: float = 0.95) -> KBHit | None`. `query` MUST embed `text` via the bound provider, search the workspace collection, and return hits ordered by descending score. `find_similar` MUST call `query(text, top_k=1)` internally and MUST return `None` when the top hit's score is strictly less than `threshold`.

`filter_stations`, when not `None`, SHALL restrict results to chunks whose `payload.related_stations` contains at least one of the supplied stable station ids. The Qdrant filter expression MUST be a `should` clause that matches any element in the supplied list against the indexed `related_stations` keyword field — i.e., the filter is logically OR over the supplied ids, not AND. Each entry in `filter_stations` MUST match `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$`; any non-matching value MUST cause `query` to raise `ValueError` before any embedding or Qdrant call. An empty list (`filter_stations=[]`) MUST be treated identically to `filter_stations=None` — no station restriction is applied — so callers can normalize a missing-vs-empty distinction without conditional branches.

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

## ADDED Requirements

### Requirement: KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path

The sidecar SHALL expose `KnowledgeBase.upsert_chunk(text: str, *, payload: KBPayload) -> str` as a public coroutine method on `KnowledgeBase`. The method MUST execute the following steps in order:

1. **Layer 1 hash dedup** — call `backend.exists_by_hash(self._collection_name, payload.text_hash)`. When the hash already exists in Qdrant, the method MUST return the literal string `"dedup:hash"` and MUST NOT call `provider.embed`, MUST NOT issue a Qdrant upsert, and MUST NOT append a `token_usage.jsonl` line for the skipped embedding.
2. **Embed once** — call `provider.embed([text])` exactly once. The bound provider's `default_module` SHALL be the same value used for the surrounding query path (e.g. `"qa_agent"` when the `KnowledgeBase` instance is constructed with the Q&A query provider) so cost accounting flows through the existing `TrackedProvider` chain without per-call plumbing.
3. **Layer 2 similarity dedup** — invoke `find_similar(text, threshold=0.95)` (which internally calls `query(text, top_k=1)` against the freshly-embedded vector path) and inspect the result. When the returned `KBHit` is non-`None` and its `score >= 0.95`, the method MUST return the literal string `"dedup:sim"` without issuing a Qdrant upsert.
4. **Upsert** — when neither dedup layer matches, persist the chunk as a single Qdrant point with the supplied `payload` and the just-computed embedding. The method MUST return the new `point_id` as a string.

The returned string MUST start with `"dedup:"` exactly when no upsert occurred. Callers (notably `add_to_kb`) MUST be able to rely on this prefix to distinguish dedup outcomes from new-point ids without parsing further.

`upsert_chunk` MUST NOT bypass the `payload` validation that `KBPayload` already enforces; in particular, an invalid `related_stations` id MUST surface as the same `pydantic.ValidationError` raised by `KBPayload` construction at the call site, rather than being silently suppressed inside `upsert_chunk`.

#### Scenario: Hash dedup short-circuits before embed

- **WHEN** `upsert_chunk("hello", payload=<payload with already-present text_hash>)` is invoked
- **THEN** the return value MUST equal the literal string `"dedup:hash"`
- **AND** `provider.embed` MUST NOT be called
- **AND** no Qdrant upsert MUST be issued

#### Scenario: Similarity dedup short-circuits after embed

- **WHEN** `upsert_chunk("hello rephrased", payload=<payload>)` is invoked, the hash is novel, but the freshly-embedded vector matches an existing point with score `0.97`
- **THEN** the return value MUST equal the literal string `"dedup:sim"`
- **AND** `provider.embed` MUST be called exactly once
- **AND** no Qdrant upsert MUST be issued

#### Scenario: New chunk yields point id

- **WHEN** `upsert_chunk(text, payload=<payload>)` is invoked with novel hash AND no similar existing chunk
- **THEN** the return value MUST be a non-empty string that does NOT start with `"dedup:"`
- **AND** the same value MUST be retrievable as the Qdrant `point_id` of the persisted point

#### Scenario: Dedup token format reserved

- **WHEN** any test reads the return value of `upsert_chunk`
- **THEN** strings starting with `"dedup:"` MUST be drawn from the closed set `{"dedup:hash", "dedup:sim"}` — no other dedup variant MUST be returned by P0 production code
