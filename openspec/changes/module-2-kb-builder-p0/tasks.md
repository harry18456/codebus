## 1. Scaffolding（對應 `implementation-plan.md` 步驟 14 起手）

- [ ] 1.1 建立 `sidecar/src/codebus_agent/kb/payload.py` 空模組，將 `KBPayload` / `KBHit` / `KBStats` / `KBProgressEvent` 佔位 import 於 `codebus_agent/kb/__init__.py` re-export（先不實作欄位）
- [ ] 1.2 建立 `sidecar/src/codebus_agent/kb/chunker.py` 空模組，宣告 `chunk_text` / `dispatch_for_file_entry` 佔位簽名
- [ ] 1.3 建立 `sidecar/src/codebus_agent/kb/knowledge_base.py` 空模組，宣告 `KnowledgeBase` 佔位 class
- [ ] 1.4 建立 `sidecar/tests/kb/fixtures/` 並放入三類樣本：`sample-code.py`（多 function、>600 token）、`sample-doc.md`（三個 `##` heading）、`sample-plain.txt`（short）
- [ ] 1.5 確認 `sidecar/tests/kb/__init__.py` 存在（既有），無則建立

## 2. RED — `KBPayload schema` 規約測試先行（TDD）

- [ ] 2.1 [P] 於 `sidecar/tests/kb/test_payload.py` 加 `test_kbpayload_happy_path_round_trips`（含 `model_dump()` ↔ `model_validate()` 對稱驗證）
- [ ] 2.2 [P] 於 `test_payload.py` 加 `test_kbpayload_rejects_invalid_text_hash`（非 64 字 / 非 hex）
- [ ] 2.3 [P] 於 `test_payload.py` 加 `test_kbpayload_rejects_malformed_related_stations`（`s1-x` / 超長 slug）
- [ ] 2.4 [P] 於 `test_payload.py` 加 `test_kbpayload_enforces_chunk_index_total_invariant`（`chunk_index=3, chunk_total=2` 須 raise）

## 3. GREEN — 實作 `KBPayload schema`

- [ ] 3.1 於 `payload.py` 實作 `KBPayload schema`（含 `text_hash` regex validator、`related_stations` regex validator、`chunk_index/chunk_total` `model_validator`）
- [ ] 3.2 於 `payload.py` 補實作 `KBHit`（`point_id, score, payload`）、`KBStats`（見 `KBStats returned by build` Requirement）、`KBProgressEvent`（見 `Progress callback protocol` Requirement）
- [ ] 3.3 執行 `uv run pytest sidecar/tests/kb/test_payload.py` 確認 2.1 ~ 2.4 全綠

## 4. RED — `Token-window chunker respects line boundaries` 規約測試

- [ ] 4.1 [P] 於 `sidecar/tests/kb/test_chunker.py` 加 `test_chunk_text_lands_on_line_boundary`（多行長文本，assert 每塊以 `\n` 結尾或為最後一塊）
- [ ] 4.2 [P] 於 `test_chunker.py` 加 `test_chunk_text_overlap_preserves_continuity`（1200+ token，相鄰兩塊共享 ≥60 token）
- [ ] 4.3 [P] 於 `test_chunker.py` 加 `test_chunk_text_short_returns_single_chunk`（<chunk_size 時只 1 塊、`line_start=1`、`line_end=totalLines`）
- [ ] 4.4 [P] 於 `test_chunker.py` 加 `test_chunk_text_empty_returns_empty`（`text=""` → `[]`、不 raise）

## 5. GREEN — 實作 token-window chunker（對應 design `Tokenizer 選 tiktoken，不要求 provider 提供`）

- [ ] 5.1 於 `chunker.py` 實作 `chunk_text(text, *, chunk_size=600, overlap=60)`（Token-window chunker respects line boundaries），採 `tiktoken.encoding_for_model("cl100k_base")` 計 token
- [ ] 5.2 於 `chunker.py` 加 line-boundary backtrack helper（若切點不在行尾，倒退到前一個 `\n`；遇到整塊無換行的極端狀況放行不強切）
- [ ] 5.3 將 `tiktoken` 加入 `sidecar/pyproject.toml` dependencies 並執行 `uv sync`
- [ ] 5.4 執行 `uv run pytest sidecar/tests/kb/test_chunker.py` 確認 4.1 ~ 4.4 全綠

