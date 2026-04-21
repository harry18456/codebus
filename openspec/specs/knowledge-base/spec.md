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

The builder SHALL group chunks into batches of 32 and SHALL submit at most three batches to `provider.embed()` concurrently, enforced by an `asyncio.Semaphore(3)`. Each batch's `EmbedResponse.usage` MUST be recorded via `ctx.usage_tracker.record(usage=response.usage, module="kb_build")`. When a single chunk exceeds the provider's declared maximum input token count, the builder MUST split the chunk into halves and retry; when the halved chunk still exceeds the limit, the builder MUST skip it, emit a warning into `KBStats.warnings`, and MUST NOT raise.

#### Scenario: Batch size capped at 32

- **WHEN** the builder processes 100 chunks against a provider that records every `embed()` call
- **THEN** the provider MUST receive exactly 4 calls (32, 32, 32, 4) and no batch MUST exceed 32 entries

#### Scenario: Concurrency capped at 3 in-flight batches

- **WHEN** the builder runs against a provider whose `embed()` blocks until released and 10 batches are queued
- **THEN** at most 3 `embed()` invocations MUST be concurrently in-flight at any moment

#### Scenario: UsageTracker records one entry per batch

- **WHEN** the builder processes 64 chunks (two batches) through a provider that returns non-zero `usage`
- **THEN** `usage_tracker.record` MUST be called at least twice with `module="kb_build"` and the sum of recorded `prompt_tokens` MUST equal the provider's reported total

#### Scenario: Oversized chunk split then skipped

- **WHEN** a chunk's token count exceeds the provider's max input even after halving
- **THEN** the chunk MUST be skipped, `KBStats.warnings` MUST gain an entry naming the offending file path, and the build MUST complete without raising


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

The sidecar SHALL expose `KnowledgeBase.query(text: str, *, top_k: int = 8, filter_path: str | None = None, filter_source_kind: list[str] | None = None) -> list[KBHit]` and `KnowledgeBase.find_similar(text: str, *, threshold: float = 0.95) -> KBHit | None`. `query` MUST embed `text` via the bound provider, search the workspace collection, and return hits ordered by descending score. `find_similar` MUST call `query(text, top_k=1)` internally and MUST return `None` when the top hit's score is strictly less than `threshold`.

#### Scenario: Query returns top_k hits ordered by score

- **WHEN** `query("Storage", top_k=3)` is called against a populated collection
- **THEN** the returned list MUST contain at most 3 `KBHit` entries and scores MUST be monotonically non-increasing

#### Scenario: filter_path restricts results

- **WHEN** `query("Storage", filter_path="src/storage/types.ts")` is called
- **THEN** every returned hit's `payload.file_path` MUST equal `"src/storage/types.ts"`

#### Scenario: filter_source_kind restricts results

- **WHEN** `query("x", filter_source_kind=["code"])` is called against a collection containing both `code` and `skeleton` payloads
- **THEN** no returned hit's `payload.source_kind` MUST be `"skeleton"`

#### Scenario: find_similar returns None below threshold

- **WHEN** `find_similar("rare query", threshold=0.95)` is called and the top hit's score is `0.80`
- **THEN** the return value MUST be `None`

#### Scenario: find_similar returns hit at or above threshold

- **WHEN** `find_similar("known text", threshold=0.95)` is called and the top hit's score is `0.96`
- **THEN** the return value MUST be a `KBHit` whose `score >= 0.95`


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
