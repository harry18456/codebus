## 1. Scaffolding（對應 `implementation-plan.md` 步驟 15 起手）

- [x] 1.1 建立 `sidecar/src/codebus_agent/api/tasks.py` 空模組（持 `TaskRegistry` / `TaskHandle` / SSE & result 端點 placeholder）
- [x] 1.2 建立 `sidecar/src/codebus_agent/api/kb.py` 空模組（持 `POST /kb/build` placeholder）
- [x] 1.3 於 `sidecar/src/codebus_agent/scanner/models.py` 加 `ScannerProgressEvent` Pydantic model 與 `ScannerProgressCallback` Protocol 佔位（暫不用於 service.scan）
- [x] 1.4 於 `sidecar/pyproject.toml` 新增 `sse-starlette` 依賴並 `uv sync`
- [x] 1.5 於 `sidecar/src/codebus_agent/api/__init__.py` 預掛 tasks_router / kb_router（路由以 placeholder 端點先佔，留待 GREEN 補實作）

## 2. RED — `Single-slot in-memory task registry` 規約測試

對應 design `Single-slot task store over dict-based pool`（registry 持 `Optional[TaskHandle]` 而非 dict-based pool）。

- [x] 2.1 [P] 於 `sidecar/tests/api/test_task_registry.py` 加 `test_registry_is_single_slot_and_overwrites_on_new_task`（done 後新 task 進來覆蓋；single-slot 而非 dict-based pool）
- [x] 2.2 [P] 於 `test_task_registry.py` 加 `test_running_task_blocks_new_task_creation`（直接呼 registry API 擋 second running，回 409 / TASK_IN_FLIGHT）
- [x] 2.3 [P] 於 `test_task_registry.py` 加 `test_terminal_handle_survives_until_overwritten`（done 後 result 仍可拿）
- [x] 2.4 [P] 於 `test_task_registry.py` 加 `test_task_id_format_matches_regex`（對應 `task_id format` 與 design `task_id 用前綴 + 8 字 hex random`：`scan_<hex8>` / `kb_<hex8>`）

## 3. GREEN — 實作 `Single-slot in-memory task registry` + `task_id format`

對應 design `Single-slot task store over dict-based pool` 與 `task_id 用前綴 + 8 字 hex random`。

- [x] 3.1 於 `api/tasks.py` 為 `Single-slot in-memory task registry` 實作 `TaskHandle`（`id`/`kind`/`status`/`subscribers: list[asyncio.Queue]`/`result`/`error_event`）
- [x] 3.2 於 `api/tasks.py` 實作 `TaskRegistry`（`Single-slot in-memory task registry` 主體：單一 `Optional[TaskHandle]`、不用 dict-based pool；`create(kind) -> TaskHandle | None`、`get(id) -> TaskHandle | None`、`current_running() -> TaskHandle | None`）
- [x] 3.3 於 `api/tasks.py` 實作 `_generate_task_id(kind) -> str` 滿足 `task_id format`（`secrets.token_hex(4)` 產 8-hex random，符合 regex `^(scan|kb)_[0-9a-f]{8}$`）
- [x] 3.4 於 `create_app` 把 `TaskRegistry` 掛上 `app.state.tasks`（`Single-slot in-memory task registry` 生命週期跟著 app）
- [x] 3.5 執行 `uv run pytest sidecar/tests/api/test_task_registry.py` 確認 2.1 ~ 2.4 全綠

## 4. RED — `SSE event stream endpoint` 規約測試

對應 design `asyncio.Queue` 作 event channel；每位訂閱者自帶 `asyncio.Queue` 副本（fan-out subscriber queue 模式）。

- [x] 4.1 [P] 於 `sidecar/tests/api/test_tasks_sse.py` 加 `test_sse_emits_progress_done_in_order`（fixture 直接 emit 三筆，subscriber queue 收到順序對）
- [x] 4.2 [P] 於 `test_tasks_sse.py` 加 `test_sse_rejects_without_bearer_token`（401，無 event-stream body）
- [x] 4.3 [P] 於 `test_tasks_sse.py` 加 `test_sse_multiple_subscribers_receive_identical_sequences`（每位訂閱者自帶 asyncio.Queue 副本，互不影響）
- [x] 4.4 [P] 於 `test_tasks_sse.py` 加 `test_sse_subscriber_disconnect_does_not_affect_others`
- [x] 4.5 [P] 於 `test_tasks_sse.py` 加 `test_sse_emits_only_progress_done_error_types_in_this_change`（其他 type 不應出現）

