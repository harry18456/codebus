## Context

Module 2 KB Builder 是 CodeBus 資料層的骨幹：它把 Module 1 Scanner 產的 `ScanResult` 切 chunk、embed、寫進本地 Qdrant collection；Explorer Agent（步驟 17）與 Q&A Agent（步驟 25）都靠它的 `query()` / `find_similar()` 才能做 RAG。`docs/module-2-kb-builder.md` 已把 spec 寫完整（§一 ~ §十三），本次 change 做 **§十三 P0** 的實作落地，不改 spec 文字、不動 P1。

現狀前置條件（2026-04-21 已就緒）：
- `qdrant-client` capability 已提供 `resolve_url` / `build_client` / `probe` / `ensure_collection`（`openspec/specs/qdrant-client/spec.md`，change `qdrant-lifecycle-bootstrap` 2026-04-21 archive）。
- `llm-provider` capability 已提供 `LLMProvider` Protocol + `MockProvider.embed()` + `TrackedProvider` 裝飾 + `EmbedResponse.usage`（M1 archive）。
- `folder-scanner` capability 已產出 `ScanResult` / `FileEntry`，`FileEntry.content` 保證經過 Sanitizer Pass 1（change `scanner-sanitizer-orchestration` 2026-04-21 archive）。
- `usage-tracking` capability 已提供 `UsageTracker.record(usage, module=...)` 寫 `token_usage.jsonl`。

約束：
- **Runtime 不得直接 import `qdrant_client` SDK**（`qdrant-client` capability 既有 Requirement）—— 所有新增包裝必須寫在 `codebus_agent.kb.qdrant_client` module 內。
- **Provider 必包 TrackedProvider**（核心不變式 4）—— 呼叫 `embed()` 時不得繞過 registry。
- **Zero outbound 不變式（M1）**—— 測試階段只能用 `MockProvider.embed()`，不可真外呼。
- **SSE endpoint 不在 scope**（步驟 15 才做）—— 本次只定義 progress callback 協定。

## Goals / Non-Goals

**Goals:**

- 把 `module-2-kb-builder.md §十三` 的 P0 七項全部落地，內含 chunk、embed pipeline、content-hash 去重、query API、progress callback。
- 所有新增模組都有單元測試覆蓋，且測試不依賴真 Qdrant / 真 embedding API（以 in-memory fake + MockProvider 為主路徑）。
- 保持 `codebus_agent.kb.qdrant_client` 作為 SDK 唯一入口，runtime 其他模組繼續透過它間接呼叫。
- Progress callback 設計要讓後續步驟 15（SSE wire）能零改動串接。

**Non-Goals:**

- 不做 git metadata chunk（`module-2-kb-builder.md §四` git 部分屬 P1）。
- 不做 similarity dedup Layer 2（0.95 threshold，僅 `qa_agent add_to_kb` 用，屬 D-016 後端 P1）。
- 不做 `upsert_chunk` / `delete_chunk`（D-016 後端 P1）。
- 不做 HTTP endpoint / SSE 串線（步驟 15）。
- 不做 Timeline integration fixture 與 D-007 cost benchmark（需真 embedding provider）。
- 不挑 AST-aware chunker、增量 build、embedding 快取（`module-2-kb-builder.md §十二` 明示 Phase 2）。

## Decisions

### Tokenizer 選 tiktoken，不要求 provider 提供

**選項 A（選用）**：統一走 `tiktoken.encoding_for_model("cl100k_base")`。
**選項 B**：讓 `LLMProvider` 暴露 `tokenize(text) -> list[int]`，chunker 透過 provider 算 token。

選 A，理由：
- Chunker 要離線決定切法，不應跟 provider 耦合；若未來換 provider（`role="embedding"` 用不同模型）只要 tokenizer 近似即可，chunk 邊界不必 byte-for-byte 對齊。
- Tiktoken `cl100k_base` 是 OpenAI / Anthropic 近期模型的共同近似值，誤差容忍。
- 選 B 會讓 MockProvider 也得寫 tokenize 邏輯，測試複雜度上升。
- `EmbedResponse.usage.prompt_tokens` 仍由 provider 實測回報，`UsageTracker` 存真 token 數，不受 tiktoken 誤差影響。

