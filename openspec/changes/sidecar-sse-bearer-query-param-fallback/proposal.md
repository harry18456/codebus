## Why

引用 D-001（Tauri + Python sidecar 混合架構）/ D-009（本地優先）/ D-011（資安與合規）— bearer + loopback 是 sidecar 對外通訊的兩條防線（CLAUDE.md 不變式 #5），本 change **不鬆綁**任何一條，只把 sidecar middleware 對齊 frontend 早已 ship 的 transport。

m1-power-on archive 後 sidecar `BearerAuthMiddleware`（auth subpackage 入口模組）只接受 `Authorization: Bearer <token>` header；但 frontend `useSseTask`（SSE 訂閱 composable）因 browser-native `EventSource` API **無法設 custom header**（`frontend-shell` Requirement「useSseTask consumes bearer through useSidecar」明文要求 EventSource），改把 bearer 放 `?bearer=<token>` query param。兩條 archived spec SHALL clause 從一開始就互不相容，但 sidecar 測試走 FastAPI TestClient（可設 header），從 m1-power-on 起這條真實 browser 經 EventSource 觸發的路徑從未被驗證。

`entry-workspace-onramp` change（目前 parked，依賴本 change ship 才能解阻塞）是第一條從 entry page 真實觸發 `/scan?stream=true` → SSE 的 user flow，發現所有 SSE 從 WebView 出去都會 401。

## What Changes

- **Modified `BearerAuthMiddleware`**：新增 query-param fallback，但**只在 path 符合 `^/tasks/[^/]+/events$`** 時生效。其他 endpoint（POST `/scan` / `/explore` / `/qa` / `/generate` / `/kb/build`、`/auth/*`、`/settings/*`、`/healthz`）仍只接受 `Authorization: Bearer` header — 這些 endpoint 全部從 `useSidecar.fetch()` 呼叫，可以正常設 header。
- **路徑限定（防禦深度）**：query-param bearer 只在唯一一條 EventSource 必經的路徑接受；其他 endpoint 收到 `?bearer=` 完全忽略，照樣 401。確保 bearer 不會誤入 POST 的 access log / Referer / browser history。
- **uvicorn `access_log=False` 補註**：sidecar 入口模組已存在 `access_log=False` 設定（避免 query-string bearer 落 access log）；補一行 inline comment 鎖住為什麼這個值不能改 true，配合本 change 的 query-param transport 一起鎖 invariant。
- **Spec amendment（`sidecar-runtime`）**：Requirement「Bearer token authentication」加 1 個 ADDED Scenario「SSE events endpoint accepts bearer via query parameter」+ 原 Requirement 主敘述補 path-scoped 例外條款；其他 endpoint 的 SHALL 不動。
- **5 個 sidecar test**：path-scoped accept、path-scoped reject（POST `/scan` 帶 `?bearer=` 仍 401）、query-param token 正確 / 錯誤、純 header 既有路徑不破。

## Non-Goals

對應前一輪 discuss session 的結論，明確 out-of-scope：

- **Tauri WebView CSP enforcement**（discuss #3）：Tauri 設定當前 `"csp": null`。任何透過 `@nuxtjs/mdc` 渲染的 markdown / Q&A answer 若繞過 sanitization，可 inject script 經 loopback 偷 in-memory bearer。修需定義 CSP allowlist + regression test 全 mdc / Quiz / station / qa 渲染 path 的相容性。獨立 change `tauri-csp-baseline` 處理。
- **Tauri IPC trust boundary audit**（discuss #5）：`#[tauri::command]` handlers（tutorial / audit_files / keyring 三組）走 Tauri 自己的 capability JSON + `validate_path`，不過 bearer middleware；威脅模型整體稽核需獨立 ADR。獨立 change `ipc-trust-boundary-audit` 處理。
- **Bearer rotation mid-session**：當前模型 = 一個 bearer per sidecar process lifecycle。Rotation 要 re-handshake protocol，等實際需求出現再做。
- **改用 fetch-based SSE polyfill（如 `@microsoft/fetch-event-source`）取代 browser EventSource**：fetch-based 可設 header，徹底消滅 query-param 路徑。考慮過但拒絕 — 為解一個 path-scoped query-param 已能解的問題，引入新依賴不划算；且 `frontend-shell` Requirement 明文要求 browser-native EventSource。
- **不動 `frontend-shell` spec / `useSseTask.ts` 程式碼**：frontend 行為 0 改變，本 change 純 sidecar-side 修正以對齊既有 frontend。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `sidecar-runtime`: Requirement「Bearer token authentication」— 新增 Scenario「SSE events endpoint accepts bearer via query parameter」+ 原 Requirement 主敘述加上 path-scoped fallback 例外條款。其他 Scenario（Missing bearer rejected / Wrong bearer rejected / Correct bearer accepted）不動。

## Impact

- Affected specs:
  - openspec/specs/sidecar-runtime/spec.md（modified — Requirement: Bearer token authentication，新增 1 Scenario）
- Affected code:
  - Modified:
    - sidecar/src/codebus_agent/auth/__init__.py（BearerAuthMiddleware 加 path-scoped query-param fallback；約 6-8 行 diff）
    - sidecar/src/codebus_agent/api/main.py（access_log=False 旁加 inline comment 鎖住為什麼）
  - New:
    - sidecar/tests/auth/test_bearer_query_param.py（5 cases：SSE path accept / non-SSE path reject / query token correct / query token wrong / header path 不破）
