## MODIFIED Requirements

### Requirement: KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path

The sidecar SHALL expose `KnowledgeBase.upsert_chunk(text: str, *, payload: KBPayload) -> tuple[str, str]` as a public coroutine method on `KnowledgeBase`. The first element of the returned tuple is an `outcome` literal drawn from the closed set `{"new", "dedup_hash", "dedup_sim"}`; the second element is the Qdrant `point_id` (real UUID string for both new writes and dedup-skipped writes — the existing point's id when dedup matches). The method MUST execute the following steps in order:

1. **Layer 1 hash dedup** — call `backend.exists_by_hash(self._collection_name, payload.text_hash)`. When the hash already exists in Qdrant, the method MUST look up the existing point's id (e.g. via `backend.search_points` filtered on `text_hash`) and return `("dedup_hash", <existing_point_id>)`. The method MUST NOT call `provider.embed`, MUST NOT issue a Qdrant upsert, and MUST NOT append a `token_usage.jsonl` line for the skipped embedding.
2. **Embed once** — call `provider.embed([text])` exactly once. The bound provider's `default_module` SHALL be the value used for the surrounding query path (e.g. `"qa_agent"` when the `KnowledgeBase` instance is constructed with the Q&A query provider) so cost accounting flows through the existing `TrackedProvider` chain without per-call plumbing.
3. **Layer 2 similarity dedup** — invoke `find_similar(text, threshold=0.95)` (which internally calls `query(text, top_k=1)`) and inspect the result. When the returned `KBHit` is non-`None` and its `score >= 0.95`, the method MUST return `("dedup_sim", <hit.point_id>)` without issuing a Qdrant upsert.
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
