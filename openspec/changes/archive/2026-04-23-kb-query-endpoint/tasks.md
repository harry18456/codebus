## 1. Scaffolding

- [x] 1.1 於 `sidecar/src/codebus_agent/api/kb.py` 預留 `KBQueryRequest` Pydantic model 與 `kb_query_endpoint` placeholder（`@router.post("/kb/query")` 先 return 503，待 Section 3 GREEN 補實作）

## 2. RED — `POST /kb/query endpoint` 規約測試

對應 spec `knowledge-base / POST /kb/query endpoint` 與 `sidecar-runtime / KB query endpoint registration`。

- [x] 2.1 [P] 於 `sidecar/tests/api/test_kb_query.py` 加 `test_query_returns_hits_ordered_by_score`（POST /kb/query endpoint Scenario "Successful query returns hits ordered by score"；用 InMemoryQdrantBackend + SpyProvider 預埋幾個 point 後查）
- [x] 2.2 [P] 於 `test_kb_query.py` 加 `test_empty_collection_returns_200_with_empty_hits`（POST /kb/query endpoint Scenario "Empty collection returns empty hits list with 200"；workspace 沒 build 過 → 200 `{"hits": []}` 而非 404）
- [x] 2.3 [P] 於 `test_kb_query.py` 加 `test_missing_openai_key_returns_503`（POST /kb/query endpoint Scenario "Missing OpenAI API key returns 503 KB_NOT_CONFIGURED"；不注入 kb_query_provider，斷言 503 + `code: "KB_NOT_CONFIGURED"`）
- [x] 2.4 [P] 於 `test_kb_query.py` 加 `test_invalid_body_returns_422`（POST /kb/query endpoint Scenario "Invalid request body returns 422"；缺 text、`top_k=0`、`top_k=51` 三個 case 各一）
- [x] 2.5 [P] 於 `test_kb_query.py` 加 `test_filter_path_narrows_results`（POST /kb/query endpoint Scenario "filter_path narrows results in HTTP path"）
- [x] 2.6 [P] 於 `test_kb_query.py` 加 `test_bearer_required`（POST /kb/query endpoint Scenario "Bearer token required"；無 header → 401，未進 endpoint）
- [x] 2.7 [P] 於 `test_kb_query.py` 加 `test_query_records_usage_with_module_kb_query`（POST /kb/query endpoint Scenario "Query usage recorded with module=kb_query"；用 TrackedProvider with default_module="kb_query" 包 SpyProvider，斷言 token_usage.jsonl 有 `module="kb_query"` 行）

## 3. GREEN — 實作 `POST /kb/query endpoint`

對應 spec `knowledge-base / POST /kb/query endpoint`。

- [x] 3.1 於 `api/kb.py` 實作 `KBQueryRequest` Pydantic model：`workspace_root: str`、`text: str`、`top_k: int = Field(default=8, ge=1, le=50)`、`filter_path: str | None = None`、`filter_source_kind: list[str] | None = None`、`extra="forbid"`
- [x] 3.2 於 `api/kb.py` 寫 `_require_query_deps(request)` helper：解析 `app.state.kb_backend` / `kb_query_provider` / `kb_usage_tracker` / `kb_embedding_dim`，缺任一回 503 `KB_NOT_CONFIGURED`（與 `_require_kb_deps` 共用 missing 列表 pattern）
- [x] 3.3 於 `api/kb.py` 實作 `kb_query_endpoint` handler：build `KnowledgeBase`、呼 `kb.query(text, top_k=..., filter_path=..., filter_source_kind=...)`、回 `{"hits": [hit.model_dump(mode="json") for hit in result]}`
- [x] 3.4 於 `api/kb.py::__all__` 加上 `KBQueryRequest` / `kb_query_endpoint`
- [x] 3.5 執行 `uv run pytest sidecar/tests/api/test_kb_query.py -v` 確認 2.1 ~ 2.7 全綠

## 4. RED — `KB query endpoint registration` 規約測試

對應 spec `sidecar-runtime / KB query endpoint registration`。

- [x] 4.1 [P] 於 `sidecar/tests/test_wire_kb_dependencies.py` 加 `test_query_provider_factory_uses_kb_query_module`（KB query endpoint registration Scenario "Both KB build and KB query slots present after wiring"；驗 `app.state.kb_query_provider` 是 callable，invoke 後回 TrackedProvider 帶 `_default_module == "kb_query"`、與 `kb_provider` 是不同 instance）
- [x] 4.2 [P] 於 `test_wire_kb_dependencies.py` 加 `test_missing_openai_key_leaves_query_provider_none`（KB query endpoint registration Scenario "Missing OpenAI API key leaves both provider slots None"）

## 5. GREEN — 實作 `KB query endpoint registration`

對應 spec `sidecar-runtime / KB query endpoint registration`。

- [x] 5.1 於 `api/__init__.py` 加 `_make_query_provider_factory()` helper：與既有 `_make_provider_factory` 相同結構但 `default_module="kb_query"`（採 refactor:把 `_make_provider_factory` 改吃 `default_module` 參數,build/query 共用同一 factory function 但不同 module 標籤,避免重複碼）
- [x] 5.2 於 `api/__init__.py::wire_kb_dependencies` 在 `openai_api_key` 有設時設定 `app.state.kb_query_provider = _make_query_provider_factory()`；無設時 `None`
- [x] 5.3 執行 `uv run pytest sidecar/tests/test_wire_kb_dependencies.py -v` 確認 4.1 / 4.2 全綠 + 既有 5 條測無 regression

## 6. 文件與 housekeeping

- [x] 6.1 於 `docs/sidecar-api.md` 補 `/kb/query` 條目：method / body schema / 200 / 503 / 422 / 401 對應，連動 `kb-query-endpoint`
- [x] 6.2 於 `docs/module-2-kb-builder.md §七` Production wiring 段補 `kb_query_provider` slot 說明（與 `kb_provider` 並列、module 標籤分離）
- [x] 6.3 於 `CLAUDE.md` 「最近一筆 in-progress」改指 `kb-query-endpoint`，sidecar 描述補「`POST /kb/query` 同步 KB 查詢端點 + `kb_query_provider` factory(`default_module="kb_query"`)」

## 7. 驗證與 commit gate

- [x] 7.1 執行 `uv run pytest sidecar/tests/api/` 確認 API 層全綠（40 passed）
- [x] 7.2 執行 `uv run pytest sidecar/tests/` 完整 suite 無 regression（519 passed, 9 skipped）
- [x] 7.3 執行 `pre-commit run --all-files` 全綠
- [x] 7.4 手動煙霧測：`POST /kb/build` 跑完 → `POST /kb/query` body 帶同 workspace_root 與一段 query text → 收到 `{"hits": [...]}`、檢 `<workspace>/token_usage.jsonl` 多一筆 `operation="embed"` / `module="kb_query"` 行（非 `kb_build`），驗 build vs query cost 分離記錄 ✅（/kb/query 回 3 hits score 遞減 0.60/0.23/0.22;token_usage.jsonl 1 行 `kb_build` $0.000022 + 1 行 `kb_query` $0.00000006,module 分離乾淨,無重複）