**不新增 D-編號 ADR**：此決策範圍侷限於 Module 2 內部；不影響 provider 抽象也不牽扯其他模組。若 Phase 2 換 AST-aware 策略需要重新評估，屆時再開。

### Chunk 策略以 FileEntry.kind + language 單表分派

**選項 A（選用）**：`chunker.dispatch(file_entry) -> ChunkDraft[]` 依一張固定 dispatch table 路由到 `_code_strategy` / `_doc_strategy` / `_skeleton_strategy` / `_oversized_strategy`。
**選項 B**：Strategy 類別繼承（abstract base + 4 subclass）。

選 A，理由：
- 策略數量（4 種）且穩定；類別階層增 boilerplate 無收益。
- Dispatch table 容易在單元測試裡遍歷所有 kind×language combo。
- 選 B 若日後加 AST-aware 策略再抽 class 也不遲。

### content-hash normalization 只 strip

沿用 `module-2-kb-builder.md §五`：`normalized_text = text.strip()`。不做 lowercase / whitespace collapse / 註解移除等激進 normalize，避免「同一行 code 不同縮排但 semantic 不同」被誤判為重複。SHA256 collision 機率忽略不計。

### Progress callback 協定設計

採 **async callback + 結構化事件** 而非 queue：

```python
class KBProgressEvent(BaseModel):
    phase: Literal["chunking", "embedding", "upserting", "done"]
    current: int
    total: int
    message: str | None = None
    workspace_id: str

ProgressCallback = Callable[[KBProgressEvent], Awaitable[None]]

class KnowledgeBase:
    async def build(
        self,
        scan_result: ScanResult,
        *,
        on_progress: ProgressCallback | None = None,
    ) -> KBStats: ...
```

理由：
- 步驟 15（SSE wire）只要在 endpoint handler 內建一個 `on_progress` 把 event JSON-encode 後 push 進 SSE stream 即可，零改動 Module 2。
- 單元測試可用 list.append 當 callback，抓所有 event 做 assertion，不需 mock queue。
- Async callback 比 sync callback 開放度高（SSE pipeline 本身是 async）。

### `KnowledgeBase` 建構時綁定 workspace，不採全域 singleton

簽名：`KnowledgeBase(*, client: AsyncQdrantClient, provider: LLMProvider, usage_tracker: UsageTracker, workspace_id: str, embedding_dim: int)`。

- `workspace_id` 決定 Qdrant collection name：`codebus_{workspace_id}`（spec §三）。
- Provider 拿的是 `registry.get(role="embedding")`（`llm-provider` role routing）。
- 不做全域 singleton，避免跨 workspace 串音；sidecar 在 POST /kb/build 時 per-request 建一個。

### `workspace_id` 算法

`workspace_id = sha256(workspace_root).hexdigest()[:16]`。

理由：
- Qdrant collection name 不能含特殊字元；直接拿 path 當 id 會有跨平台問題（`C:\Users\x` vs `/home/x`）。
- 同一個 workspace_root 兩次 build 產同一 collection，天然支援 rebuild（本次不做 `preserve_qa_chunks`，預設 drop-and-recreate）。
- 16 hex chars（64 bits）碰撞機率對單機 workspace 數量級完全夠用。

### Embedding batch in-flight 限制 3

採 `asyncio.Semaphore(3)`：上限 3 個 batch × 32 chunks = 96 個向量同時計算中。

- 太低（1）：bandwidth 浪費；進度條卡頓。
- 太高（>10）：MockProvider 無意義；真 provider 會撞 rate limit。
- 3 是 `module-2-kb-builder.md §六` 既定值，對齊 spec。

### Qdrant payload index 建立時機

在 `ensure_collection` 之後、第一次 upsert 之前，**冪等地** 呼叫 `create_payload_index(collection, field="text_hash", schema="keyword")` 與 `create_payload_index(collection, field="related_stations", schema="keyword")`。

- Qdrant SDK 的 create_payload_index 已內建 idempotency（既存就回 OK），不需自己 check-then-create。
- 包成 `_ensure_kb_payload_indices(client, collection)` helper 放在 `qdrant_client.py`，讓 `KnowledgeBase.__init__` 一次呼叫。

