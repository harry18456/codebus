## Context

CLI 端 chat-verb 在 v3-chat-verb 已 archived：`codebus_core::verb::chat::run_chat_turn` 提供 per-turn stateless 函數，caller 控 REPL loop；spawn-time read-only sandbox (`CHAT_TOOLSET = ["Read", "Glob", "Grep"]`)；session_id 由 stream-json init event 取得，N+1 輪走 `--resume <id>`；`VerbLifecycleEvent::PromoteSuggestion { reason }` 從 line marker `[CODEBUS_PROMOTE_SUGGESTION] ...` parse；`RunLog` 加 `session_id: Option<String>` 欄位向後相容；spike ❶❷❸❺❻ 全 PASS。

GUI 端目前的 chat 入口 = 0（terminal only）。Workspace shell（`v3-app-workspace-goal` 完成）已建好 sidebar + tabs + goal lifecycle pipeline（`ipc/goals.rs` 約 580 行，含 `ActiveRuns` cross-thread state、`goal-stream` Tauri event、`spawn_goal` thread 模型、`catch_unwind` 隔離）— 為 chat IPC 提供完整 reference impl。

`ActivityStreamItem` / `ThoughtItem` / `foldTimeline` 已 render `VerbEvent` 形狀，chat 同型直接 reuse 不另寫一份。

Layout 收斂為 FB Messenger / Intercom widget pattern — 經比較 right drawer / floating / split-pane tab 後選定：保留 wiki 寬度的同時不付 floating 的工程稅，mental model 99% 用戶熟悉。

## Goals / Non-Goals

**Goals:**

- 提供 GUI 端 multi-turn chat 入口，邊看 wiki 邊問問題不互相阻擋
- Promote-to-goal flow 與既有 manual goal spawn 視同等地位（同一個 `spawn_goal` IPC、同一個 `RunDetailRunning` view）
- Session 在切 vault / 主動 New chat 之外都保留，跨 Workspace tab 切換不消失
- 後端複雜度集中在新 `chats.rs` IPC，前端複雜度集中在新 `useChatStore` + `ChatWidget` 元件樹
- 不動 archived 模組（chat-verb library / RunLog schema / VerbLifecycleEvent 變體）

**Non-Goals:**

- 不引入 disk-persist chat session（memory-only across app reload）
- 不做 Chat history picker / 多 session 切換 UI
- 不引入第四個 Workspace tab
- 不做 draggable / snap-to-edge widget 模式
- 不修改 Rust 後端 `run_chat_turn` 邏輯或 spawn flag 邏輯
- 不引入全域 font-scale 系統（另條 backlog 處理）

## Decisions

### Widget Layout — Bottom-right Corner Pinned, Two States

固定 bottom-right corner 16px 距邊緣。兩個 state：

- **Collapsed**：3rem × 3rem 圓形 bubble，icon `💬`，可加 unread badge（紅點）表示有未處理的 promote suggestion 或 turn 完成
- **Expanded**：22rem (≈ 352px @ base 16) × 32rem (≈ 512px) panel，含 header / transcript / input 三區
- Resize handle：僅 top-left corner 一個（其他 corner pinned to viewport edge），width 18–40rem / height 24–60rem
- Window resize 時：若 widget size 超過 50% × 80% viewport，自動 clamp 到 max
- **不可拖動** — 永遠 bottom-right，避免 panel 跑出 viewport 的 edge case

**為何選 widget 而非 right drawer**：drawer 永遠吃掉 ~22rem 寬度，wiki content 在 laptop 1366 視窗 + wiki tree 展開時剩 ~460px 過擠；widget collapsed 0 吃寬度、expanded 也只遮 bottom-right 一塊（wiki content 上半仍 full width）。

**為何不可拖動**：free-floating 需要 viewport clamping + persist position state + minimize chip location 設計，polish 工程量 2-3×，且 UX edge case 多（panel 跑出視窗、用戶忘記位置）。Widget pattern 用戶 99% 都用過（FB / Intercom）不需學習。