## 5. GREEN — 實作 `SSE event stream endpoint`

對應 design `asyncio.Queue` 作 event channel；每位訂閱者自帶 `asyncio.Queue` 副本。

- [x] 5.1 於 `api/tasks.py` 實作 `SSE event stream endpoint` `GET /tasks/{id}/events`，用 `sse_starlette.EventSourceResponse`，loop pop subscriber queue 並 yield JSON line
- [x] 5.2 於 `api/tasks.py` 實作 `TaskHandle.subscribe() -> asyncio.Queue` 與 `TaskHandle.unsubscribe(q)` helper（`SSE event stream endpoint` 連線 lifecycle 用，每位訂閱者自帶 asyncio.Queue 副本）
- [x] 5.3 於 `api/tasks.py` 實作 `TaskHandle.emit(event: dict)` fan-out 到所有 subscriber queue（`SSE event stream endpoint` 廣播路徑）
- [x] 5.4 確認 bearer middleware 套用至 `tasks_router`（`SSE event stream endpoint` 與既有 `/scan` 同層 auth）
- [x] 5.5 執行 `uv run pytest sidecar/tests/api/test_tasks_sse.py` 確認 4.1 ~ 4.5 全綠

## 6. RED — `Task result lookup endpoint` 規約測試

- [x] 6.1 [P] 於 `sidecar/tests/api/test_task_result.py` 加 `test_result_returns_200_when_done`
- [x] 6.2 [P] 於 `test_task_result.py` 加 `test_result_returns_409_when_running`（body 含 `code: TASK_NOT_DONE`）
- [x] 6.3 [P] 於 `test_task_result.py` 加 `test_result_returns_404_when_unknown`
- [x] 6.4 [P] 於 `test_task_result.py` 加 `test_result_requires_bearer`

## 7. GREEN — 實作 `Task result lookup endpoint`

- [x] 7.1 於 `api/tasks.py` 實作 `Task result lookup endpoint`：look up handle、依 status 回 200/409/404，bearer middleware 必經
- [x] 7.2 執行 `uv run pytest sidecar/tests/api/test_task_result.py` 確認 6.1 ~ 6.4 全綠（驗證 `Task result lookup endpoint` 三種狀態回應）

## 8. RED — `Background task error containment` 規約測試

對應 design `error event 安全性`（safe code/message，不洩漏 traceback）與 `背景 task 用 asyncio.create_task + app.state 引用`（task 例外必收斂為 SSE error event）。

- [x] 8.1 [P] 於 `sidecar/tests/api/test_task_error_containment.py` 加 `test_background_exception_emits_safe_error_event`（mock 背景 coroutine 拋例外，斷言 emit code/message 不含 repr/traceback）
- [x] 8.2 [P] 於 `test_task_error_containment.py` 加 `test_subscriber_after_error_still_receives_terminal_event`
- [x] 8.3 [P] 於 `test_task_error_containment.py` 加 `test_full_traceback_written_to_logger_only`（caplog 斷言）

## 9. GREEN — 實作 `Background task error containment` wrapper

對應 design 「背景 task 用 `asyncio.create_task` + `app.state` 引用」 與 `error event 安全性`。

- [x] 9.1 於 `api/tasks.py` 實作 `_run_background_task(handle, coro_factory)` 達成 `Background task error containment`：用 `asyncio.create_task` spawn、`try` 跑 coro_factory、`except` emit safe error event + log full、`finally` close subscribers
- [x] 9.2 於 `api/tasks.py` 定義錯誤代碼表常數 `ERROR_CODES = {"SCAN_FAILED", "KB_EMBED_FAILED", "INTERNAL_ERROR"}` 與 `_classify_exception(exc) -> str`（`Background task error containment` 子組件：依 exception 類型挑 code，不洩漏 repr）
- [x] 9.3 確保 done event 只在 coro 正常結束才 emit，error event 只在 except 路徑 emit（`Background task error containment` 互斥不變式）
- [x] 9.4 執行 `uv run pytest sidecar/tests/api/test_task_error_containment.py` 確認 8.1 ~ 8.3 全綠

