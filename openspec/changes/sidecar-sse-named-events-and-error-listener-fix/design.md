## Context

CodeBus 的 sidecar ↔ 前端通訊有兩條：(1) 同步 HTTP（POST /scan, /kb/build, /explore, /generate, /qa 等），(2) 進度回傳走 SSE（`GET /tasks/<id>/events`）。SSE 通道由 `sse-progress-skeleton` 階段引入、`sse-starlette.EventSourceResponse` 實作，前端有單一 `useSseTask` composable 統一收 event，下游 `useExplorerStream` / `useQaSession` / `useWorkspaceOnramp` 各自解析 inner JSON 推進業務狀態機。

2026-05-03 走 `entry-workspace-onramp` 6.4 manual smoke 時，scan task SSE 連線雖通（HTTP 200）但 frontend 始終卡在 `phase=scanning` 或跳到 `phase=error` 顯示「unknown SSE error」。實測 `EventSourceResponse({"data": ...})` wire output 確認**沒帶 `event:` line**，加上 `useSseTask` 的 `'error'` listener 會吃 EventSource 的 native connection-error，兩個 bug 串聯造成上述行為。

先前 `sidecar-sse-bearer-query-param-fallback` change（archive 2026-05-03）修好了 EventSource 走 query-param bearer 的 401 問題，本 change 修剩下的 wire-format 與 listener 兩個 root cause。Phase 6 已 archive 的 explorer / qa SSE consumer 也讀外層 `ev.type`，因此**實質上一直是壞的**，只是被 manual smoke gap 遮蓋（沒 workspace 就觸發不到 explorer / qa）。

## Goals / Non-Goals

**Goals:**

- 一次修好兩個 bug，讓 scan / kb-build / explore / generate / qa 五條 SSE 通道對 `useSseTask` 都行為一致：`done` 觸發成功 callback、`error` 觸發失敗 callback、connection close 不誤報。
- Defensive test 從 wire format 與 EventSource native event 兩個層面鎖死，避免下次 regression 再被 manual smoke 才抓到。
- Spec 同步加 Scenario，讓未來新加 SSE event type 的 change 自動繼承「必帶 `event:` line」與「named error listener 必須 gate `MessageEvent` instanceof 檢查」這兩條約束。
- 不破壞既有 reconnect / backoff / events FIFO cap 行為（spec 已鎖且運作正常）。

**Non-Goals:**

- 不重寫 `useSseTask` 重連 / 退避 / events cap 邏輯。
- 不改 SSE event payload JSON schema（外層多 `event:` 行，內層 JSON 不變）。
- 不擴 `NAMED_EVENT_TYPES` 名單；本 change 只動 `'error'` 的註冊位置。
- 不升 sse-starlette 版本；根因是 caller 自己沒 yield `event` key，不是 library bug。
- 不動 `useExplorerStream` / `useQaSession` 業務邏輯；它們讀外層 `ev.type`，Bug 1 修好後自動正確。
- 不在本 change 補做 Phase 6 explorer / qa 端到端 manual smoke；那留給 entry-workspace-onramp unpark 後一併走。

## Decisions

### Decision 1: Sidecar 在 `_event_generator` yield 時補 `event` field（單點改動）

`sidecar/src/codebus_agent/api/tasks.py:380` 改成：

```python
yield {"event": event.get("type", "message"), "data": _json_dump(event)}
```

**為什麼**：所有 task SSE 流量都走 `stream_task_events` → `_event_generator`，這是唯一的 sink 點。改一行、影響範圍最小、最容易 grep 鎖死。

**為什麼是 caller 補 `event` field 而非 library 自動推導**：sse-starlette 的 `EventSourceResponse` 不知道我們的 dict 內層 schema，本來也不應該假設 `data` 內藏的是 JSON 含 `type`。把責任放在 caller 是正確的關注點分離。

**Default 用 `"message"` fallback**：如果哪天 emitter 漏給 `type` field，wire 上的 `event:` line 會變成 `event: message`，前端 onmessage 仍能收到（degraded 但不壞）。比直接 `event["type"]` raise KeyError 安全。