### Concurrency — Chat Turn 與 Goal Run 可同時存在於 active_runs

`ActiveRuns` 既有設計：HashMap keyed by `run_id`（= `<mode>-<started_at_slug>`），value 含 cancel flag。Chat turn 與 goal run mode prefix 不同，自然不衝突。

**規則**：

- 同一 vault 同時可有 1 個 active goal run + 1 個 active chat turn（互不阻擋，CHAT_TOOLSET 是 read-only 與 goal write 不衝突）
- `spawn_chat_turn` 若 active_runs 已有同一 vault 的 chat turn 進行中 → fail loud（v1 chat 一次一輪語意）：`AppError::Invalid { field: "active_runs", message: "another chat turn is already active in this session" }`
- Promote flow 觸發 `spawn_goal` 若 active_runs 已有 goal run → 既有 spawn_goal 邏輯會 reject（既有 spec scenario "Second spawn_goal during active run rejected at backend"），chat panel 端 catch 錯誤並在 promote pill 旁顯示 inline error "Another goal is running. Wait for it to finish."
- Chat 與 goal cancel 互相獨立：cancel chat turn 不影響 goal run，反之亦然

**為何**：chat 是 read-only 探索、goal 是 write 動作，語意上正交，user 一邊看 chat 思考一邊看 goal 跑是合理 workflow。Promote spawn 卡 goal 衝突走 fail-loud 比預設 queue 簡單。

### IPC Surface — Mirror goals.rs Pattern, Separate chat-stream Channel

新增 `codebus-app/src-tauri/src/ipc/chats.rs`：

- `spawn_chat_turn(vault_path: String, text: String, session_id: Option<String>) -> Result<ChatTurnRunId, AppError>` — 同 `spawn_goal` 的 `std::thread::Builder::spawn` + `AssertUnwindSafe(catch_unwind)` 模式，每輪一個獨立 thread；emit `chat-stream` Tauri event 帶 `{ run_id, event }` payload；on terminal state 在 emit 最後一個 event 後 cleanup `active_runs`
- `cancel_chat_turn(run_id: String) -> Result<(), AppError>` — 同 `cancel_goal` 的 idempotent cooperative cancel
- 內部呼叫 `codebus_core::verb::chat::run_chat_turn` with `ChatTurnOptions { text, session_id }` + `cancel: Some(Arc<AtomicBool>)`
- `chat-stream` 為獨立 Tauri event channel（與 `goal-stream` 並存），channel separation 讓前端 listener 不需 filter run_id 屬於 chat 還是 goal

**Promote spawn 走既有 spawn_goal IPC** — chat panel 端構造 transcript 字串後 call 既有 `spawn_goal(vault_path, transcript)`，不另開 `spawn_chat_promoted_goal` 之類的特殊 IPC。Goal lifecycle 與 manual spawn 完全相同（同 RunLog、同 RunDetailRunning view、同 active_runs 規則）。

**為何**：goals.rs 已是 580 行成熟 reference impl，含 cross-thread isolation、catch_unwind、active_runs cleanup 等 critical 機制 — chat 端 mirror 就少寫一份 cross-thread 心智模型 + 共享測試 pattern。Channel 分離是 spec-level 對等：goal / chat 各有 lifecycle、不該共用 event filter。Promote 借既有 spawn_goal 避免 IPC 增生、降低 spec 表面積。

### Transcript Dump Format for Promote

User 按 Promote pill → frontend 構造 transcript 餵給 `spawn_goal(vault_path, transcript)`。Format 與 CLI 端對齊（`chat-verb` 既有設計）：

```text
Based on this conversation:

<user>: auth 怎麼運作
<assistant>: ...（assistant 第一輪回答 text chunks 串接）...
<user>: JWT 也講
<assistant>: ...（assistant 第二輪回答 text chunks 串接）...
<user>: 幫我把這段寫成 wiki
<assistant>: [CODEBUS_PROMOTE_SUGGESTION] auth + JWT 適合寫成 wiki

Write: auth + JWT 適合寫成 wiki
```

