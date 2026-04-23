# Module 2 — Knowledge Base Builder Spec

> 把 Scanner 輸出切 chunk、embed、存進 Qdrant，供 Explorer / Q&A 查詢。
> 關聯決策：D-015（Sanitizer）、D-016（Q&A KB growth）、D-017（Sandbox）。
> 關聯文件：`module-1-scanner.md`（唯一輸入）、`llm-provider.md`（embed）、`qa-agent.md`（add_to_kb）。

---

## 一、職責與邊界

### 負責
- 接收 Module 1 `ScanResult`
- Chunk 切分（程式碼 / 文字 / git metadata 用不同策略）
- 呼叫 Provider `embed()` 拿向量
- Qdrant collection 建立與 upsert
- 去重（content hash + similarity）
- 提供 `kb_search` / `add_to_kb` / `find_similar` 給上層

### 不負責
- 檔案遍歷與 sanitize（Module 1）
- 內容理解 / 決策（Module 4 / 8）
- Sanitizer 規則（D-015）

---

## 二、資料流

```
ScanResult (Module 1)
    │
    ▼
[A] 分類 chunk 策略（按 FileEntry.kind / language）
    │
    ▼
[B] 切 chunk → ChunkDraft[]
    │
    ▼
[C] content-hash 預先去重
    │
    ▼
[D] Batch embed（Provider）
    │
    ▼
[E] Qdrant upsert（含 metadata）
    │
    ▼
[F] 完成 build，回 KBStats
```

---

## 三、Qdrant Collection Schema

### Collection 命名
```
codebus_{workspace_id}
```
每個 workspace 獨立 collection；跨 workspace 不共用（Phase 3 才評估共用索引）。

### Vector 設定
```python
{
  "size": <EMBEDDING_DIM>,      # TODO review: 實際 embedding model 的 dim
  "distance": "Cosine"
}
```

`EMBEDDING_DIM` 由 Provider 的 `embedding_dim` 屬性取得（`llm-provider.md` §二）。

### Payload Schema（每個 point）

```python
class KBPayload(BaseModel):
    # 來源
    source_kind: Literal["code", "doc", "git_commit", "git_blame", "skeleton"]
    # "skeleton" = binary / lockfile / generated 的存在證明（§四：只存 path + meta，text 為空）
    file_path: str | None           # 相對 workspace（source_kind=code/doc/skeleton）

    # 註：Q&A Agent 透過 `add_to_kb` 補進的 chunk 仍屬 code / doc，
    # 經由 added_by="qa_agent" 區分 provenance（見下方「血統」欄位），不另開 source_kind。
    line_start: int | None
    line_end: int | None
    commit_oid: str | None          # source_kind=git_*

    # 內容
    text: str                        # 清理後（已過 Sanitizer）
    text_hash: str                   # sha256(text)，去重用
    language: str | None

    # 血統
    added_by: Literal["scanner", "qa_agent"]
    session_id: str | None           # 首次 scan 為 None；qa_agent 加入時填
    chunk_index: int                 # 同檔內第幾塊
    chunk_total: int                 # 同檔共幾塊

    # 時序
    created_at: datetime
    source_mtime: datetime | None    # 檔案最後修改時間（便於 Phase 2 增量）

    # 稽核
    sanitize_stats: dict[str, int] = Field(default_factory=dict)  # {"email": 0, ...}
    # Scanner pass 1 會填；qa_agent 若 scrub 不回 stats 允許空 dict

    # Station 脈絡（D-029）
    related_stations: list[str] = Field(default_factory=list)
    # Module 5 stable station id（`s{NN}-slug`，見 module-5-generator.md §7.4）。
    # Scanner 產出為空 []；qa_agent 透過 add_to_kb 寫入時填（見 qa-agent.md §三）。
    # 格式驗證 regex：^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$
```

**Payload index**（Qdrant payload_schema）：`related_stations` 建 keyword index，讓 `kb_search` 可做 `filter={"related_stations": "s02-storage-contract"}` 的 station-scoped 查詢（`qa-agent.md §三` 依賴此能力）。

---

## 四、Chunk 策略

### 原則
- **Token-based 滑動窗口**（非 AST）
- 預設 **chunk_size = 600 tokens**、**overlap = 60 tokens**
- Token 計算用 tiktoken 或 provider 提供的 tokenizer