**Alternatives 拒絕**：
- A. 改寫 `_json_dump` 把 `type` 拉出來：拒絕。`_json_dump` 名字暗示只負責序列化，加 wire-format side-effect 違反 SRP。
- B. 在 `TaskHandle.emit` 攔截補 type：拒絕。`emit` 是純資料層 API，不該知道下游 transport 是 SSE 還是 WebSocket。
- C. Frontend 一律改讀 inner `ev.data.type`：拒絕。這違反 EventSource 設計意圖（具名 listener 就是設計給 named events 用的），而且 `useExplorerStream` / `useQaSession` 已經寫成 `switch (ev.type)`，全改回頭路。

### Decision 2: Frontend `useSseTask` 把 `'error'` 從 `NAMED_EVENT_TYPES` 拿掉、改用獨立 gated listener

`web/app/composables/useSseTask.ts`：

```typescript
const NAMED_EVENT_TYPES = [
  'agent_thought', 'agent_action_result', 'judge_verdict',
  'coverage_gaps', 'usage_delta', 'llm_call', 'progress',
  'budget_warning', 'rag_hits', 'kb_growth', 'qa_answer', 'done'
  // 'error' moved to dedicated listener below
] as const

function attachListeners(es: EventSource): void {
  es.onopen = () => { ... }
  es.onerror = () => { ... reconnect ... }
  es.onmessage = (event) => { ... }

  for (const evType of NAMED_EVENT_TYPES) {
    es.addEventListener(evType, (event) => {
      const me = event as MessageEvent<string>
      appendEvent({ type: evType, data: parseData(me.data) })
    })
  }

  // Dedicated 'error' listener — MUST gate by MessageEvent instanceof
  // because EventSource also dispatches connection-level errors here.
  es.addEventListener('error', (event) => {
    if (!(event instanceof MessageEvent) || typeof event.data !== 'string') {
      return  // connection-level error — onerror handles reconnect
    }
    appendEvent({ type: 'error', data: parseData(event.data) })
  })
}
```

**為什麼**：HTML EventSource 規格定義 `error` 事件名同時被「connection drop」與「server-emitted `event: error` SSE message」共用。`onerror` 拿到的是前者（generic Event），具名 `addEventListener('error', ...)` 兩種都會收到。不能用同一條 callback 處理兩種來源——必須在 callback 內 `instanceof MessageEvent` 區分，且只把 `MessageEvent` 推進 events 陣列。

**為什麼 `onerror` 不需要動**：既有 `onerror = () => { es.close(); source = null; scheduleReconnect() }` 邏輯正確處理了 connection-level 錯誤（reconnect with backoff）。不該把它跟具名 error 混在一起，也不該在 `onerror` 裡 push events。

**Alternatives 拒絕**：
- A. 完全不註冊 `'error'` 具名 listener、改在 onmessage 裡看 inner `ev.data.type === 'error'`：拒絕。Bug 1 修好後伺服器 emit `event: error` 不會走 onmessage 而是走具名 listener；要走 onmessage 就得回頭把 server 拆掉 `event:` 行，跟 Decision 1 矛盾。
- B. 換 EventSource polyfill 之類能傳 headers 的庫（如 `@microsoft/fetch-event-source`）：拒絕。換庫成本高、`useSseTask` 還有 events cap / status state machine 一堆既有行為要重接，且 polyfill 本身仍受 HTML spec 約束（error 事件名衝突問題不會消失）。
- C. 把 connection-level 錯誤也 push 進 events 陣列、讓 consumer 自己看 `ev.data === 'undefined'` 過濾：拒絕。這把 SSE protocol 的 wire 細節漏到所有 consumer，違反封裝。

### Decision 3: Defensive test 從 wire 與 native event 兩層鎖死

**Sidecar wire-format test** — `sidecar/tests/api/test_tasks_sse_wire_format.py`：