- `<user>:` / `<assistant>:` literal label
- assistant 文字 = 該輪 `StreamEvent::Text` chunks 串接（不包含 tool_use / thinking / promote marker line）
- Last line `Write: <reason>` = 該輪 `PromoteSuggestion.reason`
- 整個 string 當 single goal text 餵給 `spawn_goal`

**為何**：transcript dump 是 codebus 概念非 claude 概念，必須在 caller 端組；CLI 與 GUI 共用同 format 讓未來 testability 一致；line marker convention 與 chat-verb SKILL.md 約定吻合（marker 在 assistant message 開頭）。

### useChatStore Schema

```ts
interface ChatStore {
  // Session 狀態
  sessionId: string | null              // null = 還沒有 turn / 剛 reset
  turns: ChatTurn[]                     // 已完成的 turns (user + assistant + tool events)
  activeTurn: ChatTurnLive | null       // 進行中的 turn (live events buffer)
  tokensTotal: TokenUsage               // session 累計 tokens
  promoteSuggestion: PromoteSuggestion | null  // 待處理的 promote pill (per current assistant turn)

  // Widget UI 狀態（per-vault memory-only）
  expanded: boolean                     // 展開 / 折疊 (bubble)
  width: number                         // rem unit，default 22
  height: number                        // rem unit，default 32
  onboardedVaults: Set<string>          // 看過 onboarding hint 的 vault path (mirror from localStorage)

  // Undo 緩衝
  lastTranscript: ChatTurn[] | null     // 5s undo buffer
  lastSessionId: string | null

  // Actions
  spawnTurn: (vaultPath: string, text: string) => Promise<void>
  cancelActiveTurn: () => Promise<void>
  newSession: () => void                // 5s undo toast 觸發點
  undoNewSession: () => void
  toggleExpanded: () => void
  setSize: (width: number, height: number) => void
  dismissPromoteSuggestion: () => void
  acceptPromoteSuggestion: (vaultPath: string) => Promise<void>  // 構 transcript → call spawn_goal IPC
  resetForVault: (vaultPath: string) => void   // 切 vault unmount 時呼叫
  markOnboarded: (vaultPath: string) => void   // 寫 localStorage
}
```

ChatTurn / ChatTurnLive 形狀 mirror `useGoalsStore` 的 `activeRun.events: VerbEvent[]`，每 turn 一個 events array。Onboarding hint 用 localStorage key `codebus-chat-onboarded-<sha1(vault_path)>` 持久化「看過了」（per-vault），其他 widget UI state memory-only。

**為何另立 store 不塞 useGoalsStore**：goals store 綁的是「one active goal at a time」semantic，chat 是 multi-turn 累積，語意不同；共用會讓 goal 邏輯被 chat turn 污染（例如 `activeRun` 含義不清）。

### Markdown Renderer Selection — react-markdown

Chat assistant message 用 `react-markdown`（不 reuse Milkdown）：

- Milkdown 是 editor-first（read-only mode 也是基於 ProseMirror），bundle 重、初始化慢，適合 wiki preview 但 overkill for chat
- react-markdown 純 React component + remark plugin chain，bundle 小、SSR-safe、可客製化 link renderer
- 需要自訂 `components.a` renderer 攔截 wiki link 點擊：

```tsx
<ReactMarkdown
  components={{
    a: ({ href, children }) => {
      if (href?.match(/^wiki\/.+\.md$/)) {
        return (
          <button
            onClick={() => {
              openWikiPage(href)
              collapseChat()
            }}
            className="..."
          >
            {children}
          </button>
        )
      }
      if (href?.match(/^https?:/)) {
        return (
          <a
            onClick={(e) => {
              e.preventDefault()
              void invokeOpener(href)
            }}
          >
            {children}
          </a>
        )
      }
      return <span>{children}</span>  // 其他 link v1 不可點，render as inert text
    },
  }}
/>
```

