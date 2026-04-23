## Why

`module-2-kb-builder-p0` 落地了 `KnowledgeBase.query()` / `find_similar()` 的 Python 層 API，`kb-build-production-wiring` 把寫入端串到真實 OpenAI embed。但**讀取端沒有任何 HTTP 端點**——Tauri / 前端 / 任何 sidecar 外的 caller 都無法查詢已建好的 KB。Module 4 Explorer Agent（D-012 ReAct loop）依賴 KB query 做檢索，沒這條 endpoint 它無從下手。本 change 把 `KB.query()` 接上 `POST /kb/query`，補完 KB pipeline 的 round-trip：scan → build → query。

對齊 `docs/decisions.md` D-003（LLM provider 抽象）、D-021（usage tracking 套到 query 的 embed call）、D-027（Qdrant 走 local binary）。

## What Changes

- **新增 `POST /kb/query` endpoint**（`api/kb.py`）——同步 JSON 回應（不走 SSE，query 通常 < 1s，SSE 複雜度不值得）
- **Request body**：`{workspace_root: str, text: str, top_k: int = 8, filter_path: str | None = None, filter_source_kind: list[str] | None = None}`
- **Response body**（200）：`{"hits": [KBHit, ...]}`，每個 `KBHit` 含 `point_id` / `score` / `payload`（既有 `KBPayload` schema）
- **錯誤回應**：
  - `503 KB_NOT_CONFIGURED`（`app.state.kb_provider` 是 None；同 `POST /kb/build` 的 graceful degrade 契約）—— query 需要 embed `text` 成向量,沒 provider 不能跑
  - `422`（Pydantic validation；body 缺欄、`top_k <= 0` 等）
  - `200 {"hits": []}`（合法 query,但 collection 不存在或無 hit；不回 404,讓 caller 只處理一種「無結果」狀態）
- **依賴注入**:複用 `kb-build-production-wiring` 的 `_require_kb_deps` 拿 backend / tracker_factory / embedding_dim,但 **provider 走獨立的 query factory**——`wire_kb_dependencies` 多塞一個 `app.state.kb_query_provider` slot,factory 內 build TrackedProvider 帶 `default_module="kb_query"`(對齊 `usage-tracker-dedup` 的 module 標籤分離,讓 `token_usage.jsonl` 能區分 build cost vs query cost)
- **無新 env var**、**無新 dependency**(Qdrant SDK + OpenAI client 已在用)

## Non-Goals

- **`POST /kb/find_similar` 端點**：M2 沒 caller(`add_to_kb` dedup 屬 Module 8 Q&A Agent),M2 落地會反過來誘發過早設計。Q&A change 上場時再補
- **SSE streaming 邊查邊吐**：query 通常 < 1s,SSE setup 成本(EventSourceResponse + 訂閱者 lifecycle)不划算。長 query 屬 Module 4 多步檢索的事,單次 query 同步即可
- **跨 workspace 查詢**:`POST /kb/query` 一次只查單一 workspace 的 collection。多 workspace federation 不在 M2 scope
- **Query result pagination / cursor**:M2 top_k ≤ 50(下方 Non-Goal 上限),客戶端拿一批就夠;cursor 設計屬 Module 7 前端表格分頁時再評估
- **快取查詢結果**:每次 query 都實打 Qdrant + 實打 OpenAI embed query string;cache 屬 perf 優化,Module 4 上場後若有 hot path 再加
- **Query Sanitizer Pass 2 例外處理**:query 字串走 TrackedProvider 內既有 Pass 2 sanitize（既有契約,不變動）。本 change 不引入新的 sanitize 路徑
- **Workspace 不存在 / collection 未 build 時的 404**:回 200 `{"hits": []}`,讓 caller 邏輯單一(D-016 add_to_kb 也是「找不到就空陣列」精神對齊)
- **`top_k > 50`**:hard cap 50(Pydantic `Field(le=50)`),避免誤觸發大查詢。實際使用通常 ≤ 16

## Capabilities

### New Capabilities

(無)

### Modified Capabilities

- `knowledge-base`：新增 `POST /kb/query endpoint` Requirement——HTTP 層契約(request / response shape、503 / 422 / 200 三條路徑、與 `KnowledgeBase query and find_similar API` Python 層 Requirement 的對接點)
- `sidecar-runtime`：新增 `KB query endpoint registration` Requirement——bearer middleware + `app.include_router(kb_router)` 既有 wiring 即覆蓋,但 Requirement 明文記錄 `/kb/query` MUST 走同層 bearer auth、MUST 用既有 `_require_kb_deps` 解析 deps

## Impact

- **受影響 spec**:`openspec/specs/{knowledge-base,sidecar-runtime}/spec.md`(皆 delta)
- **受影響 code**:
  - `sidecar/src/codebus_agent/api/kb.py`(新 `kb_query_endpoint` handler + `KBQueryRequest` Pydantic model + `_require_query_deps` helper)
  - `sidecar/src/codebus_agent/api/__init__.py`(`wire_kb_dependencies` 新增 `kb_query_provider` factory slot,帶 `default_module="kb_query"`)
- **受影響測試**:
  - `sidecar/tests/api/test_kb_query.py`(新檔)——cover happy path / 503 / empty / filter 四條
- **受影響文件**:
  - `docs/sidecar-api.md`(補 `/kb/query` 條目)
  - `docs/module-2-kb-builder.md §七`(production wiring 段補 query 路徑說明)
  - `CLAUDE.md` Repo 現況 sidecar 描述補 `POST /kb/query` + in-progress pointer
- **無新依賴**、**無 env var 變動**、**無 ADR 新增**(D-003 / D-021 已涵蓋)