```python
def test_sse_wire_includes_event_line_per_emission(...):
    # Use TestClient.stream() to capture raw bytes
    with TestClient(app).stream("GET", f"/tasks/{tid}/events", ...) as resp:
        body = b"".join(resp.iter_raw())
    # Assert each emitted event has BOTH lines
    assert b"event: progress\r\n" in body
    assert b"data: " in body
    assert b"event: done\r\n" in body
    # And event: line value matches inner JSON's type field
    ...
```

**為什麼用 raw bytes 而非 sse_starlette parsed events**：bug 就在 wire 層；上層 parsed events 看不到「`event:` line 缺漏」這個症狀。

**Frontend connection-error test** — `web/tests/composables/useSseTask.connection-error.spec.ts`：

```typescript
it('does not push event on EventSource connection close', () => {
  const mockES = createMockEventSource()
  vi.stubGlobal('EventSource', vi.fn(() => mockES))
  const { events } = useSseTask('scan_deadbeef')
  // Server closes connection — fire generic Event named 'error'
  mockES.dispatchEvent(new Event('error'))
  expect(events.value.filter(e => e.type === 'error')).toHaveLength(0)
})

it('pushes event on server-emitted error MessageEvent', () => {
  ...
  mockES.dispatchEvent(new MessageEvent('error', { data: '{"code":"X","message":"y"}' }))
  expect(events.value).toContainEqual({type:'error', data:{code:'X', message:'y'}})
})
```

**為什麼用 jsdom + `vi.stubGlobal` 而非真實 EventSource**：jsdom 沒實作 EventSource。我們需要 mock 才能精準控制 dispatch 哪種 Event 子類別、模擬 race。

### Decision 4: Spec delta 兩條都用 MODIFIED 不用 ADDED

兩個 capability 都是在**既有 Requirement 之下加新 Scenario**，不是新增獨立 Requirement。按 spectra spec 規約用 `## MODIFIED Requirements`，整個 Requirement block（含舊 Scenarios）必須完整貼回。

**為什麼**：MODIFIED 才會在 archive 時正確合併進 base spec 的同名 Requirement（在描述段加新句、在 Scenarios 末尾加新條）。如果用 ADDED 新建一條同名 Requirement，archive 會拒絕（重複名稱）或產生並列的兩條 Requirement。

## Risks / Trade-offs

- **Risk**: Phase 6 已 archive 的 explorer / qa consumer 之前在「壞的 SSE wire format」下到底跑出什麼結果，沒 ground truth。Bug 1 修好後可能暴露其他依賴於「外層 `ev.type` 永遠 = `'message'`」的隱性假設。
  → **Mitigation**: 修完跑 `cd web && npm run test --silent --run` 全綠 + 手動跑 `useExplorerStream` / `useQaSession` 的既有 unit test。如真有 regression，再開 follow-up change 處理（不在本 change 範圍）。
- **Risk**: Sidecar 已部署的 PyInstaller binary 沒這修，使用者跑舊 binary + 新 frontend 仍 broken。
  → **Mitigation**: tasks 末尾明確列「重打 PyInstaller binary」步驟，並在 archive 前驗 worktree 內 `sidecar/dist/codebus-sidecar-<triple>.exe` 的 mtime 比 fix commit 新。
- **Risk**: `event.get("type", "message")` fallback 萬一某個 emit 真的沒帶 type，`event:` line 會誤標 `message`，frontend onmessage 仍收到但業務邏輯不會推進。
  → **Mitigation**: 不需 mitigation。現有所有 emitter（scanner / kb / explore / generate / qa）都會帶 `type`，這是 task wrapper 與各 module 的既有約定。fallback 純粹保險；真要驗，把它做成 ERROR_CODES frozenset 之類也不在本 change 範圍。
- **Trade-off**: 把 `'error'` 從 `NAMED_EVENT_TYPES` array 拆出來，增加一個小的「不一致」（其他事件在 loop 裡註冊、`'error'` 在 loop 外獨立註冊）。但這個不一致是**必要的**——HTML spec 強制規定，不在 code 層分流就在 consumer 層誤判。寫一條 `// MUST be outside the loop because EventSource dispatches connection-level errors here too` 註解可保留可讀性。
