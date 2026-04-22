## Why

Module 1 Scanner 與 Module 2 KB Builder 的 progress 訊號目前只活在 in-process callback——`POST /scan` 是同步整批回傳、`POST /kb/build` 端點根本還不存在；前端 / Tauri 殼無法在這兩個長任務跑的時候顯示進度，也無法在失敗時即時收到錯誤事件。`docs/sidecar-api.md §四` 已經把 SSE event schema 與 `GET /tasks/{id}/events` 端點規格完整定下來（D-001 混合架構 + D-022 llm_calls 稽核鏈基於同一 SSE 通道），但 sidecar 還沒長出對應的 task store / event stream 基礎設施。本 change 對齊 `docs/implementation-plan.md` 步驟 15，把 Module 1/2 的進度事件接上 SSE，給後續 Agent 階段（步驟 22 emit `agent_thought` / `judge_verdict` / `action_result`）打底。

關聯 ADR：D-001（混合架構：Tauri 殼 ↔ Sidecar IPC）、D-002（雙模 discriminator day 1，task lifecycle 不分模式）、D-022（SSE 是 LLM Call Inspector live view 的同一通道）。spec §七「single FIFO queue」既定，故 task store 走 single-slot 而非 dict-based pool。

## What Changes

- 新增 SSE 端點 `GET /tasks/{id}/events`（`text/event-stream`），輸出三種 event type：`progress` / `done` / `error`；其餘 event type（`agent_thought` / `judge_verdict` / `rag_hits` / `kb_growth` / `answer_stream` / `usage_delta` / `usage_summary` / `llm_call`）**不在本 change 範圍**，由後續 Agent / Q&A change 在同一 endpoint 上擴增。
- 新增 in-memory **single-slot task store**：包含 `task_id` 產生器（`{kind}_{rand}` 格式，如 `scan_abc123` / `kb_xyz789`）、status（`running` / `done` / `error`）、`asyncio.Queue` event channel、終局結果 payload（done 後可從 `GET /tasks/{id}/result` 拿）。同時只允許一個 task 在跑；新請求若有其他 task 在跑回 `409 Conflict`。
- Module 1 Scanner：新增 `ProgressCallback` Protocol + `scan(on_progress=...)` 可選參數；Scanner walk 與 sanitizer Pass 1 階段各 emit 至少一筆 `progress`（phase=`scanning`）。
- Module 1 `POST /scan` 端點新增 `?stream=true` opt-in：帶此 query 時切到 async path，立即回 `{"task_id": "scan_..."}` 並啟背景 task；未帶時維持 M1 同步行為（**不破壞既有契約**）。
- Module 2 新增 `POST /kb/build` async endpoint：body `{workspace_root, scan_result}`，回 `{"task_id": "kb_..."}`；背景 task 內 invoke `KnowledgeBase.build()`、把現有 `KBProgressEvent`（chunking / embedding / upserting / done 四 phase）翻譯成 spec §四 wire schema（統一 `phase=embedding`、`current/total/current_file` 由 KB 階段對應）。
- 新增 `GET /tasks/{id}/result` 端點：task `status=done` 時回終局 payload（Scanner 回 `ScanResult`、KB 回 `KBStats`）；非 done 回 `409`。
- bearer + loopback 中介層套用至所有新端點；error event 不揭露 stack trace（只回 `code` + 安全的 `message`）。

## Non-Goals

- **多 task 並行 / task pool**：spec §七 規定 single FIFO，本 change 維持 single-slot；多 task 排程留給未來 change。
- **task 持久化 / restart restore**：sidecar 重啟後既有 task 視為遺失，前端需重發請求。
- **SSE reconnect with `Last-Event-ID`**：本 change 走一次性連線；連線斷掉就要重發請求並訂新 task。
- **task 取消端點（`DELETE /tasks/{id}`）**：MVP 先不做；客戶端關閉 SSE 連線後背景 task 仍會跑完。
- **Agent / Q&A SSE event type**：`agent_thought` / `judge_verdict` / `rag_hits` / `kb_growth` / `answer_stream` / `usage_delta` / `usage_summary` / `llm_call` 屬步驟 22+ 範圍，本 change 只埋 `progress` / `done` / `error` 三種。
- **前端 UI 進度條**：屬 Module 7，本 change 只到 sidecar SSE wire；Tauri / Nuxt 端訂閱在 Module 7 階段做。
- **POST /scan 強制改 async**：保留同步 fallback（無 `?stream=true` 時走原路），不破壞 `scanner-skeleton` 既有契約。
- **`POST /kb/build` production dep wiring**：本 change 只負責 SSE 端點 + `app.state.kb_*` hook（缺值時回 503 `KB_NOT_CONFIGURED`），實際把 embedding provider / Qdrant backend / UsageTracker / embedding_dim 注入 `app.state` 的 wiring（含 provider 選型、API key 解析、collection 既存策略）留給下一條 change `kb-build-production-wiring`。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `sidecar-runtime`：新增 single-slot task store + `GET /tasks/{id}/events` SSE 端點 + `GET /tasks/{id}/result` 端點 + 409 衝突語意；既有 bearer + loopback + ephemeral port 規約不變。
- `folder-scanner`：`POST /scan` 新增 `?stream=true` opt-in async 模式；同步路徑契約保留；服務層加 `ProgressCallback` Protocol。
- `knowledge-base`：新增 `POST /kb/build` async endpoint + KB phase 名稱到 spec §四 wire schema 的翻譯規則；既有 `KnowledgeBase.build(on_progress=...)` 介面不變。

## Impact

- **Affected specs**：`openspec/specs/sidecar-runtime/spec.md`、`openspec/specs/folder-scanner/spec.md`、`openspec/specs/knowledge-base/spec.md`（皆為 ADDED Requirements，無 MODIFIED / REMOVED）。
- **Affected code**：
  - `sidecar/src/codebus_agent/api/__init__.py`（router 註冊新端點）
  - `sidecar/src/codebus_agent/api/tasks.py`（新檔：task store + SSE 端點 + result 端點）
  - `sidecar/src/codebus_agent/api/scan.py`（新增 `?stream=true` 分支）
  - `sidecar/src/codebus_agent/api/kb.py`（新檔：`POST /kb/build`）
  - `sidecar/src/codebus_agent/scanner/service.py`（加 `on_progress` 參數）
  - `sidecar/src/codebus_agent/scanner/models.py`（加 `ProgressCallback` Protocol + `ScannerProgressEvent` 模型）
  - `sidecar/src/codebus_agent/kb/knowledge_base.py`（KB phase 翻譯 helper，可選擇放到 api/kb.py adapter 層）
  - `sidecar/tests/api/test_tasks_sse.py`（新檔：SSE stream 解析測試）
  - `sidecar/tests/api/test_scan_stream.py`（新檔：`?stream=true` 行為測試）
  - `sidecar/tests/api/test_kb_build.py`（新檔：`POST /kb/build` async 行為測試）
- **Affected docs**：`docs/sidecar-api.md`（補 task lifecycle / 409 語意；spec §四 既有 event schema 不動）、`docs/implementation-plan.md` 步驟 15 標完成日、`CLAUDE.md` Repo 現況補一句、`docs/module-1-scanner.md` 補 progress callback 介面、`docs/module-2-kb-builder.md` 補 `POST /kb/build` 對接。
- **Affected dependencies**：`sse-starlette`（FastAPI SSE 標準套件）需加進 `sidecar/pyproject.toml`；無其他新依賴。
- **No breaking change**：既有 `POST /scan`（同步）契約保留；`KnowledgeBase.build()` Python API 不變。
