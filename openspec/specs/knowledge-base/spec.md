# knowledge-base Specification

## Purpose

TBD - created by archiving change 'module-2-kb-builder-p0'. Update Purpose after archive.

## Requirements

### Requirement: KBPayload schema

The sidecar SHALL expose a Pydantic `KBPayload` model in `codebus_agent.kb.payload` whose fields mirror `docs/module-2-kb-builder.md §三` and whose values are validated on construction. The payload SHALL carry the following fields with the listed constraints: `source_kind` (Literal `"code" | "doc" | "git_commit" | "git_blame" | "skeleton"`), `file_path` (string or None), `line_start` / `line_end` (non-negative integers or None), `commit_oid` (string or None), `text` (string), `text_hash` (64-character lowercase hexadecimal SHA-256 digest), `language` (string or None), `added_by` (Literal `"scanner" | "qa_agent"`), `session_id` (string or None), `chunk_index` (non-negative integer), `chunk_total` (positive integer, `>= chunk_index + 1`), `created_at` (datetime), `source_mtime` (datetime or None), `sanitize_stats` (mapping of string to non-negative integer), and `related_stations` (list of strings). Every entry in `related_stations` MUST match the regex `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$`; any non-matching value MUST cause Pydantic `ValidationError` at construction time.

#### Scenario: Valid payload constructs without error

- **WHEN** `KBPayload` is instantiated with `source_kind="code"`, `text="hello"`, `text_hash=<sha256 of "hello">`, `added_by="scanner"`, `chunk_index=0`, `chunk_total=1`, and all other required fields populated with type-correct values
- **THEN** construction MUST succeed and `model_dump()` MUST round-trip through `model_validate()` without loss

#### Scenario: Invalid text_hash rejected

- **WHEN** `KBPayload` is instantiated with `text_hash="not-a-digest"` (wrong length or non-hex characters)
- **THEN** construction MUST raise `pydantic.ValidationError`

#### Scenario: Invalid related_stations id rejected

- **WHEN** `KBPayload` is instantiated with `related_stations=["s1-x"]` (station id missing the two-digit segment)
- **THEN** construction MUST raise `pydantic.ValidationError`

#### Scenario: chunk_total must cover chunk_index

- **WHEN** `KBPayload` is instantiated with `chunk_index=3, chunk_total=2`
- **THEN** construction MUST raise `pydantic.ValidationError`


<!-- @trace
source: module-2-kb-builder-p0
updated: 2026-04-21
code:
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - sidecar/src/codebus_agent/providers/usage_tracker.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/kb/__init__.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/chunker.py
tests:
  - sidecar/tests/kb/fixtures/sample-doc.md
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_strategy.py
  - sidecar/tests/kb/fixtures/sample-code.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/kb/fixtures/sample-plain.txt
  - sidecar/tests/kb/test_qdrant_kb.py
  - sidecar/tests/kb/test_chunker.py
  - sidecar/tests/kb/test_payload.py
-->

---
### Requirement: Token-window chunker respects line boundaries

The sidecar SHALL provide `chunk_text(text: str, *, chunk_size: int = 600, overlap: int = 60) -> list[ChunkDraft]` in `codebus_agent.kb.chunker`. The chunker MUST measure tokens using a tiktoken encoding (default `cl100k_base`) and MUST produce slices that never end in the middle of a source line. When a natural token-window boundary would fall mid-line, the chunker MUST extend the slice backward to the nearest preceding newline so the emitted chunk ends with a complete line. Each `ChunkDraft` MUST record `text`, `line_start` (1-based inclusive), `line_end` (1-based inclusive), `token_count`, and `chunk_index` / `chunk_total` populated by the caller before persistence.

#### Scenario: Chunk boundaries land on newline

- **WHEN** `chunk_text` is called on a multi-line text whose natural token window would cut across line 42
- **THEN** the returned chunks MUST each end with `\n` (or be the final chunk) and no chunk MUST contain a partial line

#### Scenario: Overlap preserves continuity