**為何不用 AST**：
- 跨語言要 tree-sitter，依賴重
- MVP 準度夠用；AST-aware 效能收益在 Phase 2 評估

### 按 `FileEntry.kind` + `language` 分派

（Module 1 的 `FileEntry.kind` Literal 見 `module-1-scanner.md §十一`：`text / binary / oversized / lockfile / generated`；`symlinks` 是 `ScanResult.symlinks` 另一條列表，不在 FileEntry 內。）

| kind | language 條件 | 策略 | KBPayload.source_kind |
|---|---|---|---|
| `text` | `language ∈ {markdown, rst, asciidoc, plaintext}` | **doc 策略**：按 `##` heading 先分段，段超過 600 token 再 window 切 | `doc` |
| `text` | 其他（程式語言 / 無法判定） | **code 策略**：Token window 600/60；**尊重行邊界**（不切到半行） | `code` |
| `oversized` | — | 只 chunk `oversized_preview`（前 200 行）+ 記 `is_preview: true` | `code` / `doc`（同上判斷） |
| `binary` / `lockfile` / `generated` | — | **不 chunk**，但 upsert 一筆 skeleton payload（只存 path + meta，text 為空 placeholder）讓 Explorer 能查到「這檔存在」 | `skeleton` |
| （`ScanResult.symlinks`，非 FileEntry） | — | 不 chunk；symlink 表另存 metadata，不進向量 | — |

### Git Metadata chunk

| 來源 | chunk 方式 |
|---|---|
| `recent_commits` | 每 10 commits 一塊；格式「YYYY-MM-DD [oid] author — subject」串接 |
| `file_activity` | 整份序列化成一 chunk（repo 小）或 per-dir 分塊（repo 大）|
| `blame` | 每檔一 chunk，內容「line 1-20: author@date」簡表（不放原 code，code 已在 code chunk） |

---

## 五、去重

### 兩層

**Layer 1：content hash（精確去重）**
```python
h = sha256(normalized_text).hexdigest()
if await qdrant.exists_by_hash(collection, h):
    return "skipped_hash"
```
`normalized_text = text.strip()`，不做更激進正規化（避免誤殺）。

**Layer 2：similarity dedup（D-016 的 add_to_kb 用）**
```python
# embed 完成後、upsert 之前
hits = await qdrant.search(collection, vec, top_k=1)
if hits and hits[0].score >= SIMILARITY_DEDUP_THRESHOLD:  # 0.95
    return "skipped_similar"
```
僅 `qa_agent` 加入時啟用；初始 scan 只用 Layer 1。

### Hash 索引
在 payload 加 `text_hash` 欄位並建 Qdrant payload index（允許 `exists_by_hash` O(1) 查）。

---

## 六、Embedding Pipeline

### Batch
- Batch size **32**（視 provider rate limit 可調）
- 失敗重試 3 次（exponential backoff，由 Provider 層處理）

### 並行
- `asyncio.gather` 分批，單次 in-flight 最多 3 個 batch（`asyncio.Semaphore(3)`）
- 超過 rate limit → Provider 層 backoff → 上層 log progress stall

### Progress 回報（SSE，連動 `sidecar-api.md` §四 / §三-bis）

KB 建置的進度路徑分兩層：

1. **Source 層**（`KnowledgeBase.build(..., on_progress=…)`）：
   `KBProgressEvent(phase: Literal["chunking", "embedding", "upserting", "done"], current, total, workspace_id)`
   ——`chunking` 完成計數、`embedding` 每完成一個 batch、`upserting` 每完成一個 Qdrant
   batch、`done` 終止——共四個 source phase。
2. **Wire 層**（`api/kb.py::_KBProgressAdapter`）：把上面三個非 done phase 全部
   折疊成單一 wire phase `"embedding"`，`done` source 不對應 wire progress event
   （SSE 的終端 `done` 由 task wrapper 發出）：

```json
{ "type": "progress", "phase": "embedding", "current": 480, "total": 1200 }
```

`_KBProgressAdapter` 在第一個 non-done event 鎖定 anchor total（KB 自然從
`chunking` 開始，total 即 `chunks_emitted`），後續 wire stream 保證：

- 至少一筆 `current == 0`（`chunking` 進場時 override 成 0）
- 至少一筆 `current == anchor_total`（`upserting` 進場時 snap 到 anchor_total）
- `current` 單調非降；`embedding` 階段以 `chunks_emitted` 為分母按比例縮放，
  即使中間因 dedup 改變內在 total 也不會出現倒退。

