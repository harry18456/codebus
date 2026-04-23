## 1. Scaffolding 與 ADR（對應 `implementation-plan.md` 步驟 15 之後新增 15.5）

- [x] 1.1 於 `docs/decisions.md` 新增 D-032 落實「決策 1：embedding provider 選 OpenAI `text-embedding-3-small`（dim 1536）」（列考慮過的選項 + 不做本地 provider 的理由）
- [x] 1.2 於 `sidecar/pyproject.toml` 確認 `openai>=1.0` 依賴存在並 `uv sync`（dep check；若已在則 no-op）
- [x] 1.3 於 `sidecar/src/codebus_agent/providers/openai_embedding.py` 建空模組（OpenAI embedding provider 類別 stub + `__all__`）
- [x] 1.4 於 `sidecar/src/codebus_agent/api/__init__.py` 預留 KB dependency injection hook placeholder（`def wire_kb_dependencies(...)  -> None: ...`），尚未 include 進 `create_app`

## 2. RED — `OpenAI embedding provider` 規約測試

對應 spec `llm-provider / OpenAI embedding provider` + 決策 5「OpenAI API key 只吃 env var」+ 決策 6「retry / backoff 策略委派給 Provider 層」。

- [x] 2.1 [P] 於 `sidecar/tests/providers/test_openai_embedding.py` 加 `test_embed_returns_dim_1536_vectors`（用 `respx` mock OpenAI HTTP，斷言回傳 dim）
- [x] 2.2 [P] 於 `test_openai_embedding.py` 加 `test_missing_env_var_blocks_construction`（斷言明確錯誤訊息提到 `CODEBUS_OPENAI_API_KEY`、不 fallback `OPENAI_API_KEY`）
- [x] 2.3 [P] 於 `test_openai_embedding.py` 加 `test_401_maps_to_openai_auth_failed`（mock 401，斷言拋出的 exception 類型 + `_classify_exception` mapping）
- [x] 2.4 [P] 於 `test_openai_embedding.py` 加 `test_429_after_retries_maps_to_openai_rate_limited`（mock 429*4，斷言決策 6：retry / backoff 策略委派給 Provider 層 的 max retries 用完 + mapping）
- [x] 2.5 [P] 於 `test_openai_embedding.py` 加 `test_retry_attempts_each_recorded_in_token_usage`（決策 6：retry / backoff 策略委派給 Provider 層，不在 KB pipeline 做 的 per-attempt tracking）
- [x] 2.6 [P] 於 `test_openai_embedding.py` 加 `test_registry_rejects_unwrapped_openai_provider`（registry guard 對新 provider 仍生效）

## 3. GREEN — 實作 `OpenAI embedding provider`

對應 spec `llm-provider / OpenAI embedding provider` 與決策 1「embedding provider 選 OpenAI」。

- [x] 3.1 於 `providers/openai_embedding.py` 實作 `OpenAIEmbeddingProvider` 類別（`embed(texts) -> EmbedResponse`，用 `openai>=1.0` AsyncClient，model `text-embedding-3-small`）
- [x] 3.2 於 `providers/openai_embedding.py` 定義 `OpenAIAuthError` / `OpenAIRateLimitError` 例外類別（並於 `api/tasks.py::_classify_exception` 加入 mapping 到 `OPENAI_AUTH_FAILED` / `OPENAI_RATE_LIMITED` 並擴充 `ERROR_CODES`）
- [x] 3.3 於 `providers/openai_embedding.py` 落實 env var 解析（只讀 `CODEBUS_OPENAI_API_KEY`，缺即 raise；對齊決策 5：OpenAI API key 只吃 env var，不寫入任何持久化）
- [x] 3.4 於 `providers/registry.py` 補 `register_embedding(provider)` helper（若尚無），確保 `TrackedProvider` guard 適用新 provider ——（決策：既有 `ProviderRegistry(providers: dict)` 已足夠，RED 測 2.6 即用此 pattern；改為擴充 `TrackedProvider.ALLOWED_INNER_TYPES` 納入 `OpenAIEmbeddingProvider`，不另外加 helper 避免多重 API 路徑）
- [x] 3.5 執行 `uv run pytest sidecar/tests/providers/test_openai_embedding.py` 確認 2.1 ~ 2.6 全綠

## 4. RED — `KB dependency injection hook` 規約測試

對應 spec `sidecar-runtime / KB dependency injection hook`。