## 10. RED — `Scanner progress callback hook` 規約測試

- [x] 10.1 [P] 於 `sidecar/tests/scanner/test_progress_callback.py` 加 `test_scan_without_callback_preserves_sync_contract`（與既有 `test_service.py` 行為等價）
- [x] 10.2 [P] 於 `test_progress_callback.py` 加 `test_scan_emits_at_least_one_walking_and_sanitizing_event`
- [x] 10.3 [P] 於 `test_progress_callback.py` 加 `test_callback_exception_propagates_and_does_not_return_partial_result`
- [x] 10.4 [P] 於 `test_progress_callback.py` 加 `test_progress_event_invariants`（current >= 0、current <= total when total not None）

## 11. GREEN — 實作 `Scanner progress callback hook`

- [x] 11.1 於 `scanner/models.py` 為 `Scanner progress callback hook` 落實 `ScannerProgressEvent` Pydantic model（`phase: Literal["walking", "sanitizing"]`、`current/total/current_file` 依 spec）與 `ScannerProgressCallback` Protocol
- [x] 11.2 於 `scanner/service.py` 把 `scan(...)` 簽名擴成 `async def scan(..., on_progress: ScannerProgressCallback | None = None)`（`Scanner progress callback hook` 入口；無 callback 時保持 sync 行為等價）
- [x] 11.3 於 `scanner/service.py` walk 階段每 50 檔 await 一次 callback；sanitize 階段同樣每 50 檔 emit 一次（`Scanner progress callback hook` 兩段觸發點）
- [x] 11.4 確認 `tests/scanner/test_service.py` 既有測仍綠（`Scanner progress callback hook` 無 callback path 行為等價）
- [x] 11.5 執行 `uv run pytest sidecar/tests/scanner/test_progress_callback.py` 確認 10.1 ~ 10.4 全綠

## 12. RED — `POST /scan opt-in async streaming mode` 規約測試

對應 design `POST /scan?stream=true` opt-in，不預設改 async（既有同步契約保留，新功能僅在 query 帶 `stream=true` 時生效）與 `Module 1 / 2 phase 名稱對應`（scanner walking/sanitizing → 統一翻成 `scanning`）。

- [x] 12.1 [P] 於 `sidecar/tests/api/test_scan_stream.py` 加 `test_scan_without_stream_query_returns_sync_result`（既有契約保留）
- [x] 12.2 [P] 於 `test_scan_stream.py` 加 `test_scan_with_stream_true_returns_task_id_immediately`（latency 上限斷言）
- [x] 12.3 [P] 於 `test_scan_stream.py` 加 `test_scan_stream_phase_collapsed_to_scanning`（subscriber 看到 `phase: "scanning"`，符合 Module 1 phase 名稱對應）
- [x] 12.4 [P] 於 `test_scan_stream.py` 加 `test_scan_stream_done_then_result_returns_full_scan_result`

## 13. GREEN — 實作 `POST /scan opt-in async streaming mode`

對應 design `POST /scan?stream=true` opt-in，不預設改 async 與 `Module 1 / 2 phase 名稱對應`。

- [x] 13.1 於 `api/scan.py` 為 `POST /scan opt-in async streaming mode` 加 `?stream=true` 分支：建 task handle、啟 `_run_background_task`、立即回 `{task_id}`
- [x] 13.2 於 `api/scan.py` 寫 `_scanner_event_to_wire(ScannerProgressEvent) -> dict`（`POST /scan opt-in async streaming mode` 翻譯層）：依 Module 1 phase 名稱對應把 `walking`/`sanitizing` 都翻成 `phase: "scanning"`、保留 `current/total/current_file`
- [x] 13.3 確保非 stream 路徑（無 query）程式碼路徑完全不動（`POST /scan opt-in async streaming mode` 是 opt-in，既有 sync 端點測仍綠）
- [x] 13.4 執行 `uv run pytest sidecar/tests/api/test_scan_stream.py` 確認 12.1 ~ 12.4 全綠

## 14. RED — `POST /kb/build async endpoint` + `KB progress phase translation to wire schema` 規約測試

對應 design `POST /kb/build` 預設 async（無同步路徑） 與 `Module 1 / 2 phase 名稱對應`（KB chunking/embedding/upserting 全翻成 `embedding`）。