- **WHEN** `chunk_text` is called with `chunk_size=600, overlap=60` on a text exceeding 1200 tokens
- **THEN** consecutive chunks MUST share at least 60 tokens of content at their boundary

#### Scenario: Short text produces single chunk

- **WHEN** `chunk_text` is called on a text whose tiktoken-measured length is less than `chunk_size`
- **THEN** the returned list MUST contain exactly one `ChunkDraft` whose `line_start=1` and `line_end` equals the total line count

#### Scenario: Empty text produces empty list

- **WHEN** `chunk_text` is called with `text=""`
- **THEN** the returned list MUST be empty and no exception MUST be raised


<!-- @trace
source: module-2-kb-builder-p0
updated: 2026-04-21
code:
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - sidecar/src/codebus_agent/providers/usage_tracker.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/kb/__init__.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/chunker.py
tests:
  - sidecar/tests/kb/fixtures/sample-doc.md
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_strategy.py
  - sidecar/tests/kb/fixtures/sample-code.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/kb/fixtures/sample-plain.txt
  - sidecar/tests/kb/test_qdrant_kb.py
  - sidecar/tests/kb/test_chunker.py
  - sidecar/tests/kb/test_payload.py
-->

---
### Requirement: Chunk strategy dispatch by FileEntry kind and language

The sidecar SHALL dispatch chunking by `FileEntry.kind` and `language` according to the table in `docs/module-2-kb-builder.md §四`. Files with `kind="text"` and `language in {"markdown", "rst", "asciidoc", "plaintext"}` MUST use the doc strategy (heading-first split, then token window for sub-segments exceeding `chunk_size`). Files with `kind="text"` and any other language MUST use the code strategy (pure token window with line-boundary respect). Files with `kind="oversized"` MUST chunk only the `oversized_preview` payload and MUST set a marker distinguishing preview chunks from full-file chunks. Files with `kind in {"binary", "lockfile", "generated"}` MUST NOT chunk text; the builder MUST emit exactly one skeleton payload per such file whose `source_kind="skeleton"`, whose `text` is the empty string, and whose `file_path` is preserved. Symlinks reported on `ScanResult.symlinks` MUST NOT produce any `KBPayload` in the current scope.

#### Scenario: Markdown routed to doc strategy

- **WHEN** the builder processes a `FileEntry` with `kind="text", language="markdown"` containing three `##` headings
- **THEN** the emitted `ChunkDraft` list MUST reflect heading-based segmentation (at least one chunk starts at each heading boundary)

#### Scenario: Source code routed to code strategy

- **WHEN** the builder processes a `FileEntry` with `kind="text", language="python"`
- **THEN** the emitted chunks MUST be produced by the token-window strategy, not the heading strategy

#### Scenario: Binary file produces skeleton payload

- **WHEN** the builder processes a `FileEntry` with `kind="binary", file_path="assets/logo.png"`
- **THEN** exactly one payload MUST be produced with `source_kind="skeleton"`, `text=""`, `file_path="assets/logo.png"`, `chunk_index=0`, `chunk_total=1`

#### Scenario: Oversized file chunks preview only

- **WHEN** the builder processes a `FileEntry` with `kind="oversized", oversized_preview=<first 200 lines>, content=None`
- **THEN** the emitted chunks MUST cover only the preview content and each MUST carry a preview marker distinguishing them from full-file chunks

#### Scenario: Symlink produces no payload

- **WHEN** `ScanResult.symlinks` contains an entry
- **THEN** the builder MUST emit zero payloads for it


<!-- @trace
source: module-2-kb-builder-p0
updated: 2026-04-21
code:
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - sidecar/src/codebus_agent/providers/usage_tracker.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/kb/__init__.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/chunker.py
tests:
  - sidecar/tests/kb/fixtures/sample-doc.md
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_strategy.py
  - sidecar/tests/kb/fixtures/sample-code.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/kb/fixtures/sample-plain.txt
  - sidecar/tests/kb/test_qdrant_kb.py
  - sidecar/tests/kb/test_chunker.py
  - sidecar/tests/kb/test_payload.py