- [x] 4.1 [P] 於 `sidecar/tests/test_wire_kb_dependencies.py` 加 `test_wires_all_four_slots_when_env_present`（驗 KB dependency injection hook 四個 slot 都填）
- [x] 4.2 [P] 於 `test_wire_kb_dependencies.py` 加 `test_missing_openai_key_leaves_provider_none_but_qdrant_wired`（KB dependency injection hook 依決策 2：missing API key → graceful degrade（維持 503）而非啟動失敗）
- [x] 4.3 [P] 於 `test_wire_kb_dependencies.py` 加 `test_usage_tracker_slot_is_factory_not_instance`（KB dependency injection hook 依決策 3：UsageTracker 用 factory 注入，而非預建實例；斷言 slot 是 callable）
- [x] 4.4 [P] 於 `test_wire_kb_dependencies.py` 加 `test_kb_provider_slot_is_factory_returning_tracked_provider`（KB dependency injection hook 依 A 方案：provider 也是 factory；回傳 TrackedProvider 包 OpenAIEmbeddingProvider with EMBED role）
- [x] 4.5 [P] 於 `test_wire_kb_dependencies.py` 加 `test_healthz_reports_openai_embedding_dependency_states`（KB dependency injection hook 的 healthz 擴充，三種狀態：ok / degraded / not-configured）

## 5. GREEN — 實作 `KB dependency injection hook`

對應 spec `sidecar-runtime / KB dependency injection hook`。

- [x] 5.1 於 `api/__init__.py` 實作 KB dependency injection hook 主體 `wire_kb_dependencies(app, *, openai_api_key, qdrant_url)`（env-driven 構造 provider + backend + tracker factory + embedding_dim）
- [x] 5.2 於 `api/__init__.py` 把 KB dependency injection hook 接進 `create_app`（依決策 2：missing API key → graceful degrade（維持 503）而非啟動失敗，qdrant_url 非 None 時 + openai_api_key 非 None 時各自獨立 wire；缺任一留 None）
- [x] 5.3 於 `api/main.py` 依決策 5：OpenAI API key 只吃 env var，不寫入任何持久化 讀 `CODEBUS_OPENAI_API_KEY` env var 後傳入 `create_app`；無值時傳 None
- [x] 5.4 於 `api/kb.py::_require_kb_deps` 依決策 3：UsageTracker 用 factory 注入，而非預建實例 改回傳 tracker factory（`Callable[[Path], UsageTracker]`）；端點內用 `request.workspace_root` 呼叫 factory；provider 依 A-plan 也改 factory
- [x] 5.5 於 `api/__init__.py` 的 `healthz` dependency map 加 `openai_embedding` key（啟動時做一次 smoke `embed(["ping"])`，狀態暫存於 `app.state.openai_embedding_probe`；raw provider 不經 TrackedProvider）
- [x] 5.6 更新既有 `sidecar/tests/api/test_kb_build.py` 的 `app_with_kb_deps` fixture：把 `InMemoryQdrantBackend` / `SpyProvider` / `UsageTracker` instance 包進 `lambda _ws: tracker` factory（backward compat 橋接）
- [x] 5.7 執行 `uv run pytest sidecar/tests/test_wire_kb_dependencies.py sidecar/tests/api/test_kb_build.py` 確認 4.1 ~ 4.5 全綠 + sse-progress-skeleton 原測仍綠

## 6. RED — `KB build production dependency wiring` + dim-mismatch guard 規約測試

對應 spec `knowledge-base / KB build production dependency wiring`。

- [x] 6.1 [P] 於 `sidecar/tests/kb/test_dim_mismatch.py` 加 `test_kb_build_aborts_before_embed_on_dim_mismatch`（KB build production dependency wiring 依決策 4：dim-mismatch guard 放在 KB 端不是 Backend 端，collection 存在但 dim 不同 → 沒呼叫 provider.embed 就 raise `KBDimMismatchError`）
- [x] 6.2 [P] 於 `test_dim_mismatch.py` 加 `test_dim_mismatch_error_event_contains_expected_and_actual`（KB build production dependency wiring 的 SSE error body 欄位）
- [x] 6.3 [P] 於 `sidecar/tests/api/test_kb_build_production.py` 加 `test_missing_openai_key_returns_503_kb_not_configured`（KB build production dependency wiring spec Scenario 2）
- [x] 6.4 [P] 於 `test_kb_build_production.py` 加 `test_happy_path_kbstats_nonzero_counters`（KB build production dependency wiring spec Scenario 1；用 in-memory backend + fake embedding provider 產 dim 1536 隨機向量）
- [x] 6.5 [P] 於 `test_kb_build_production.py` 加 `test_usage_tracker_writes_to_workspace_scoped_path`（KB build production dependency wiring spec Scenario 5；驗 `{ws}/token_usage.jsonl` 有 `operation: "embed"` / `module: "kb_build"` 行）
- [x] 6.6 [P] 於 `test_kb_build_production.py` 加 `test_openai_rate_limited_surfaces_as_sse_error_event`（KB build production dependency wiring spec Scenario 4）

