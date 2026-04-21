# Sample Documentation Fixture

This fixture exercises the doc-strategy chunker which prefers heading boundaries over a pure token window.

## Overview

CodeBus runs a local Python sidecar that ingests workspace files, chunks them, and stores embeddings in a Qdrant collection bound to the workspace.

The KB Builder is the data-plane shim that produces the vector index downstream agents (Explorer, Q&A) consume via `query()` and `find_similar()`.

## Architecture

The pipeline has three stages:

1. **Scan** — `folder-scanner` walks the workspace, decodes text files, and runs Sanitizer Pass 1.
2. **Chunk** — `kb.chunker.dispatch_for_file_entry` routes each `FileEntry` through one of four strategies (code / doc / skeleton / oversized).
3. **Embed + Upsert** — Chunks are batched (32 per batch, max 3 in-flight), embedded by the role-bound provider, deduplicated by content hash, then upserted to Qdrant.

Each stage emits `KBProgressEvent` so the sidecar's SSE wire (Module 1/2 step 15) can stream progress to the frontend.

## Usage

```python
from codebus_agent.kb import KnowledgeBase

kb = KnowledgeBase(
    client=qdrant_client,
    provider=embedding_provider,
    usage_tracker=tracker,
    workspace_id="<sha256[:16]>",
    embedding_dim=8,
)
stats = await kb.build(scan_result, on_progress=callback)
```

The fixture should comfortably exceed the default `chunk_size=600` token window so the doc strategy is exercised end-to-end across multiple sub-segments.