-->

---
### Requirement: Content-hash Layer 1 deduplication

The sidecar SHALL compute `text_hash = sha256(normalized_text).hexdigest()` where `normalized_text = text.strip()` before persisting any chunk. Prior to every Qdrant upsert, the builder MUST call `backend.exists_by_hash(collection, text_hash)`; when the backend reports the hash already present, the builder MUST skip the upsert, increment `KBStats.skipped_hash_count`, and MUST NOT call `provider.embed()` for that chunk. Deduplication state MUST NOT persist across `KnowledgeBase` instances — the check runs against the live Qdrant collection, not an in-process cache.

#### Scenario: Identical text skipped on second build

- **WHEN** a `KnowledgeBase` instance builds a `ScanResult` containing the same chunk text twice (same file, or two files with identical content)
- **THEN** exactly one point MUST be upserted to the backend and `KBStats.skipped_hash_count` MUST be at least 1

#### Scenario: No embedding call for duplicate

- **WHEN** the builder detects a duplicate via `exists_by_hash` returning `True`
- **THEN** the provider's `embed()` MUST NOT be invoked for that chunk (verifiable via a spying provider in tests)

#### Scenario: Whitespace-only diff still deduplicates

- **WHEN** two chunks have text differing only in leading or trailing whitespace
- **THEN** they MUST produce the same `text_hash` and the second MUST be skipped


<!-- @trace
source: module-2-kb-builder-p0
updated: 2026-04-21
code:
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - sidecar/src/codebus_agent/providers/usage_tracker.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/kb/__init__.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/chunker.py
tests:
  - sidecar/tests/kb/fixtures/sample-doc.md
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_strategy.py
  - sidecar/tests/kb/fixtures/sample-code.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/kb/fixtures/sample-plain.txt
  - sidecar/tests/kb/test_qdrant_kb.py
  - sidecar/tests/kb/test_chunker.py
  - sidecar/tests/kb/test_payload.py
-->

---
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


<!-- @trace
source: spec-cleanup-stage-5-batch-b
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - docs/sidecar-api.md
  - CLAUDE.md
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
-->

---
### Requirement: Progress callback protocol

`KnowledgeBase.build(scan_result, *, on_progress=None)` SHALL accept an optional async callable of type `Callable[[KBProgressEvent], Awaitable[None]]`. When provided, the callback MUST be invoked on `phase` transitions (`"chunking"`, `"embedding"`, `"upserting"`, `"done"`) and at least once per completed embedding batch during the `"embedding"` phase. Each event MUST carry `phase`, `current`, `total`, `workspace_id`, and optionally `message`. When `on_progress` is `None`, the build MUST proceed without emitting events and MUST NOT raise.

#### Scenario: Phase transitions emit events

- **WHEN** `build` runs with a list-appending async callback against a non-empty `ScanResult`
- **THEN** the recorded events MUST contain at least one event with each of `phase in {"chunking", "embedding", "upserting", "done"}`

#### Scenario: Per-batch embedding progress

- **WHEN** `build` processes 96 chunks (three batches) with a progress callback
- **THEN** during the `"embedding"` phase the callback MUST be invoked at least three times and the final embedding-phase event MUST have `current == total`

#### Scenario: No callback means silent run

- **WHEN** `build` runs with `on_progress=None`
- **THEN** no exception MUST be raised and the returned `KBStats` MUST match a run with a no-op callback


<!-- @trace
source: module-2-kb-builder-p0
updated: 2026-04-21
code:
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - sidecar/src/codebus_agent/providers/usage_tracker.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/kb/__init__.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/chunker.py
tests:
  - sidecar/tests/kb/fixtures/sample-doc.md
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_strategy.py
  - sidecar/tests/kb/fixtures/sample-code.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/kb/fixtures/sample-plain.txt
  - sidecar/tests/kb/test_qdrant_kb.py
  - sidecar/tests/kb/test_chunker.py
  - sidecar/tests/kb/test_payload.py
-->

---
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
### Requirement: Workspace-scoped Qdrant collection naming