### Qdrant 離線測試策略

不啟 Qdrant 即能跑測試：抽 `KBQdrantBackend` Protocol（`upsert_points` / `search_points` / `exists_by_hash` / `ensure_indices` / `drop_collection`），生產用 `AsyncQdrantBackend`（包 SDK），測試用 `InMemoryQdrantBackend`（dict + 餘弦手算）。

- 既有 `sidecar/tests/kb/` 已有 `test_qdrant_client.py` 需真 Qdrant（auto-skip），本次新增測試不走那條路。
- Protocol 拆法對齊 `agent-core.md` 的 Judge / CoverageChecker Protocol 風格。

## Risks / Trade-offs

- **[Tiktoken token 數與真 provider 不完全一致] → 緩解**：chunk 邊界是 approximation，實際 prompt_tokens 以 provider 回報為準寫 audit；`usage_tracker.record` 用 `EmbedResponse.usage`，不用 tiktoken 估算（除非 provider 沒回）。
- **[In-memory Qdrant backend 與真 Qdrant 行為差異] → 緩解**：Protocol 只暴露本次真正需要的 4 個操作；契約測試（同一批 assertion 同時跑 InMemory 與 Real backend）留 P1，先靠 code review 保證 InMemory 實作不抄捷徑。
- **[Drop-and-rebuild 會丟 qa_agent chunks] → 緩解**：本次 P0 預設走 drop-and-recreate（spec §十），文檔清楚標示；`preserve_qa_chunks: true` 選項屬 P1，D-016 上線前使用者不會有 qa_agent chunks，風險為零。
- **[Text_hash collision 把不同文字誤判相同] → 緩解**：SHA256 實務碰撞機率 `2^-128`，不處理；選 normalize 只 strip 亦降低人為誤殺。
- **[Workspace_id 算法若改變會讓舊 collection 孤立] → 緩解**：`workspace_id = sha256(workspace_root)[:16]` 寫進 `knowledge-base` spec Requirement，後續要改需走 breaking change change proposal 並寫 migration。
- **[Embedding batch 超出 model max input] → 緩解**：`module-2-kb-builder.md §十` 規定二切重 chunk、仍超 skip + warning；本次 P0 實作此 fallback。
- **[Progress callback 被 slow consumer 拖慢 build] → 緩解**：callback 是 `async`，consumer 自己決定是否 `await`；Module 2 `await` 完才進下一批，不會 unbounded queue up。

## Migration Plan

本次無資料遷移（沒有既有 collection 要升級）；唯一需注意的是：
- 原 `sidecar/src/codebus_agent/kb/qdrant_client.py` 已有 `resolve_url` / `build_client` / `probe` / `ensure_collection`；本次**擴充**同檔加 `upsert_points` / `search_points` / `exists_by_hash` / `ensure_payload_indices` 與 `QdrantCollectionSchemaError` 同層的 `KBQdrantError`。**不改** 既有函式簽名，不影響 `qdrant-lifecycle-bootstrap` 既有測試。
- 若 apply 期間任一 task 偵測到既有 `codebus_*` collection（本機開發殘留），第一次 build 走 drop-and-recreate 的行為會清掉它——無稽核風險（P0 階段還沒有 qa_agent chunks），但建議 apply 前先手動清 `~/.codebus/kb/collection/` 以免干擾 benchmark。

Rollback：本次變更是純新增檔案 + 擴充既有 helper，無 DB schema 破壞性變更；若 apply 完發現問題，revert commit 即可，Qdrant 側若已 upsert 測試資料，手動 `drop_collection(codebus_*)` 清掉。

## Open Questions

- **`workspace_id` 算法是否要考慮 `workspace_type` discriminator**？目前以 `workspace_root` sha256；Phase 2 加 `topic` mode 時，topic workspace 無 `workspace_root`，屆時需擴充為 `sha256(f"{workspace_type}:{source_key}")[:16]`——本次不處理，但 `knowledge-base` spec Requirement 文字要留 extension point。
- **Oversized file 的 `is_preview: true` 標記要放哪**？目前規劃放 `KBPayload.sanitize_stats` 同級新增 `chunk_flags: list[str]` 欄位（值 `["preview"]`）。spec phase 決定。
