## Why

Module 2 KB Builder 是資料層唯一把 Scanner 輸出轉成向量索引的模組；在它落地之前，Explorer Agent 與 Q&A Agent 都沒東西可查（`docs/implementation-plan.md §二` 步驟 14、`module-2-kb-builder.md §十三 P0`）。Scanner Pass 1 Sanitizer 已於 2026-04-21 串通（change `scanner-sanitizer-orchestration`），`FileEntry.content` 現在保證是 sanitize 過的，Module 2 可以直接吃；先前阻擋這步的前置條件已解除。

關聯決策：D-001（Tauri + Python sidecar + Qdrant 混合架構）、D-012（自寫 ReAct / Instructor）、D-015（三段 Sanitizer — Module 2 依賴 Pass 1 輸出）、D-016（Q&A add_to_kb 的後端地基）、D-021（token_usage 追蹤 — embed 呼叫走同一條管線）、D-027（Qdrant 本地 binary 主路徑）。

## What Changes

本次落地 `module-2-kb-builder.md §十三` 的 **P0 全部七項**，但**不**包含 P1（git metadata chunk、`upsert_chunk` / `delete_chunk` for qa_agent、rebuild 保留 qa chunks、Timeline integration fixture）。HTTP endpoint 與 SSE wire 屬 step 15，不在本次 scope；本 change 在 Module 2 側提供 progress callback Hook，供 step 15 串接。

- **新增 `KBPayload` Pydantic schema**（`module-2-kb-builder.md §三`）：涵蓋 source_kind / file_path / line_start / line_end / commit_oid / text / text_hash / language / added_by / session_id / chunk_index / chunk_total / created_at / source_mtime / sanitize_stats / related_stations。`text_hash` 與 `related_stations` 需建 Qdrant payload index，`related_stations` 套用 regex `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$` 驗證。
- **新增 Token-window chunker**：預設 `chunk_size=600` token / `overlap=60` token，尊重行邊界（不切到半行）；tokenizer 走 tiktoken（與 provider 無關）。
- **新增 Chunk 策略分派**：依 `FileEntry.kind` + `language` 分派 `code` / `doc` / `skeleton` / `oversized` 四條路徑；`text_hash` = `sha256(normalized_text).hexdigest()`，`normalized_text = text.strip()`。
- **新增 `KnowledgeBase` 類別（build pipeline）**：`build(scan_result) → KBStats`、`query(text, *, top_k, filter_path, filter_source_kind) → list[KBHit]`、`find_similar(text, *, threshold=0.95) → KBHit | None`、`stats() → KBStats`；整條管線串 `ScanResult → chunk → embed → content-hash Layer 1 dedup → Qdrant upsert`。
- **新增 Layer 1 content-hash 去重**：每塊 chunk upsert 前先 `exists_by_hash(collection, text_hash)`，已存在就回 `skipped_hash`；不做 Layer 2 similarity dedup（屬 P1 / D-016 qa_agent 才啟用）。
- **新增 Qdrant upsert / search / exists_by_hash 包裝**：在既有 `codebus_agent.kb.qdrant_client` module 內擴充，保留「runtime 不直接 import `qdrant_client` SDK」的既有約束（`qdrant-client` 既有 Requirement）。
- **新增 Batch embed pipeline**：batch size 32、`asyncio.Semaphore(3)` 限制 in-flight；每 batch 完成時呼叫 progress callback（型別 `Callable[[KBProgressEvent], Awaitable[None]]`），供 step 15 串 SSE；同步把 `EmbedResponse.usage` 寫進 `ctx.usage_tracker`（module="kb_build"）。
- **修改 `qdrant-client` capability**：新增 Requirement「KB-facing vector upsert、search、hash-existence 包裝」；既有 Requirement（lifecycle / probe / ensure_collection）不動。

## Non-Goals