## 7. GREEN — 實作 `KB build production dependency wiring` + dim-mismatch guard

對應 spec `knowledge-base / KB build production dependency wiring`。

- [x] 7.1 於 `kb/knowledge_base.py` 依決策 4：dim-mismatch guard 放在 KB 端不是 Backend 端 新增 `KBDimMismatchError` + `build()` 開頭呼叫 `backend.ensure_collection(name, expected_dim)` 做 dim 比對（在 chunking 結束、embed 開始前擋下）
- [x] 7.2 於 `kb/backend.py::QdrantHttpBackend` 加 `ensure_collection(name, expected_dim)`：delegate `_qc.ensure_collection`；collection 不存在 → 建立；存在但 dim 不符 → 捕 `QdrantCollectionSchemaError` 轉 `KBDimMismatchError(expected, actual)`；`InMemoryQdrantBackend`(test double) 同步實作
- [x] 7.3 於 `api/tasks.py::_classify_exception` 與 `ERROR_CODES` 加入 `KB_DIM_MISMATCH` 對應（KB build production dependency wiring 的 409 對應）
- [x] 7.4 於 `api/tasks.py::_run_background_task` 的 error event 加上 `expected_dim` / `actual_dim` / `suggestion` 欄位（針對 `KB_DIM_MISMATCH`，透過 `_enrich_error_event`）
- [x] 7.5 執行 `uv run pytest sidecar/tests/kb/test_dim_mismatch.py sidecar/tests/api/test_kb_build_production.py` 確認 6.1 ~ 6.6 全綠

## 8. 文件與 CLAUDE.md 收尾

- [x] 8.1 於 `docs/module-2-kb-builder.md §七` 補 production wiring 段（引 `KB build production dependency wiring` Requirement + 503 / 409 / 200 三條路徑 + 連動 D-032）
- [x] 8.2 於 `docs/llm-provider.md` 補 OpenAI embedding 段（引 `OpenAI embedding provider` Requirement + D-032 + 錯誤碼對照表）
- [x] 8.3 於 `docs/sidecar-api.md §一` healthz 章節補 `openai_embedding` dependency key 的三種狀態
- [x] 8.4 於 `docs/implementation-plan.md` 步驟 15 之後加 15.5「KB build production wiring 接齊 — 2026-04-22 提案（change `kb-build-production-wiring`）」
- [x] 8.5 於 `CLAUDE.md` 更新「最近一筆 in-progress」指到本 change，Repo 現況 sidecar 描述補「可用 `CODEBUS_OPENAI_API_KEY` env 啟用真實 KB build」

## 9. 驗證與 commit gate

- [x] 9.1 執行 `uv run pytest sidecar/tests/providers/` 確認 provider 層全綠（77 passed）
- [x] 9.2 執行 `uv run pytest sidecar/tests/api/` 確認 API 層無 regression（30 passed）
- [x] 9.3 執行 `uv run pytest` 完整 suite 無 regression（495 passed, 17 skipped）
- [x] 9.4 執行 `pre-commit run --all-files` 全綠
- [x] 9.5 手動煙霧測（需 `CODEBUS_OPENAI_API_KEY` + Qdrant 起來）：(a) `/healthz` `openai_embedding.status == "ok"` ✅；(b) `POST /scan?stream=true` 拿 scan_result ✅；(c) `POST /kb/build` → SSE 看 progress + done → `GET /tasks/{id}/result` 拿 `KBStats`（`chunks_emitted=3` / `points_upserted=3`）✅；(d) 確認 `<ws>/token_usage.jsonl` 有 `operation: "embed"` / `module: "kb_build"` 行 ✅。**發現**：TrackedProvider 自動記 + KnowledgeBase 手動記 → 同一 embed call 被記兩次（cost 帳會 2x），留給下一條 change 處理（task `usage-tracker-dedup` 之類）。
- [x] 9.6 手動煙霧測 2（graceful degrade）：清 `CODEBUS_OPENAI_API_KEY` 後重啟 sidecar → `POST /kb/build` 回 503 `KB_NOT_CONFIGURED` + `missing: [kb_provider, kb_usage_tracker, kb_embedding_dim]` ✅ + `/healthz` `openai_embedding.status == "not-configured"` ✅