**為何**：react-markdown 更輕、客製化 link renderer 比 Milkdown 直觀；wiki citation 是核心需求所以 link renderer 要可控；外部 https link 用 plugin-opener 開瀏覽器；source code path 等 v2 再評估是否接 reveal-in-files。

### Cmd+K / Ctrl+K Shortcut — Workspace Layer Only

Hook `useChatShortcut` 在 `Workspace.tsx` mount 時 register、unmount 時 unregister。**不在 Lobby 註冊** — Lobby 沒有 vault context，shortcut 觸發無意義。

```ts
// useChatShortcut.ts
useEffect(() => {
  function handler(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "k") {
      e.preventDefault()
      useChatStore.getState().toggleExpanded()
    }
  }
  window.addEventListener("keydown", handler)
  return () => window.removeEventListener("keydown", handler)
}, [])
```

Mac / Windows / Linux 都 Cmd+K / Ctrl+K（`metaKey || ctrlKey` 兼容）；shortcut hint 文字在 onboarding hint 內顯示 platform-detect（`navigator.platform`）。

**為何**：spotlight pattern 在 Linear / Notion / Cursor 都用 K，學習成本 0；mac users 慣 Cmd+K，Win/Linux 慣 Ctrl+K，兩個都接相容性最高；shortcut 限 Workspace 避免 Lobby dead-end UX。

### Session Reset Behaviors

| Trigger | sessionId | turns | UI |
|---|---|---|---|
| 切 vault（Workspace unmount） | clear | clear | reset to collapsed bubble + onboarding hint per next vault |
| `+ New chat` button | save lastSessionId/lastTranscript → clear | save → clear | 顯示 5s undo toast「Started new chat. [Undo]」 |
| Undo within 5s | restore from last | restore from last | toast 收回 |
| 5s 過後 | gc lastSessionId/lastTranscript | gc | toast fade out |
| App reload | lost（memory-only） | lost | reset everything |
| Widget collapse/expand toggle | unchanged | unchanged | toggle 只動 expanded flag |

**為何**：切 vault 是 cross-cwd race 風險（spike ❸ 已驗）必須清；New chat undo 用 Gmail/Slack pattern 防誤觸；reload 不保留是 memory-only 策略明確 trade-off（避免 stale session 復活 + 不引入 disk persist 複雜度）。

## Implementation Contract

**對 apply 階段的 durable handoff** — 名實際 observable behavior + interface shape + acceptance criteria，不靠 line number 或 file path 當 contract 本體。

#### Behavior — User-facing