- **不做 git metadata chunk**（recent_commits / file_activity / blame）——P1，配合 Scanner Phase 2 增量 build 再做。
- **不做 similarity dedup Layer 2**（`threshold=0.95`）——僅 `qa_agent` `add_to_kb` 啟用；屬 D-016 後端，P1 才做。
- **不做 `upsert_chunk` / `delete_chunk` for qa_agent**——D-016 後端 P1；本次只提供 `build()` 與唯讀 `query()` / `find_similar()`。
- **不做 rebuild 保留 `added_by=qa_agent` 選項**——P1，配合 D-016 後端一起做。
- **不做 HTTP endpoint / SSE wiring**——屬 `implementation-plan.md` 步驟 15（Module 1/2 SSE progress emit 串通）；本次僅暴露 progress callback hook，endpoint 由後續 change 接。
- **不做 Timeline integration fixture 與 cost benchmark**——P1；D-007 cost benchmark 需要 real embedding provider，M1 只有 MockProvider，跑不出有意義數據。
- **不做 AST-aware chunk（tree-sitter）**——`module-2-kb-builder.md §十二` 明示 Phase 2 評估。
- **不做增量 build / embedding 快取**——`module-2-kb-builder.md §十二` 明示 Phase 2。
- **不做跨 workspace 共用 collection**——Phase 3。

## Capabilities

### New Capabilities

- `knowledge-base`: Module 2 KB Builder 的主要 capability，涵蓋 `KBPayload` schema、chunker、build pipeline、query API、content-hash 去重、progress callback 協定、與 UsageTracker 整合。

### Modified Capabilities

- `qdrant-client`: 新增 KB-facing Requirement — vector upsert、search、hash-existence 包裝（既有 lifecycle / probe / ensure_collection Requirement 不動；runtime 不直接 import SDK 的既有約束繼續成立，新增的包裝仍寫在 `codebus_agent.kb.qdrant_client` 內）。

## Impact

- **Affected specs**:
  - 新增 `openspec/specs/knowledge-base/spec.md`
  - 修改 `openspec/specs/qdrant-client/spec.md`（新增 Requirement delta）
- **Affected code**:
  - 新增 `sidecar/src/codebus_agent/kb/payload.py`（`KBPayload` / `KBHit` / `KBStats` / `KBProgressEvent`）
  - 新增 `sidecar/src/codebus_agent/kb/chunker.py`（token-window chunker + 策略分派）
  - 新增 `sidecar/src/codebus_agent/kb/knowledge_base.py`（`KnowledgeBase` class — build / query / find_similar / stats）
  - 擴充 `sidecar/src/codebus_agent/kb/qdrant_client.py`（加 `upsert_points` / `search_points` / `exists_by_hash` 與 payload index 建立）
  - 新增 `sidecar/tests/kb/test_payload.py`、`test_chunker.py`、`test_knowledge_base.py`、`test_qdrant_client_upsert.py`
  - 新增 `sidecar/tests/kb/fixtures/`（chunker 測試用文字 / 程式碼樣本）
- **Affected docs**:
  - `docs/module-2-kb-builder.md`：§十三 P0 加完成日註記；其餘文字已於過往 change 寫好，不動
  - `docs/implementation-plan.md`：§二 步驟 14 加完成日註記
  - `docs/decisions.md`：如在 design 階段新增非 trivial 取捨（例如 tokenizer 選型、chunk 策略分派細節），再新增 D-編號 ADR
- **Dependencies**:
  - 既有 `qdrant-client` capability（connect / probe / ensure_collection）
  - 既有 `llm-provider` capability（`embed()` / TrackedProvider 裝飾 / `EmbedResponse.usage`）
  - 既有 `folder-scanner` capability（`ScanResult` / `FileEntry`）
  - 既有 `usage-tracking` capability（`token_usage.jsonl` / `UsageTracker.record(module="kb_build")`）
- **Unblocks**:
  - `implementation-plan.md` 步驟 15（Module 1/2 SSE progress emit 串通）
  - `implementation-plan.md` 步驟 17（Explorer 真工具串 KB）
  - `implementation-plan.md` 步驟 25（Q&A Agent P0 + `add_to_kb`）
