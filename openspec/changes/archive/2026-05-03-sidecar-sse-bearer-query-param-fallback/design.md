## Context

m1-power-on archive 後 sidecar `BearerAuthMiddleware` 只接受 `Authorization: Bearer <token>` header，frontend `useSseTask` 因 browser-native `EventSource` API 限制走 `?bearer=<token>` query param。兩條 archived spec SHALL clause 互不相容；sidecar 端用 FastAPI TestClient 測試（可設 header），實機 browser 觸發 SSE 的路徑從未驗證；`entry-workspace-onramp` 是第一條真實觸發此路徑的 user flow，發現 401。

`bearer + loopback` 是 sidecar 對外通訊的唯二防線（CLAUDE.md 不變式 #5）。本 change 不弱化任一條，只把 sidecar middleware 對齊 frontend 已 ship 的 transport，並用 path-scoping 縮小 query-param fallback 的攻擊面。

## Goals / Non-Goals

**Goals:**

- 解 `entry-workspace-onramp` 在實機 cargo tauri dev 觸發 SSE 必 401 的阻塞
- 對齊 `frontend-shell` Requirement「useSseTask consumes bearer through useSidecar」既有的 EventSource transport
- query-param fallback 嚴格限定在 SSE 必經路徑 `^/tasks/[^/]+/events$`，其他 endpoint 仍只能 header
- bearer 不落 access log（uvicorn 既有 `access_log=False` 須鎖死）

**Non-Goals:**

- 動 frontend `useSseTask` 行為（已正確 ship）
- 動 `frontend-shell` spec
- 鬆綁 bearer / loopback 任一條 invariant
- 加 CSP / 稽核 IPC trust boundary（兩條都列在 proposal Non-Goals 為 follow-up change）

## Decisions

### Decision 1: query-param fallback 限定在 path `^/tasks/[^/]+/events$`

`BearerAuthMiddleware.dispatch()` 加一段 fallback：當 Authorization header 缺席且 `request.url.path` 符合 `^/tasks/[^/]+/events$`，才從 `request.query_params.get("bearer")` 拿 token；其他 path 照舊 401。

**Why path-scoped 不全域：** query-param 形式的 bearer 比 header 多三個漏點 — browser history、Referer header、access log 預設行為。把 fallback 限縮到唯一一條 `EventSource` 必經的 path，其他 endpoint（POST 都從 `useSidecar.fetch()` 走，能設 header）就吃不到這個 fallback；未來不小心擴大也會被 path regex 擋下。

**Why 用 regex 而不是 startswith：** path 結構有兩個變動位（task_id），但開頭 `/tasks/` 與結尾 `/events` 都固定。用 regex `^/tasks/[^/]+/events$` 比 `startswith("/tasks/") and endswith("/events")` 精準（後者會誤接受 `/tasks/foo/events/leak`）。`task_id` 進一步格式 validation 由 endpoint handler 自己做（既有邏輯不變）。

### Decision 2: `access_log=False` 必須鎖死，加 inline comment 解釋為什麼不能改 true

uvicorn 的 access log 預設會 log query string（`GET /tasks/scan_xxx/events?bearer=xxx HTTP/1.1`），等於 bearer 直接落到 stdout / log 檔。`sidecar/src/codebus_agent/api/main.py` 既有 `access_log=False`，理由本來只是「降噪」。本 change 把這個值升格為**安全 invariant**：加一行 inline comment 解釋「不可改 true，否則 SSE bearer 會 leak 到 log」。配合 query-param 路徑一起鎖。

**Why 不改用 access log filter：** 寫一個 filter scrub query string 是更精細的方案，但加複雜度（uvicorn log filter 整合 / 測試），且對純 debug 場景沒有真正需求 — 既然 access log 全關不影響運作，就不要為了「未來可能要 access log」開洞。

### Decision 3: 用 `secrets.compare_digest` 比對 query-param token，與 header path 共用同一比對邏輯

兩條 path 共用同一個 `_validate_token(presented: str)` helper（內部走 `secrets.compare_digest`），避免 timing attack 防禦只做了 header 那條而漏掉 query-param 那條。

### Decision 4: SSE 路徑外即使 `?bearer=` 也照樣 401，不為了「使用者方便」開洞

例如 POST `/scan?bearer=...` 即使 token 對也回 401（因為沒有 Authorization header）。**Why:** 防止使用者 / 開發者在 cURL 或 dev tool 養成 query-param 習慣，以後不小心擴大用法。Path-scoping 是「default deny + narrow allow」，不是「default allow + narrow deny」。

## Risks / Trade-offs

- **[Risk]** query-param bearer 仍會出現在 Tauri WebView 內部的 URL 表面（DevTools Network 面板、performance API resource timing） → **Mitigation:** Tauri WebView 是 user-owned isolated runtime，沒有 telemetry SDK 把 URL 上報外部。`csp: null` 的衍生風險獨立 change `tauri-csp-baseline` 處理。
- **[Risk]** path regex 寫錯（例如把 `/tasks/foo/events` 多匹了 `/tasks/foo/events/leak`）導致 fallback 接受了不該接受的 path → **Mitigation:** 5 個 test 中包含 path-scoped reject，**正面測** 帶 `?bearer=` 的 POST `/scan` 仍 401，覆蓋 regex 邊界。
- **[Trade-off]** 不引入 `@microsoft/fetch-event-source` 替換 browser EventSource。Accept: 引入新依賴只為消滅 query-param 一條路徑成本太高；frontend-shell spec 也明文要求 browser-native EventSource。
- **[Risk]** access_log invariant 只靠 inline comment 沒有自動測試 → **Mitigation:** 加一條 unit test：build app instance，inspect uvicorn config 預設 `access_log` 是否為 False（防止有人改 main.py 後忘了發現）。