The sidecar SHALL compute `workspace_id = sha256(workspace_root).hexdigest()[:16]` where `workspace_root` is the absolute resolved path. The Qdrant collection name for folder workspaces MUST be `codebus_{workspace_id}`. `KnowledgeBase` MUST ensure the collection exists (via `ensure_collection`) at construction time and MUST create payload indices for `text_hash` (keyword) and `related_stations` (keyword) before the first upsert.

#### Scenario: Deterministic collection name per workspace

- **WHEN** two `KnowledgeBase` instances are constructed with the same `workspace_root` and the same `embedding_dim`
- **THEN** both instances MUST bind to the same collection name `codebus_<sha256 prefix>`

#### Scenario: Different workspaces get distinct collections

- **WHEN** two `KnowledgeBase` instances are constructed with different `workspace_root` paths
- **THEN** their collection names MUST differ

#### Scenario: Payload indices created once

- **WHEN** a `KnowledgeBase` is constructed twice against the same workspace in the same process
- **THEN** payload-index creation MUST succeed both times (idempotent) and MUST NOT raise `QdrantCollectionSchemaError`


<!-- @trace
source: module-2-kb-builder-p0
updated: 2026-04-21
code:
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - sidecar/src/codebus_agent/providers/usage_tracker.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/kb/__init__.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/chunker.py
tests:
  - sidecar/tests/kb/fixtures/sample-doc.md
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_strategy.py
  - sidecar/tests/kb/fixtures/sample-code.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/kb/fixtures/sample-plain.txt
  - sidecar/tests/kb/test_qdrant_kb.py
  - sidecar/tests/kb/test_chunker.py
  - sidecar/tests/kb/test_payload.py
-->

---
### Requirement: KBStats returned by build

The sidecar SHALL define a `KBStats` Pydantic model returned by `KnowledgeBase.build()` with at least the fields: `chunks_emitted` (non-negative integer), `points_upserted` (non-negative integer), `skipped_hash_count` (non-negative integer), `batches_embedded` (non-negative integer), `prompt_tokens_total` (non-negative integer), `warnings` (list of strings), `duration_seconds` (non-negative float), `workspace_id` (string), `collection_name` (string). The invariant `points_upserted + skipped_hash_count == chunks_emitted` MUST hold unless warnings list a skipped-due-to-oversize entry, in which case `points_upserted + skipped_hash_count + len(oversize warnings) == chunks_emitted`.

#### Scenario: Stats accounting balances

- **WHEN** `build` completes a run with no warnings
- **THEN** `points_upserted + skipped_hash_count == chunks_emitted` MUST hold

#### Scenario: Stats exposes workspace identity

- **WHEN** `build` completes against a known workspace_root
- **THEN** the returned `KBStats.workspace_id` MUST equal the first 16 hex chars of `sha256(workspace_root).hexdigest()`

<!-- @trace
source: module-2-kb-builder-p0
updated: 2026-04-21
code:
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - sidecar/src/codebus_agent/providers/usage_tracker.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/kb/__init__.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/chunker.py
tests:
  - sidecar/tests/kb/fixtures/sample-doc.md
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_strategy.py
  - sidecar/tests/kb/fixtures/sample-code.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/kb/fixtures/sample-plain.txt
  - sidecar/tests/kb/test_qdrant_kb.py
  - sidecar/tests/kb/test_chunker.py
  - sidecar/tests/kb/test_payload.py
-->

---
### Requirement: POST /kb/build async endpoint