- Workspace mount 後 widget 立即可見（collapsed bubble，bottom-right corner）；shortcut Cmd+K / Ctrl+K 任何時刻按下 toggle expand/collapse
- 首次某 vault expand widget → transcript 區顯示 onboarding hint（含「Ask anything about this vault. AI will suggest [Promote to goal] when discussion is worth documenting — or just ask AI to promote it yourself.」+ 對應繁中翻譯）
- 第二次以後同 vault expand → 無 hint，placeholder「Type your message...」/「輸入訊息...」
- User 輸入 + Enter / Send button → 送出 `spawn_chat_turn(vault_path, text, sessionId)` → backend emit `chat-stream` events → activeTurn buffer 即時 render
- 進行中 turn → input 區換成 `⏹ Stop` 按鈕（取代 send button），按下 cancel turn 並保留 session_id
- Turn 結束（succeeded / cancelled）→ events 從 activeTurn 移到 turns 陣列固化 → input 區恢復；若 emit 過 `PromoteSuggestion` event → 該 assistant message 末尾 render inline pill `[Promote to goal: <reason>] [Dismiss]`
- 點 `[Promote to goal]` → frontend 構造 transcript dump → call `spawn_goal(vault_path, transcript)` → on success widget collapse to bubble + workspace `setActiveTab("goals")` + `setSelectedRunId(new_run_id)` → workspace 進入 `RunDetailRunning`
- `[Promote to goal]` spawn_goal 若 reject（其他 goal 已 active）→ pill 旁 inline error「Another goal is running. Wait for it to finish.」+ pill 維持可點重試
- 點 `+ New chat` button → save current to last buffer → clear → 顯示 5s toast「Started new chat. [Undo]」→ 點 Undo restore
- 切 vault（Lobby → 另一 vault 或返回 Lobby）→ store `resetForVault` 被 Workspace unmount cleanup 觸發 → session + transcript + size + undo buffer 全清
- Chat assistant message 內 markdown link `[label](wiki/.../*.md)` → 點擊切 Wiki tab + loadPage + chat collapse；`[label](https://...)` → 用 plugin-opener 開瀏覽器；其他 path → inert text 不可點
- Token usage 顯示 panel header 右側「`{N}k ↑`」session 總計；hover 顯示 tooltip 含 cache read / cache create / input / output 分項
- Activity stream render reuse 既有 `ActivityStreamItem` / `ThoughtItem` / `foldTimeline`，per-turn 上方加 user prompt block 區隔；tool_use 一行式（與 goal RunDetailRunning 視覺一致）

#### Interface — Tauri IPC

```rust
// codebus-app/src-tauri/src/ipc/chats.rs

#[tauri::command]
pub fn spawn_chat_turn(
    vault_path: String,
    text: String,
    session_id: Option<String>,
    app: AppHandle,
    active_runs: State<ActiveRuns>,
    state: State<AppRuntimeState>,
) -> IpcResult<ChatTurnRunId>;

#[tauri::command]
pub fn cancel_chat_turn(
    run_id: String,
    active_runs: State<ActiveRuns>,
) -> IpcResult<()>;

// Tauri event channel: "chat-stream"
// Payload shape: { run_id: String, event: VerbEvent }
```

```ts
// codebus-app/src/lib/ipc.ts (additions)

export async function spawnChatTurn(
  vaultPath: string,
  text: string,
  sessionId: string | null,
): Promise<ChatTurnRunId>

export async function cancelChatTurn(runId: string): Promise<void>

export interface ChatStreamPayload {
  run_id: string
  event: VerbEvent
}

export type IpcCommandName =
  | "spawn_goal" | "cancel_goal" | ...
  | "spawn_chat_turn"
  | "cancel_chat_turn"
```

#### Failure Modes

- `spawn_chat_turn` 同 vault 已有 active chat turn → `AppError::Invalid { field: "active_runs", message: "another chat turn is already active in this session" }`
- `spawn_chat_turn` keyring 抓不到 azure key → `VerbError::KeyringMissing` → 包成 `AppError::Internal` 含原訊息
- `spawn_chat_turn` 後 child process 拿不到 init event session_id → `VerbError::Internal` → 同上
- `cancel_chat_turn` run_id 不存在 → `Ok(())`（idempotent）
- Promote `spawn_goal` reject（其他 goal active）→ chat panel UI inline error，不關 widget
- React markdown link 點擊但 wiki page 不存在（slug 對應檔案缺失）→ 切 Wiki tab + WikiTab 自己處理空狀態（既有 spec 已 cover「No wiki pages yet」）

#### Acceptance Criteria

