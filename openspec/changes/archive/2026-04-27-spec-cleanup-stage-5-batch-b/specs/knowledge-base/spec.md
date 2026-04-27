## MODIFIED Requirements

### Requirement: Embedding batch pipeline with UsageTracker wiring

The builder SHALL group chunks into batches of 32 and SHALL submit at most three batches to `provider.embed()` concurrently, enforced by an `asyncio.Semaphore(3)`. Each batch's `EmbedResponse.usage` SHALL be recorded into `<workspace_root>/.codebus/token_usage.jsonl` exactly once per batch via the **`TrackedProvider` automatic recording path**: when the bound provider is a `TrackedProvider` constructed with `default_module="kb_build"`, the wrapper writes the line on every successful `embed()` return, and `KnowledgeBase.build` MUST NOT call `usage_tracker.record(...)` itself. This invariant is the load-bearing rule of `usage-tracker-dedup` (archive 2026-04-23) — a manual `tracker.record(...)` from the builder would produce two lines per batch (one from the wrapper, one from the builder), breaking the dedup contract that "every embed line MUST equal one batch".

When a single chunk exceeds the provider's declared maximum input token count, the builder MUST split the chunk into halves and retry; when the halved chunk still exceeds the limit, the builder MUST skip it, emit a warning into `KBStats.warnings`, and MUST NOT raise.

#### Scenario: Batch size capped at 32

- **WHEN** the builder processes 100 chunks against a provider that records every `embed()` call
- **THEN** the provider MUST receive exactly 4 calls (32, 32, 32, 4) and no batch MUST exceed 32 entries

#### Scenario: Concurrency capped at 3 in-flight batches

- **WHEN** the builder runs against a provider whose `embed()` blocks until released and 10 batches are queued
- **THEN** at most 3 `embed()` invocations MUST be concurrently in-flight at any moment

#### Scenario: UsageTracker records exactly one entry per batch via TrackedProvider only

- **WHEN** the builder processes 64 chunks (two batches) through a `TrackedProvider` whose `default_module="kb_build"` and whose inner provider returns non-zero `usage`
- **THEN** the `<workspace_root>/.codebus/token_usage.jsonl` file MUST gain exactly two lines whose `operation == "embed"` and `module == "kb_build"`
- **AND** `KnowledgeBase.build` MUST NOT itself invoke `usage_tracker.record(...)` (the wrapper is the single writer; the dedup contract from `usage-tracker-dedup` requires `len(embed_lines) == KBStats.batches_embedded`)
- **AND** the sum of recorded `prompt_tokens` MUST equal the provider's reported total

#### Scenario: Oversized chunk split then skipped

- **WHEN** a chunk's token count exceeds the provider's max input even after halving
- **THEN** the chunk MUST be skipped, `KBStats.warnings` MUST gain an entry naming the offending file path, and the build MUST complete without raising

---

### Requirement: KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path

The sidecar SHALL expose `KnowledgeBase.upsert_chunk(text: str, *, payload: KBPayload) -> tuple[str, str]` as a public coroutine method on `KnowledgeBase`. The first element of the returned tuple is an `outcome` literal drawn from the closed set `{"new", "dedup_hash", "dedup_sim"}`; the second element is the Qdrant `point_id` (real UUID string for both new writes and dedup-skipped writes — the existing point's id when dedup matches). The method MUST execute the following steps in order:

1. **Layer 1 hash dedup** — call `backend.exists_by_hash(self._collection_name, payload.text_hash)`. When the hash already exists in Qdrant, the method MUST look up the existing point's id (e.g. via `backend.search_points` filtered on `text_hash`) and return `("dedup_hash", <existing_point_id>)`. The method MUST NOT call `provider.embed`, MUST NOT issue a Qdrant upsert, and MUST NOT append a `token_usage.jsonl` line for the skipped embedding.
2. **Embed once** — call `provider.embed([text])` exactly once. The bound provider's `default_module` SHALL be the value used for the surrounding query path (e.g. `"kb_query"` when the `KnowledgeBase` instance is constructed with the Q&A query provider — `app.state.kb_query_provider` factory in production wiring) so cost accounting flows through the existing `TrackedProvider` chain without per-call plumbing. Q&A `add_to_kb` shares the `kb_query` lane because both the query embed and the `add_to_kb` embed are downstream of the Q&A endpoint, and the chat-side cost is already separately accounted under `default_module="qa_agent"` by the `llm_qa_provider` factory.
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

#### Scenario: Embed lane is kb_query when called from the Q&A pipeline

- **WHEN** `upsert_chunk` is invoked through the Q&A `add_to_kb` pipeline whose `KnowledgeBase` instance was constructed with the `app.state.kb_query_provider` factory's TrackedProvider
- **THEN** the embed call MUST land in `<workspace_root>/.codebus/token_usage.jsonl` with `module == "kb_query"` (NOT `"qa_agent"`)
- **AND** the chat-side Q&A reasoning cost MUST stay separately accounted under `module == "qa_agent"` via the `llm_qa_provider` factory — the two lanes MUST NOT collapse into one