## 6. RED — `Chunk strategy dispatch by FileEntry kind and language` 規約測試

- [ ] 6.1 [P] 於 `sidecar/tests/kb/test_strategy.py` 加 `test_markdown_routed_to_doc_strategy`（assert heading 切分）
- [ ] 6.2 [P] 於 `test_strategy.py` 加 `test_python_routed_to_code_strategy`（不用 heading 切分）
- [ ] 6.3 [P] 於 `test_strategy.py` 加 `test_binary_produces_skeleton_payload`（`source_kind="skeleton"`、`text=""`、`chunk_index=0`、`chunk_total=1`）
- [ ] 6.4 [P] 於 `test_strategy.py` 加 `test_oversized_chunks_preview_only`（走 `oversized_preview`、每塊帶 preview 旗標）
- [ ] 6.5 [P] 於 `test_strategy.py` 加 `test_symlink_produces_no_payload`

## 7. GREEN — 實作策略分派（對應 design `Chunk 策略以 FileEntry.kind + language 單表分派`）

- [ ] 7.1 於 `chunker.py` 實作 `dispatch_for_file_entry(file_entry) -> list[ChunkDraft]`（Chunk strategy dispatch by FileEntry kind and language），以單一 dispatch table 路由 code / doc / skeleton / oversized 四條路徑
- [ ] 7.2 於 `chunker.py` 加 `_doc_strategy`（先按 `##` heading 分段、超過 `chunk_size` 才再走 window）
- [ ] 7.3 於 `payload.py` 為 `ChunkDraft` 加 `flags: list[str]` 欄位，oversized 策略塞 `["preview"]`
- [ ] 7.4 於 `knowledge_base.py` 起點把 symlink list 無條件略過（不產 payload）
- [ ] 7.5 執行 `uv run pytest sidecar/tests/kb/test_strategy.py` 確認 6.1 ~ 6.5 全綠

## 8. RED — Qdrant wrapper KB-facing 規約測試（使用 real Qdrant，標記 skip-if-unreachable）

- [ ] 8.1 [P] 於 `sidecar/tests/kb/test_qdrant_kb.py` 加 `test_upsert_points_writes_and_search_returns_ids`（`KB-facing vector upsert helper`）
- [ ] 8.2 [P] 於 `test_qdrant_kb.py` 加 `test_upsert_points_serializes_datetime_as_iso8601`（`KB-facing vector upsert helper` scenario 2）
- [ ] 8.3 [P] 於 `test_qdrant_kb.py` 加 `test_search_points_empty_collection_returns_empty_list` + `test_search_points_filter_by_file_path`（`KB-facing vector search helper`）
- [ ] 8.4 [P] 於 `test_qdrant_kb.py` 加 `test_exists_by_hash_true_false_and_missing_collection`（`Hash existence helper for deduplication`）
- [ ] 8.5 [P] 於 `test_qdrant_kb.py` 加 `test_ensure_kb_payload_indices_idempotent`（`Idempotent KB payload index provisioning`）

## 9. GREEN — 擴充 `codebus_agent.kb.qdrant_client`（runtime 不得繞過）

- [ ] 9.1 於 `qdrant_client.py` 加 `upsert_points(client, collection, points)`，以 `KBPayload.model_dump(mode="json")` 序列化 datetime 為 ISO-8601
- [ ] 9.2 於 `qdrant_client.py` 加 `search_points(client, collection, vector, *, limit, query_filter=None)`，支援 `file_path` / `source_kind` 等值與 `related_stations` 成員過濾
- [ ] 9.3 於 `qdrant_client.py` 加 `exists_by_hash(client, collection, text_hash)`，collection 不存在時回 `False` 不 raise
- [ ] 9.4 於 `qdrant_client.py` 加 `ensure_kb_payload_indices(client, collection)`，對 `text_hash` 與 `related_stations` 建 keyword index，冪等（對應 design `Qdrant payload index 建立時機`）
- [ ] 9.5 執行 `uv run pytest sidecar/tests/kb/test_qdrant_kb.py` 確認 8.1 ~ 8.5 全綠（如 Qdrant 不可達則 auto-skip，記錄訊息）