對應 `openspec/changes/sse-progress-skeleton/specs/knowledge-base/spec.md`
Requirement `KB progress phase translation to wire schema`。

### Token 用量追蹤（D-021）
每批 `embed()` 回 `EmbedResponse(vectors, usage)`；build pipeline 呼叫 `ctx.usage_tracker.record(usage=response.usage, module="kb_build")` 寫進 `token_usage.jsonl`。Provider 沒回 token 數時以 tiktoken 估算 + `estimated=True`。詳見 `agent-core.md §十三`。

---

## 七、API（給上層用）

```python
class KnowledgeBase:
    async def build(self, scan_result: ScanResult) -> KBStats: ...

    async def query(
        self,
        text: str,
        *,
        top_k: int = 8,
        filter_path: str | None = None,
        filter_source_kind: list[str] | None = None,
    ) -> list[KBHit]: ...

    async def find_similar(
        self,
        text: str,
        *,
        threshold: float = 0.95,
    ) -> KBHit | None: ...

    async def upsert_chunk(
        self,
        text: str,
        *,
        payload: KBPayload,
    ) -> str:                       # returns point id
        """用於 D-016 `add_to_kb` tool 的底層"""

    async def delete_chunk(self, point_id: str) -> None:
        """用於 kb_growth rollback (D-016)"""

    async def stats(self) -> KBStats: ...
```

### `KBHit`
```python
class KBHit(BaseModel):
    point_id: str
    score: float
    payload: KBPayload
```

### HTTP 端點對接 — `POST /kb/build`（async, change `sse-progress-skeleton`）

對應 `openspec/changes/sse-progress-skeleton/specs/knowledge-base/spec.md`
Requirement `POST /kb/build async endpoint`。

```
POST /kb/build           # 預設 async，無同步變體
Body: { "workspace_root": "<abs>", "scan_result": <ScanResult JSON> }
→ 200 { "task_id": "kb_<hex8>" }   立即回，不阻塞 build
→ 409 { "code": "TASK_IN_FLIGHT", "running_task_id": "..." }
→ 503 { "code": "KB_NOT_CONFIGURED" }   sidecar 未注入 KB 依賴時
```

設計要點：

- 端點解析 `(backend, provider_factory, tracker_factory, embedding_dim)` 自 `app.state`
  （正式 wiring 由 `api/__init__.py::wire_kb_dependencies` 注入——見下方
  「Production wiring」段；測試以 `lambda _ws: instance` 包裝 in-memory double
  注入相同 `app.state` slot），缺任一即回 503。
- `kb_provider` 與 `kb_usage_tracker` 為 `Callable[[Path], ...]` factory
  （D-032 決策 3 A 方案）——每次 `POST /kb/build` 依 `request.workspace_root`
  分別呼叫兩個 factory，確保 `TrackedProvider` 內的 audit logger（`token_usage.jsonl`
  / `llm_calls.jsonl` / `sanitize_audit.jsonl`）全部落在正確的 workspace path。
- 進度走 `_make_kb_progress_adapter(handle)` → `KnowledgeBase.build(...,
  on_progress=…)`；wire 翻譯規則見上方 §六「Progress 回報」。
- 終端 `KBStats` 透過 `_run_background_task` wrapper 寫入 `handle.result`，
  消費端用 `GET /tasks/{id}/result` 取（詳見 `sidecar-api.md §三-bis`）。
- 任何 build 例外被 wrapper 收斂成 sanitized SSE `error` event（`KB_EMBED_FAILED`
  / `OPENAI_AUTH_FAILED` / `OPENAI_RATE_LIMITED` / `KB_DIM_MISMATCH` / `INTERNAL_ERROR`），
  不洩漏 traceback；完整 traceback 只進 logger。

### Production wiring（change `kb-build-production-wiring`, D-032）

對應 `openspec/changes/kb-build-production-wiring/specs/{knowledge-base,sidecar-runtime,llm-provider}/spec.md`。

**啟動路徑**（`api/main.py` → `create_app` → `wire_kb_dependencies`）：

```
env CODEBUS_OPENAI_API_KEY  ──┐
env CODEBUS_QDRANT_URL       ──┼─► wire_kb_dependencies(app, openai_api_key, qdrant_url)
                                │
                                ├─► app.state.kb_backend         = QdrantHttpBackend(client)
                                ├─► app.state.kb_provider        = factory(ws) -> TrackedProvider
                                ├─► app.state.kb_usage_tracker   = factory(ws) -> UsageTracker
                                └─► app.state.kb_embedding_dim   = 1536
```