- 開 chat widget 在 collapsed 狀態 → bubble 位於 bottom-right、3rem 圓形、不擋住 wiki content（驗證：RTL test 查 widget `getBoundingClientRect` + wiki content full width）
- 按 Cmd+K → bubble 變 panel 展開（22×32rem 預設）；再按 Cmd+K → 折回 bubble（RTL test 模擬 keydown，assert `data-expanded` flip）
- Spawn turn → mock_claude 回 stream-json events → chat panel 顯示 user prompt + assistant text + tool_use one-liners（RTL test mock IPC + emit chat-stream events）
- Promote pill click → 構 transcript 字串符合 design 規格 + call mocked `spawn_goal` with 該 transcript（assert exact string match per design example）
- New chat undo within 5s → restore last transcript/sessionId（vitest fake timers + assert state shape）
- Widget resize 拖 top-left handle → store width/height 更新、clamp 在 18–40 / 24–60 rem 範圍（RTL pointer events test）
- 切 vault unmount → store cleared（RTL Workspace remount assertion）
- Wiki link click → setActiveTab("wiki") + loadPage 被 call、widget collapse（RTL test mock loadPage + assert sequence）
- 不重複輸入「Type your message」/「Ask anything…」hint 條件：第一次同 vault 顯示 hint、第二次顯示 placeholder（vitest localStorage mock）
- Token usage header 顯示「N.Nk ↑」+ hover tooltip 顯示分項（DOM assertion）

#### Scope Boundaries

**In scope:**
- 新 IPC commands + chat-stream Tauri channel + ActiveRuns 整合
- 新 ChatWidget UI 元件樹（widget shell + transcript + input + promote pill + onboarding hint + token display + undo toast）
- useChatStore + Cmd+K shortcut hook
- chat assistant message markdown rendering + wiki link click handler
- Workspace mount Wiring（widget 在 Workspace 層級而非 tab 層級）
- i18n keys 雙語
- Unit + integration tests for above

**Out of scope:**
- Rust 後端 `verb::chat::run_chat_turn` 任何改動（archived）
- chat session disk persistence / history picker / 第四 tab
- Draggable / snap-to-edge widget mode
- 全域 font-scale 系統（另條 backlog）
- Source code path citation link / inline images / file attachments
- Starter prompt cards / pre-baked questions
- Cmd+K from Lobby（無 vault context）
- Promote 失敗 queue / 自動 retry（fail-loud only）

## Risks / Trade-offs

| Risk | Mitigation |
|---|---|
| Widget 同時 expanded + wiki tree 展開時 wiki content 太擠（1366px laptop） | 用戶可自行折 wiki tree（已支援 toggle）、widget resize 縮小到 18rem 最小寬。Tooltip 可加 hint 提示 |
| react-markdown bundle 增加（~30-50kb gzip） | 可接受 — chat 是核心功能、bundle 影響有限；alternative reuse Milkdown 反而更重 |
| Markdown 內 wiki link path 偵測誤判（例如 `[abc](wiki/foo.md)` 但 `foo.md` 不存在） | onClick handler 先 call loadPage，WikiTab 既有「空狀態 / 找不到」邏輯處理 — 不需在 chat panel 端先 validate |
| Promote suggestion 同一個 assistant turn 內 emit 多次 | chat-verb SKILL.md 約定每 message 最多一次 + library 端 parse 用 `line.starts_with()` 抓第一個 — 多餘 emit 視為 user dismiss 過後再 emit，UI 只 render 最新的 |
| Cmd+K 與其他 shortcut 衝突（瀏覽器原生 Ctrl+K 是 search bar） | Tauri webview 不會 leak browser shortcut；測試確認 webview 內 Ctrl+K 不觸發 OS 行為 |
| Widget state 切 vault 沒清乾淨（race condition） | Workspace unmount 一律 call `resetForVault` + 用 vaultPath 當 key 比對防錯；test 覆蓋 Lobby ↔ Vault ↔ 另一 Vault round-trip |
| Active chat turn 中按 New chat 行為未定義 | spec 明定：先 cancel active turn → 再走 New chat undo flow；測試 cover |
| Promote spawn_goal reject 但 chat 維持可用 → user 可能不知道 goal 沒開 | inline error 顯示具體原因 + pill 維持可點重試；spec scenario 明確 |
| Tauri 單視窗限制下多 monitor user 無法 detach chat 到第二螢幕 | v1 不解；v2 視 user feedback 評估 Tauri window plugin spawn second window |