## 10. RED — `KBQdrantBackend` Protocol + InMemory backend（對應 design `Qdrant 離線測試策略`）

- [ ] 10.1 於 `sidecar/src/codebus_agent/kb/backend.py` 定義 `KBQdrantBackend` Protocol（`upsert_points` / `search_points` / `exists_by_hash` / `ensure_indices` / `drop_collection`）；於 `sidecar/tests/kb/conftest.py` 實作 `InMemoryQdrantBackend`（dict + cosine 手算）供後續測試使用
- [ ] 10.2 [P] 於 `sidecar/tests/kb/test_knowledge_base.py` 加 `test_workspace_id_is_sha256_prefix_of_workspace_root`（`Workspace-scoped Qdrant collection naming` + design `workspace_id 算法`）
- [ ] 10.3 [P] 於 `test_knowledge_base.py` 加 `test_content_hash_layer1_skips_duplicate_and_bypasses_embed`（`Content-hash Layer 1 deduplication`；驗證 design 決策 `content-hash normalization 只 strip` — 只 strip、不做激進 normalize；用 spy provider 驗證 `embed` 未被呼叫）
- [ ] 10.4 [P] 於 `test_knowledge_base.py` 加 `test_query_top_k_ordering_and_filter_path_and_filter_source_kind`（`KnowledgeBase query and find_similar API` 前三 scenario）
- [ ] 10.5 [P] 於 `test_knowledge_base.py` 加 `test_find_similar_threshold_behavior`（`KnowledgeBase query and find_similar API` 後兩 scenario）
- [ ] 10.6 [P] 於 `test_knowledge_base.py` 加 `test_kb_stats_accounting_balances`（`KBStats returned by build` invariant）

## 11. GREEN — 實作 `KnowledgeBase` build / query / find_similar（對應 design `KnowledgeBase 建構時綁定 workspace，不採全域 singleton`）

- [ ] 11.1 於 `knowledge_base.py` 實作 `KnowledgeBase.__init__`（對應 design 決策：`KnowledgeBase` 建構時綁定 workspace，不採全域 singleton — 收 `AsyncQdrantClient` / provider / `UsageTracker` / `workspace_id` / `embedding_dim`；於此呼叫 `ensure_collection` 與 `ensure_kb_payload_indices`）
- [ ] 11.2 於 `knowledge_base.py` 實作 `_derive_workspace_id(workspace_root)`（對應 design 決策：`workspace_id` 算法 — `sha256(workspace_root).hexdigest()[:16]` 的 pure helper）
- [ ] 11.3 於 `knowledge_base.py` 實作 `build(scan_result, *, on_progress=None)`：symlink 略過 → `dispatch_for_file_entry` 切塊 → per chunk `exists_by_hash` → 未命中入 embed queue → 批次 embed → `upsert_points`
- [ ] 11.4 於 `knowledge_base.py` 實作 `query(text, *, top_k=8, filter_path=None, filter_source_kind=None)`，呼 provider.embed 後用 `search_points`；實作 `find_similar(text, *, threshold=0.95)`，底層複用 `query(text, top_k=1)`
- [ ] 11.5 執行 `uv run pytest sidecar/tests/kb/test_knowledge_base.py` 確認 10.2 ~ 10.6 全綠

## 12. RED — `Embedding batch pipeline with UsageTracker wiring`（對應 design `Embedding batch in-flight 限制 3`）