**依賴注入語意**:

- **`kb_backend`（app-level 實例）**：`QdrantHttpBackend` 包 `AsyncQdrantClient`,一個 sidecar 共享一個連線
- **`kb_provider`（workspace-level factory）**：呼叫時 build `TrackedProvider(inner=OpenAIEmbeddingProvider(), tracker=UsageTracker(<ws>/token_usage.jsonl), logger=LLMCallLogger(<ws>/llm_calls.jsonl), sanitizer=SanitizerEngine(), sanitizer_audit=SanitizerAuditLogger(<ws>/.codebus/sanitize_audit.jsonl), role=EMBED, rules_version=…, default_module="kb_build")` ——`default_module` 由 `usage-tracker-dedup` 引入,讓 TrackedProvider 自動把 `module="kb_build"` 寫進 `token_usage.jsonl`,KB pipeline 不再手動 `tracker.record(...)`(避免每個 batch 被記兩次)
- **`kb_query_provider`（workspace-level factory；change `kb-query-endpoint`）**：與 `kb_provider` 結構一樣,但 `default_module="kb_query"`。`POST /kb/query` 用此 factory 取 TrackedProvider,讓查詢路徑的 embed cost 在 `token_usage.jsonl` 標 `module="kb_query"` 而非 `"kb_build"`,可由 group-by-module 把 build vs query 的成本拆開算
- **`kb_usage_tracker`（workspace-level factory）**：回 `UsageTracker(<ws>/token_usage.jsonl)`,**目前** KB pipeline 不直接寫此 tracker（記帳路徑全走 TrackedProvider 內綁的同 path tracker）。slot 保留給未來 KB 層級的非 LLM-call 統計用途（例如 chunk 計數),Phase 2+ 與 Module 4/5 對齊時再決定是否拆掉
- **`kb_embedding_dim`（app-level 常數）**：`OpenAIEmbeddingProvider` 宣告的 `OPENAI_EMBEDDING_DIM = 1536`

**Graceful degrade 政策**（D-032 決策 2）：

| 狀況 | 行為 |
|---|---|
| `CODEBUS_OPENAI_API_KEY` 未設 | sidecar 正常啟動；`kb_provider` / `kb_embedding_dim` 留 `None`；`POST /kb/build` 回 503 `KB_NOT_CONFIGURED`；`/healthz` `openai_embedding.status = "not-configured"` |
| env 有設但 OpenAI auth 失敗 | smoke probe 在啟動時發現;`/healthz` `openai_embedding.status = "degraded"`;`POST /kb/build` 仍允許送出但會在 build 中 raise 並被包成 `OPENAI_AUTH_FAILED` SSE error |
| env 有設且 OpenAI 通 | `/healthz` `openai_embedding.status = "ok"`,happy path 跑完回 KBStats |
| Qdrant 既有 collection dim 不符 | `KnowledgeBase.build()` 在 chunking 完 / embed 前呼 `backend.ensure_collection(expected_dim)` 擋下 → `KBDimMismatchError` → SSE error event 帶 `expected_dim` / `actual_dim` / `suggestion` |

**Healthz smoke probe 例外**（D-032 決策 3）：`/healthz` 的 `openai_embedding` 狀態由啟動時的 raw `OpenAIEmbeddingProvider.embed(["ping"])` 決定——**不經 TrackedProvider**,因為 workspace path 此時還不知道,且健康檢查不是 production traffic。結果 cache 於 `app.state.openai_embedding_probe`,`/healthz` 不每次都打 OpenAI。

---

## 八、Growth Mechanism（D-016 連動）

`qa_agent` 呼叫 `add_to_kb` 時走：

```
input chunks (from Q&A Agent)
    │
    ▼
Sanitizer 寫入前 (D-015 Pass 3)
    │
    ▼
KnowledgeBase.upsert_chunk()
    │
    ├─▶ Layer 1 hash check
    ├─▶ embed()
    ├─▶ Layer 2 similarity dedup (threshold 0.95)
    ├─▶ Qdrant upsert (added_by="qa_agent", session_id=...)
    │
    ▼
KBGrowthLogger.write()  → kb_growth.jsonl
```