The sidecar SHALL expose `POST /kb/build` that accepts a JSON request body of the shape `{"workspace_root": "<absolute path>", "scan_result": <ScanResult JSON>}`. The endpoint MUST require the bearer token via the existing authentication middleware. On a successful request the endpoint SHALL create a `kind="kb"` task in the sidecar task registry, spawn a background coroutine that invokes `KnowledgeBase.build(scan_result, on_progress=<adapter>)`, return HTTP `202 Accepted` with body `{"task_id": "kb_<hex8>"}` immediately, and SHALL NOT block until the build completes. The 202 status code MUST match the convention used by all other task-spawning endpoints (`POST /scan` with stream=true / `POST /explore` / `POST /generate` / `POST /qa`) so clients can apply uniform `if status === 202: subscribe to SSE` logic. There SHALL NOT be a synchronous variant of `POST /kb/build` in this change. When the background build completes successfully the task handle's `result` MUST be set to the `KBStats` JSON returned by `build` and a `done` event MUST be emitted; when it raises, the error containment path defined by `sidecar-runtime` MUST apply.

#### Scenario: Successful request returns 202 with task_id immediately

- **WHEN** a client calls `POST /kb/build` with a valid bearer token and body `{"workspace_root": "<path>", "scan_result": {...}}` while no other task is in flight
- **THEN** the HTTP response status code MUST equal `202` (Accepted)
- **AND** the response body MUST match `{"task_id": "kb_<hex8>"}`
- **AND** the response MUST return within a small bounded latency (not blocked by KB build)

#### Scenario: Status code aligned with sibling task endpoints

- **WHEN** the sidecar test suite asserts the status code returned by each task-spawning endpoint (`POST /scan?stream=true`, `POST /kb/build`, `POST /explore`, `POST /generate`, `POST /qa`) on the success path
- **THEN** every endpoint in that set MUST return HTTP `202` (no endpoint MUST return `200` on the success path)

#### Scenario: Concurrent task in flight rejected with 409

- **WHEN** a client calls `POST /kb/build` while another task is currently `running` in the registry
- **THEN** the response MUST be HTTP `409` with body `{"code": "TASK_IN_FLIGHT", "running_task_id": "<id>"}` and no new background task MUST be started

#### Scenario: Done event makes KBStats reachable via result endpoint

- **WHEN** a client subscribes to `GET /tasks/{kb_task_id}/events` and the stream emits `done`
- **THEN** an immediately following `GET /tasks/{kb_task_id}/result` MUST return HTTP `200` with body equal to the `KBStats` JSON produced by the build


<!-- @trace
source: agent-defense-depth
updated: 2026-04-27
code:
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/api/scan.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/api/kb.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/kb/payload.py
tests:
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
-->

---
### Requirement: KB progress phase translation to wire schema

The `POST /kb/build` background task SHALL adapt every `KBProgressEvent` produced by `KnowledgeBase.build` into a wire event matching `docs/sidecar-api.md §四` `progress` schema with the field `phase` set to the literal string `"embedding"` regardless of the source event's internal phase (`chunking`, `embedding`, `upserting`, `done`). The adapter SHALL guarantee that subscribers observe at least one `progress` event whose `current == 0` near the start of the build (corresponding to the chunking transition) and at least one `progress` event whose `current == total` near the end of the build (corresponding to the upserting transition), so the wire stream forms a monotonic 0 → total progression even when the underlying KB build phases are not equal-sized. The adapter SHALL NOT emit a wire `progress` event for the source `done` phase; the terminal transition MUST be emitted as the SSE `done` event by the task wrapper.

#### Scenario: Source done phase becomes wire done event

- **WHEN** `KnowledgeBase.build` emits a `KBProgressEvent` whose internal phase is `done`
- **THEN** the adapter MUST NOT translate it into a `progress` wire event; the task wrapper MUST emit the SSE `done` event after the build coroutine returns

#### Scenario: All non-done source phases collapse to embedding

- **WHEN** `KnowledgeBase.build` emits source events with internal phases `chunking`, `embedding`, and `upserting` during a single build
- **THEN** every wire `progress` event delivered to subscribers MUST have `phase == "embedding"`

#### Scenario: Wire stream is monotonic and reaches total

- **WHEN** a build produces N total chunks across a sequence of source events
- **THEN** the sequence of wire `progress` events delivered to a subscriber MUST contain at least one event with `current == 0` and at least one event with `current == N`, and the `current` values MUST be monotonically non-decreasing