- [x] 14.1 [P] 於 `sidecar/tests/api/test_kb_build.py` 加 `test_kb_build_returns_task_id_immediately`（POST /kb/build 預設 async，無同步路徑）
- [x] 14.2 [P] 於 `test_kb_build.py` 加 `test_kb_build_rejects_concurrent_request_with_409`
- [x] 14.3 [P] 於 `test_kb_build.py` 加 `test_kb_build_done_then_result_returns_kbstats`
- [x] 14.4 [P] 於 `test_kb_build.py` 加 `test_kb_phase_collapsed_to_embedding_in_wire_events`（Module 2 phase 名稱對應）
- [x] 14.5 [P] 於 `test_kb_build.py` 加 `test_kb_wire_progress_monotonic_and_reaches_total`（current==0 至少一筆 + current==total 至少一筆 + 單調非降）
- [x] 14.6 [P] 於 `test_kb_build.py` 加 `test_kb_source_done_phase_does_not_emit_wire_progress`（done 不變 progress event）

## 15. GREEN — 實作 `POST /kb/build async endpoint` + `KB progress phase translation to wire schema`

對應 design `POST /kb/build` 預設 async（無同步路徑） 與 `Module 1 / 2 phase 名稱對應`。

- [x] 15.1 於 `api/kb.py` 實作 `POST /kb/build async endpoint`（預設 async、無同步路徑）：parse body `{workspace_root, scan_result}`、建 KB task handle、啟 `_run_background_task`、立即回 `{task_id}`
- [x] 15.2 於 `api/kb.py` 寫 `_kb_event_to_wire(KBProgressEvent) -> dict | None` 達成 `KB progress phase translation to wire schema`：`done` 回 `None`（不 emit progress）、其他三 phase 翻成 `phase: "embedding"` + 保留 current/total
- [x] 15.3 於 `api/kb.py` 寫 `_make_kb_progress_adapter(handle) -> ProgressCallback`（`KB progress phase translation to wire schema` 接合層）：包裝 KB callback，把翻譯後 dict emit 到 handle channel
- [x] 15.4 背景 task 內 invoke `KnowledgeBase(backend=QdrantHttpBackend(...), provider=registry.get(ProviderRole.embedding), ...)`（`POST /kb/build async endpoint` 主流程），build 完寫 `KBStats` 到 handle.result
- [x] 15.5 執行 `uv run pytest sidecar/tests/api/test_kb_build.py` 確認 14.1 ~ 14.6 全綠

## 16. 文件與 re-export 收尾

- [x] 16.1 於 `docs/sidecar-api.md` 補 task lifecycle 段（registry / 409 / result endpoint），spec §四 既有 event schema 不動
- [x] 16.2 於 `docs/module-1-scanner.md` 補 `ScannerProgressCallback` 介面與 `?stream=true` 入口說明
- [x] 16.3 於 `docs/module-2-kb-builder.md` 補 `POST /kb/build` 對接與 phase 翻譯規則
- [x] 16.4 於 `docs/implementation-plan.md` 步驟 15 該行尾加「— 2026-04-22 落地（change `sse-progress-skeleton`）」
- [x] 16.5 於 `CLAUDE.md` 「Repo 現況」段落補一句：sidecar SSE skeleton（`POST /kb/build` async + `?stream=true` opt-in + `GET /tasks/{id}/events|result`）已落地

## 17. 驗證與 commit gate

- [x] 17.1 執行 `uv run pytest sidecar/tests/api/`，全綠
- [x] 17.2 執行 `uv run pytest sidecar/tests/scanner/`，確認 progress retrofit 沒 regress 既有 scanner 測
- [x] 17.3 執行 `uv run pytest` 完整 suite，無 regression
- [x] 17.4 執行 `pre-commit run --all-files`，全綠
- [x] 17.5 手動煙霧測（Qdrant 啟動 + bearer）：(a) `GET /healthz` 回 qdrant.status=ok；(b) `POST /scan?stream=true` 立即回 `scan_<hex8>`，訂 SSE 收到 `phase: "scanning"` progress + `done`，`GET /tasks/{scan_id}/result` 拿 `ScanResult`；(c) `POST /kb/build`（inline scan_result）回 `503 KB_NOT_CONFIGURED`（驗證 hook 接對，real KBStats e2e 屬下一條 change `kb-build-production-wiring` scope）