### 防呆（與 qa-agent.md §七 對齊）
- 單 session add_to_kb 筆數上限 20
- 單 chunk 最大 chars 2000
- similarity threshold 0.95
- 超限 `upsert_chunk` raise `KBGrowthExceeded`，qa_agent 收 error 回 Agent prompt 指示收斂

---

## 九、效能 Target

| 規模 | 目標 |
|---|---|
| 500 檔、總 2MB | < 30s |
| 2000 檔、總 10MB | < 2 min |
| 5000 檔、總 30MB | < 5 min |

**瓶頸假設是 embedding API rate limit**，非 CPU / Qdrant。

### 成本 benchmark（D-007 連動）
實作完成後對 Demo repo 跑 cost benchmark：總 tokens / 耗時 / 金額，寫回 decisions.md D-007。

---

## 十、失敗處理

| 情況 | 處理 |
|---|---|
| Embedding API 429 | Provider 層 backoff；3 次後 raise，build job fail |
| Qdrant 連不上 | sidecar startup 時 health check；build 期間連線斷 → retry 3 次後 fail |
| 單 chunk 超出 model max input | 二切（對半）重 chunk；仍超 → skip + log warning |
| Text_hash collision（不同文字相同 hash） | 極低機率，不特別處理（SHA256 碰撞） |
| Workspace 已有同名 collection | 預設**先 drop 再 build**（rebuild 語意）；config `preserve_qa_chunks: true` 可保留 `added_by=qa_agent` 的 point |

---

## 十一、測試

### 單元
- Chunk 函式：給定文字與 size/overlap，驗證切法正確、無行內截斷
- Payload schema Pydantic validation
- Hash dedup（同文字兩次 upsert 只剩一筆）
- Similarity dedup（高相似內容跳過）

### Integration fixture
- Timeline（Demo repo）掃 + build 一次，驗證：
  - Chunk 總數在合理範圍
  - `IStorageService` 關鍵字 kb_search 能命中 `types/index.ts`
  - `find_callers` 對 `IStorageService` 能回兩個 Adapter 實作

### 效能 benchmark
`tests/perf/module_2/` 三規模測試，CI 跑記錄 baseline。

---

## 十二、MVP 不做

| 項 | 延後原因 |
|---|---|
| AST-aware chunk（tree-sitter） | 依賴重，MVP token window 夠 |
| 增量 build（file-level diff） | 配合 Scanner Phase 2 的 file watch |
| 跨 workspace 共用 collection | Phase 3 |
| 多 embedding model 同時跑 A/B | Phase 3 |
| 自動 chunk size tuning | Phase 3 |
| Hybrid search（向量 + BM25） | Phase 2 評估 |
| Embedding 快取（同檔未改不重 embed） | 增量 build 時一起做 |

---

## 十三、實作順序

| 優先 | 項目 | 工期 |
|---|---|---|
| P0 | Qdrant client wrapper（connect / ensure_collection / upsert / query）<br>※ connect / ensure_collection 由 change `qdrant-lifecycle-bootstrap` 交付；upsert / query 屬 Module 2 range — 2026-04-21 落地（change `module-2-kb-builder-p0`） | 0.5d |
| P0 | KBPayload schema + payload index — 2026-04-21 落地（change `module-2-kb-builder-p0`） | 0.25d |
| P0 | Chunk 函式（token window + 行邊界 respect） — 2026-04-21 落地（change `module-2-kb-builder-p0`） | 0.5d |
| P0 | Build pipeline（ScanResult → chunk → embed → upsert） — 2026-04-21 落地（change `module-2-kb-builder-p0`） | 0.5d |
| P0 | content-hash 去重 — 2026-04-21 落地（change `module-2-kb-builder-p0`） | 0.25d |
| P0 | `query` / `find_similar` API — 2026-04-21 落地（change `module-2-kb-builder-p0`） | 0.25d |
| P0 | SSE progress emit — 2026-04-21 落地（change `module-2-kb-builder-p0`，progress callback；SSE wire 由 Module 1/2 step 15 接續） | 0.25d |
| P1 | Git metadata chunk（commits / activity / blame） | 0.5d |
| P1 | `upsert_chunk` / `delete_chunk`（D-016 後端） | 0.5d |
| P1 | rebuild 保留 qa_agent chunks 選項 | 0.25d |
| P1 | Integration fixture + Timeline benchmark | 0.5d |

**合計 P0 ~2.5d / P0+P1 ~4.25d。**