- [ ] 12.1 [P] 於 `test_knowledge_base.py` 加 `test_embedding_batch_size_capped_at_32`（100 chunk → 4 個 batch：32/32/32/4）
- [ ] 12.2 [P] 於 `test_knowledge_base.py` 加 `test_embedding_concurrency_capped_at_3_inflight`（blocking provider 檢查同時 inflight ≤ 3）
- [ ] 12.3 [P] 於 `test_knowledge_base.py` 加 `test_usage_tracker_records_one_entry_per_batch`（assert `module="kb_build"` 與 token 合計）
- [ ] 12.4 [P] 於 `test_knowledge_base.py` 加 `test_oversized_chunk_split_then_skipped_with_warning`（halve 後仍 oversized → skip + warning，不 raise）

## 13. GREEN — 實作批次 embed pipeline

- [ ] 13.1 於 `knowledge_base.py` 加 `_embed_batches(chunks, ctx_semaphore=asyncio.Semaphore(3))`（Embedding batch pipeline with UsageTracker wiring — batch 32、`asyncio.gather`）
- [ ] 13.2 於 `knowledge_base.py` 每批 embed 完後呼 `ctx.usage_tracker.record(usage=..., module="kb_build")`
- [ ] 13.3 於 `knowledge_base.py` 加 oversized chunk 二切 fallback（仍超則 skip 並 append `KBStats.warnings`）
- [ ] 13.4 執行 `uv run pytest sidecar/tests/kb/test_knowledge_base.py -k embedding` 確認 12.1 ~ 12.4 全綠

## 14. RED — `Progress callback protocol`（對應 design `Progress callback 協定設計`）

- [ ] 14.1 [P] 於 `test_knowledge_base.py` 加 `test_progress_callback_emits_all_phase_transitions`（收集 event，assert 四個 phase 皆出現）
- [ ] 14.2 [P] 於 `test_knowledge_base.py` 加 `test_progress_callback_per_batch_embedding_progress`（96 chunk → embedding phase ≥3 events，最後一筆 `current==total`）
- [ ] 14.3 [P] 於 `test_knowledge_base.py` 加 `test_progress_callback_none_runs_silently`（`on_progress=None` 與 no-op callback 輸出 `KBStats` 相同）

## 15. GREEN — 實作 progress callback

- [ ] 15.1 於 `knowledge_base.py` 於 `chunking` / `embedding` / `upserting` / `done` 節點分別 `await on_progress(KBProgressEvent(...))`（`on_progress is None` 時 no-op）
- [ ] 15.2 執行 `uv run pytest sidecar/tests/kb/test_knowledge_base.py -k progress` 確認 14.1 ~ 14.3 全綠

## 16. 文件與 re-export 收尾

- [ ] 16.1 於 `sidecar/src/codebus_agent/kb/__init__.py` 最終化 public API re-export（`KnowledgeBase`、`KBPayload`、`KBHit`、`KBStats`、`KBProgressEvent`、`ProgressCallback`）
- [ ] 16.2 於 `docs/module-2-kb-builder.md §十三` P0 該行尾加「— 2026-04-21 落地（change `module-2-kb-builder-p0`）」註記
- [ ] 16.3 於 `docs/implementation-plan.md §二` 步驟 14 該行尾加同樣完成日註記
- [ ] 16.4 於 `CLAUDE.md` 「Repo 現況」段落補一句：Module 2 KB Builder P0 已落地（含 `KnowledgeBase` / `KBPayload` / chunker / Qdrant KB-facing 包裝）

## 17. 驗證與 commit gate

- [ ] 17.1 執行 `uv run pytest sidecar/tests/kb/`，全綠（Qdrant-touching 測試若 Qdrant 未起動可 auto-skip）
- [ ] 17.2 執行 `uv run pytest` 完整 suite，確認無 regression（既有 `tests/kb/test_qdrant_client.py`、`tests/scanner/` 等仍綠）
- [ ] 17.3 執行 `pre-commit run --all-files`，全綠
- [ ] 17.4 手動啟 Qdrant（`bash sidecar/scripts/start-qdrant.sh`）後重跑 `uv run pytest sidecar/tests/kb/test_qdrant_kb.py`，確認 KB-facing wrapper 對真 Qdrant 也通過