<!-- @trace
source: sse-progress-skeleton
updated: 2026-04-22
code:
  - sidecar/src/codebus_agent/scanner/models.py
  - CLAUDE.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/scanner/service.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/api/scan.py
  - docs/module-1-scanner.md
  - docs/module-2-kb-builder.md
  - docs/sidecar-api.md
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/api/__init__.py
  - sidecar/tests/scanner/test_fixtures_integration.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/api/test_task_error_containment.py
  - sidecar/tests/api/test_task_registry.py
  - sidecar/tests/api/test_task_result.py
  - sidecar/tests/scanner/test_service.py
  - sidecar/tests/api/test_tasks_sse.py
  - sidecar/tests/scanner/test_progress_callback.py
-->

---
### Requirement: KB build production dependency wiring

The sidecar SHALL expose a `POST /kb/build` endpoint that, when all four KB dependencies (`kb_backend`, `kb_provider`, `kb_usage_tracker` factory, `kb_embedding_dim`) are populated on `app.state`, executes a full chunk → embed → upsert pipeline and makes the resulting `KBStats` retrievable via `GET /tasks/{id}/result`. When any dependency is absent or misconfigured, the endpoint SHALL respond with a specific, documented error code so the caller can recover without restarting the sidecar.

#### Scenario: Happy path returns KBStats via result endpoint

- **WHEN** `CODEBUS_OPENAI_API_KEY` is set, Qdrant is reachable, and the caller posts a valid `{workspace_root, scan_result}` body to `POST /kb/build`
- **THEN** the endpoint MUST return `200 {"task_id": "kb_<hex8>"}` within 2 seconds, emit `progress` and `done` events over the SSE stream, and make a `KBStats` object with non-zero `chunks_emitted` and `points_upserted` reachable through `GET /tasks/{task_id}/result`

#### Scenario: Missing OpenAI API key returns 503 KB_NOT_CONFIGURED

- **WHEN** the sidecar starts without `CODEBUS_OPENAI_API_KEY` and the caller posts to `POST /kb/build`
- **THEN** the endpoint MUST return `503` with body `{"code": "KB_NOT_CONFIGURED", "missing": ["embedding_provider"]}` and MUST NOT create a task handle, MUST NOT emit any SSE events, and MUST NOT call the Qdrant backend

#### Scenario: Existing collection with wrong vector dimension returns 409 KB_DIM_MISMATCH

- **WHEN** the Qdrant collection named by the workspace already exists with a vector dimension different from the dimension declared by the configured embedding provider, and the caller posts to `POST /kb/build`
- **THEN** the background task MUST emit an SSE `error` event with `{"code": "KB_DIM_MISMATCH", "expected_dim": <provider-dim>, "actual_dim": <collection-dim>, "suggestion": "delete collection and rebuild"}` before any embedding calls are made, and MUST NOT upsert any points

#### Scenario: OpenAI rate limit surfaces as sanitized error event

- **WHEN** the OpenAI embedding provider exhausts its internal retry budget during a `POST /kb/build` task
- **THEN** the background task MUST emit an SSE `error` event with `code: "OPENAI_RATE_LIMITED"` (or `OPENAI_AUTH_FAILED` for 401 responses), MUST NOT leak the provider's stack trace in the wire event, and the full traceback MUST be written only to the sidecar logger

#### Scenario: UsageTracker records embedding call for the requesting workspace

- **WHEN** a `POST /kb/build` task completes successfully against `workspace_root=/abs/example`
- **THEN** at least one line with `operation="embed"` and `module="kb_build"` MUST be appended to `/abs/example/token_usage.jsonl` (or the workspace-scoped path defined by the existing `UsageTracker writes token_usage.jsonl` Requirement), with `input_tokens > 0` and a non-null `cost_usd`

<!-- @trace
source: kb-build-production-wiring
updated: 2026-04-23
code:
  - sidecar/src/codebus_agent/providers/openai_embedding.py
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/api/tasks.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/providers/tracked.py
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/api/main.py
  - docs/llm-provider.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/health.py
  - sidecar/uv.lock
  - docs/decisions.md
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/providers/__init__.py
  - docs/implementation-plan.md
  - CLAUDE.md
