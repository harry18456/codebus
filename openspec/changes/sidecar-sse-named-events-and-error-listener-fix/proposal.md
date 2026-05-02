## Why

`entry-workspace-onramp` 走 6.4 manual smoke 時揭露兩個串聯的 SSE bug，導致 sidecar 任何 task SSE stream 在 `done` 與「stream 收尾關連線」這兩個關鍵 transition 上都被前端誤判為 `error`。先前 archive 的 `sidecar-sse-bearer-query-param-fallback` 修好了 EventSource 401 認證問題，揭示了下游 wire format 與 listener 註冊問題。

實測結果（2026-05-03 worktree `entry-workspace-onramp`）：

- **Bug 1（Sidecar）**：`sidecar/src/codebus_agent/api/tasks.py:380` `yield {"data": _json_dump(event)}` 不帶 `event:` field。HTTP wire 端到端輸出（`fastapi.testclient.TestClient` + `EventSourceResponse` 實測）只有 `data: {"type":"done"}\r\n\r\n`，無 `event:` line。瀏覽器照 HTML EventSource 規格只 fire 預設 `message` event，frontend `addEventListener('done' | 'progress' | 'error' | ...)` 等具名 listener 永遠不觸發 → `useWorkspaceOnramp` 卡 `phase=scanning` 不前進。
- **Bug 2（Frontend）**：`web/app/composables/useSseTask.ts` `NAMED_EVENT_TYPES` 包含 `'error'`，並透過 `es.addEventListener('error', ...)` 註冊。但 EventSource connection-level 錯誤（包括 server 正常關 stream）也會 dispatch 名為 `error` 的 event 給該 listener → `me.data === undefined` → `parseData(undefined)` 走 catch 回傳字串 `'undefined'` → consumer 看到 `ev.type === 'error'` 命中 fallback path → `errorMsg='unknown SSE error'`。

兩個 bug 在 vitest 都沒抓到：sidecar `test_tasks_sse.py` 只 assert queue ordering 不 assert HTTP wire format；frontend test 用 mock 直接餵 `SseEvent` 物件、從沒模擬「真實 EventSource 在 connection close 時 fire native error event」這條。Phase 6 已 archive 的 explorer / qa SSE 因此**理論上也壞**，只是 manual smoke 從沒走到 SSE 端到端（被 entry-workspace-onramp gap 卡在沒 workspace 可觸發）。

對應 D-014（uv toolchain）+ D-001（混合架構，sidecar 與前端透過受控 IPC + SSE 通訊）。

## What Changes

- **Sidecar wire format 必帶 `event:` line**：`sidecar/src/codebus_agent/api/tasks.py:380` 改 `yield {"event": event.get("type", "message"), "data": _json_dump(event)}`；同步加 `sidecar/tests/api/test_tasks_sse_wire_format.py` 用 `TestClient.stream(...)` 抓 raw bytes 斷言「`progress` / `done` / `error` 三種 event 各自的 wire response 都同時包含 `event: <type>\r\n` 與 `data: ...\r\n` 兩行」。
- **Frontend `useSseTask` 區分 connection-level 與 server-emitted error**：從 `NAMED_EVENT_TYPES` 移除 `'error'`；改用一條獨立 `es.addEventListener('error', handleNamedError)`，handler 內**只在 `event instanceof MessageEvent && typeof event.data === 'string'` 時**才 push 進 events 陣列（保證是 SSE message event 而非 connection error）。Connection-level error 仍走 `es.onerror` 的 reconnect path（既有正確邏輯）。
- **Frontend defensive test**：加 `web/tests/composables/useSseTask.connection-error.spec.ts` 用 `vi.stubGlobal('EventSource', ...)` 模擬「server 正常關 stream → fire native error event」，斷言 `useSseTask().events.value` 不會多收一筆 `type='error'` 的 phantom event。
- **Sidecar 端 SSE wire format spec lock-in**：`openspec/specs/sidecar-runtime/spec.md` 「SSE event stream endpoint」Requirement 加新 Scenario「Wire format includes both event and data lines per emission」。
- **Frontend SSE listener spec lock-in**：`openspec/specs/frontend-shell/spec.md` 「useSseTask consumes bearer through useSidecar」Requirement 加新 Scenario「Named error listener ignores connection-level errors」。

## Non-Goals

- **不重構 `useSseTask` 重連 / 退避邏輯**：既有指數退避（1s → 2s → ... → 30s）行為已在 spec 鎖定且運作正常，本 change 只動 listener 註冊面與 connection-error 區分。
- **不改變 SSE event payload schema**：`{"type": "...", ...}` 的 JSON 結構維持，只在 SSE wire 上多加一行 `event: <type>`。Consumer 仍可繼續看 inner `data.type`，但「外層 `ev.type` 從此可信」。
- **不擴展 `NAMED_EVENT_TYPES` 新增其他 event**：本 change 不新增 sidecar 已 emit 但 frontend 沒監聽的 event 類型；那是 follow-up。
- **不改 `EventSourceResponse` library 行為或升級 sse-starlette 版本**：根因是我們自己 yield 的 dict 缺 `event` key，不是 library bug。
- **不動 `useExplorerStream` / `useQaSession` 的 inner-data 解析邏輯**：那兩個 composable 也讀 `ev.type`，Bug 1 修好後它們**自動受惠**；除非 inline review 發現新 regression 才另開 change 處理。
- **不補做 Phase 6 explorer / qa 的端到端 manual smoke**：那留給 entry-workspace-onramp `unpark` 後的 6.4 / 6.5 一起跑。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `sidecar-runtime`: 新增 SSE wire 必帶 `event:` line 的 Scenario（在既有「SSE event stream endpoint」Requirement 之下）。
- `frontend-shell`: 新增 `useSseTask` 區分 connection-level 與 server-emitted error 的 Scenario（在既有「useSseTask consumes bearer through useSidecar」Requirement 之下）。

## Impact

- Affected specs:
  - openspec/specs/sidecar-runtime/spec.md
  - openspec/specs/frontend-shell/spec.md
- Affected code:
  - Modified:
    - sidecar/src/codebus_agent/api/tasks.py
    - web/app/composables/useSseTask.ts
  - New:
    - sidecar/tests/api/test_tasks_sse_wire_format.py
    - web/tests/composables/useSseTask.connection-error.spec.ts
  - Removed:
    - (none)
- 解鎖工作：本 change archive 後 `spectra unpark entry-workspace-onramp`，跑 task 6.4 manual smoke + 6.5 hot-swap 驗證。