tests:
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/test_wire_kb_dependencies.py
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_dim_mismatch.py
  - sidecar/tests/providers/test_openai_embedding.py
-->

---
### Requirement: POST /kb/query endpoint

The sidecar SHALL expose a synchronous `POST /kb/query` HTTP endpoint that accepts a JSON body `{workspace_root: str, text: str, top_k: int = 8, filter_path: str | None = None, filter_source_kind: list[str] | None = None}` and returns a `200 OK` response with body `{"hits": [...]}` where each entry conforms to the `KBHit` schema (point_id / score / payload). The endpoint SHALL embed `text` via the workspace-scoped TrackedProvider (per `KB build production dependency wiring` factory), search the workspace's Qdrant collection, and return hits ordered by descending score, delegating to `KnowledgeBase.query(...)`. Unlike `POST /kb/build`, this endpoint MUST be synchronous (no task handle, no SSE) because typical query latency is below 1 second.

#### Scenario: Successful query returns hits ordered by score

- **WHEN** the caller posts `{"workspace_root": "/abs/ws", "text": "storage", "top_k": 3}` against a populated workspace with a valid bearer token
- **THEN** the response status MUST be `200`, the response body MUST contain `"hits"` (a list of at most 3 entries), and each entry MUST contain `point_id`, `score`, and `payload` fields with scores monotonically non-increasing

#### Scenario: Empty collection returns empty hits list with 200

- **WHEN** the caller queries a workspace whose Qdrant collection does not exist or contains no points
- **THEN** the response status MUST be `200` with body `{"hits": []}` (no `404`); callers handle the "no results" case identically whether the collection is unbuilt or simply unmatched

#### Scenario: Missing OpenAI API key returns 503 KB_NOT_CONFIGURED

- **WHEN** the sidecar was started without `CODEBUS_OPENAI_API_KEY` and the caller posts to `/kb/query`
- **THEN** the response MUST be `503` with body `{"detail": {"code": "KB_NOT_CONFIGURED", ...}}`, mirroring the `POST /kb/build` graceful-degrade contract — query needs the embedding provider to embed `text` into a vector

#### Scenario: Invalid request body returns 422

- **WHEN** the caller posts a body missing `text`, or with `top_k <= 0`, or with `top_k > 50`
- **THEN** the response MUST be `422` (Pydantic validation error); no Qdrant call MUST be made and no OpenAI embed MUST be attempted

#### Scenario: filter_path narrows results in HTTP path

- **WHEN** the caller posts `{"workspace_root": "/abs/ws", "text": "x", "filter_path": "src/foo.py"}`
- **THEN** every hit returned MUST have `payload.file_path == "src/foo.py"`, matching the existing `KnowledgeBase query and find_similar API` Requirement scenario "filter_path restricts results"

#### Scenario: Bearer token required

- **WHEN** the caller posts to `/kb/query` without an `Authorization: Bearer <token>` header
- **THEN** the response MUST be `401` and no embed call or Qdrant query MUST be attempted

#### Scenario: Query usage recorded with module=kb_query

- **WHEN** a successful `/kb/query` call completes against `workspace_root=/abs/ws`
- **THEN** at least one line MUST be appended to `/abs/ws/token_usage.jsonl` with `operation="embed"` and `module="kb_query"` (per the `usage-tracking` capability `module field` semantics — the query path's TrackedProvider factory MUST tag with `default_module="kb_query"` distinct from build's `"kb_build"`)

<!-- @trace
source: kb-query-endpoint
updated: 2026-04-23
code:
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/api/__init__.py
  - docs/sidecar-api.md
  - CLAUDE.md
  - docs/module-2-kb-builder.md
tests:
  - sidecar/tests/test_wire_kb_dependencies.py
  - sidecar/tests/api/test_kb_query.py
-->

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

<!-- @trace
source: spec-cleanup-stage-5-batch-b
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - docs/sidecar-api.md
  - CLAUDE.md
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
-->
