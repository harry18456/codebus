## Why

v3-roadmap 主線 D 條 GUI chat overlay。CLI 端 chat verb 已於 `v3-chat-verb` archive — multi-turn read-only REPL + Promote-to-goal 機制 + activity stream + cancel/resume 全部完備。但 GUI 端目前只有 Goals / Wiki / Quiz 三個 tab，使用者要對 vault 內容做探索性問答必須跳到 terminal 跑 `codebus chat` — 違背「App 一站式」design principle。

設計討論（conversation context + `docs/2026-05-13-chat-verb-discussion.md`）確認 GUI chat 需要：(1) 邊看 wiki 邊問 — 不能用 modal 蓋住 wiki content；(2) 持續可問 — 切 tab 不消失、session 在 user 主動觸發前不重置；(3) Chat 自動建議 promote、user 也能主動跟 AI 講想 promote — 雙路徑都走同一個 PromoteSuggestion lifecycle event；(4) Chat-spawned goal 出現在 Goals 列表（透過既有 spawn_goal IPC，與手動 spawn 視同等地位）。

Layout 收斂為 FB Messenger / Intercom 風格 widget — 固定 bottom-right corner、collapsed 3rem bubble / expanded 22×32rem panel — 比 right drawer 更保留 wiki 寬度、比 free-floating 簡單 2-3 倍、mental model 幾乎所有用戶都熟悉。

## What Changes

- 新增 Chat Widget 元件：固定 bottom-right corner、collapsed 3rem bubble / expanded 22×32rem panel、resizable from top-left corner only（width 18–40rem / height 24–60rem）、Cmd+K / Ctrl+K toggle、不可拖動、rem-based 寬高（font-scale-friendly）。
- 新增 Tauri IPC commands：`spawn_chat_turn(vault_path, text, session_id?) -> ChatTurnRunId` 與 `cancel_chat_turn(run_id) -> ()`，mirror 既有 `goals.rs` 的 thread + emit pattern；新增 `chat-stream` Tauri event channel 與 goal-stream 並存。
- 新增 `useChatStore` Zustand store：管理 session_id、transcript (multi-turn buffer)、active turn live events、collapsed/expanded state、resize size、onboarded-per-vault flag、token usage 累計。
- Activity stream render 重用既有 `ActivityStreamItem` / `ThoughtItem` / `foldTimeline`（chat-verb emit 的 `VerbEvent` 形狀與 goal 一致）；chat panel 內以 turn 為單位分隔顯示。
- Promote UX：chat agent emit `VerbLifecycleEvent::PromoteSuggestion { reason }` → chat panel 在該 assistant message 末尾顯示 inline pill `[Promote to goal]` + `[Dismiss]`；按下 Promote → 構造 transcript 字串 → call 既有 `spawn_goal(vault_path, transcript)` IPC → chat 自動 collapse to bubble → workspace 切到 Goals tab + 跳 `RunDetailRunning`。
- Onboarding hint：per-vault 第一次 expand chat panel 時，transcript 區顯示一行 hint「Ask anything about this vault. AI will suggest [Promote to goal] when discussion is worth documenting — or just ask AI to promote it yourself.」(en) / 對應繁中翻譯；localStorage key `codebus-chat-onboarded-<vault-hash>` 記住「看過了」，後續 session 改顯示 placeholder。
- Token usage：panel header 右側顯示「`{N}k ↑`」session 累計（從 stream-json `usage` event 累加 input + output tokens），hover 顯示 tooltip 含 cache read / cache create / input / output 分項；不顯示 USD。
- Session reset triggers：(1) 切 vault 自動清（cross-cwd `--resume` race，spike ❸ 已知問題）；(2) `+ New chat` 按鈕清 + 5s undo toast「Started new chat. [Undo]」恢復 lastTranscript + lastSessionId；(3) app reload memory-only 不保留（不寫 disk）。
- Cancel / interrupt：當前 turn 進行中 input 上方顯示 `⏹ Stop` 按鈕，按下 call `cancel_chat_turn(run_id)` → 當前 turn cancel、session_id 保留 → 下輪輸入自動帶 session_id call `spawn_chat_turn` → backend 走 `--resume <id>` 接續（claude CLI 內建 handshake，spike ❻ 已驗）。
- Wiki citation：chat panel 的 assistant message 用 markdown renderer（react-markdown 或 reuse Milkdown read-only mode）；markdown link `[label](wiki/.../*.md)` 點擊 → preventDefault + 切 Wiki tab + `loadPage(vault, slug)` + chat collapse to bubble；非 wiki path 的 link v1 不處理（avoid 開 IDE 行為混亂）；plain text mention 不自動 link（避免 regex false positive）。
- Tab persistence：chat widget mount 在 Workspace 層級而非 tab 層級，切 Goals / Wiki / Quiz tab 時 chat panel 不消失也不重 mount；切 vault（unmount Workspace）才清。
- Onboarding hint 設計提及 promote 雙路徑（AI suggest + user request），對應 chat-verb SKILL.md 已實作的雙觸發 — agent 收到「幫我把這段寫成 goal」這類 user phrase 會下一輪 emit PromoteSuggestion，GUI 端不需特別 detect user-side phrase，仍由 agent emit 統一觸發。

## Non-Goals

- 不修改 Rust 後端 `codebus_core::verb::chat::run_chat_turn` 或 `VerbLifecycleEvent::PromoteSuggestion` 行為 — chat-verb 已 archived 完整支援。
- 不引入 chat session history picker / 多 session 切換 UI — memory-only across app reload。要回顧舊 session 走 terminal `codebus chat --resume <id>`。
- 不把 chat 每輪的 `mode: "chat"` RunLog 顯示在 Goals tab — Goals filter 只顯示 `mode == goal`（含 chat-promoted goal），不加 from-chat 來源標籤。
- 不做 Chat tab（與 Goals/Wiki/Quiz 並列的第四 tab）— widget pattern 不需要 tab；session 不持久化也不適合單獨 tab。
- 不做 starter prompt cards（vault-aware 泛用 prompt 太難寫得好，v2 再評估）。
- 不做全域 font-scale 系統 — 該議題已開另條 backlog `docs/2026-05-14-app-font-scale-backlog.md`，此 change 只用 rem-based 寬高讓未來 font-scale 落地時自動生效。
- 不做 draggable panel — widget pattern 固定 bottom-right corner，不開放任意拖動以避免「panel 跑到 viewport 外」edge case。
- 不做 snap-to-edge 切換 drawer mode（floating ↔ docked）— v1 widget pattern 單一形態，v2 視 user feedback 再評估。
- 不做 wiki link 以外的 citation（source code path / external URL）clickable — v1 只 wiki link，source code link v2 評估是否接 reveal-in-files。
- 不做 chat 內 image 上傳、檔案 attach、語音輸入 — pure text REPL。
- 不做 chat with multiple concurrent vaults — per-vault state，切 vault 重置。
- 不做 promote suggestion 的 edit mode（在按下 Promote 前修改 reason 文字）— chat-verb spec v1 已 ruled out。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-workspace`: 新增 Chat Widget 相關 requirements（layout、collapsed/expanded state、Cmd+K toggle、session lifecycle、activity stream 重用、Promote pill UI、Onboarding hint、Token usage display、5s undo toast、Wiki citation link、tab persistence）；新增 `spawn_chat_turn` / `cancel_chat_turn` 兩個 IPC commands 進「Tauri IPC Commands for Goal Lifecycle and Wiki Read」requirement（重命名或擴展）；修改「One Active Goal Run At A Time」requirement 釐清 chat turn 與 goal run 的並發語意（chat turn 與 goal run 可同時存在於 `active_runs`，互不阻擋；promote 觸發的 goal spawn 若 active_runs 已有 goal 則 fail loud 並在 chat panel 內顯示錯誤）。

## Impact

- Affected specs: `app-workspace`（修改現有 requirements + 新增 Chat Widget requirements 系列）
- Affected code:
  - New:
    - codebus-app/src/components/workspace/ChatWidget.tsx（主元件，含 bubble + expanded panel + resize handle）
    - codebus-app/src/components/workspace/ChatTranscript.tsx（multi-turn transcript render + markdown renderer + promote pill）
    - codebus-app/src/components/workspace/ChatInput.tsx（textarea + stop button + send）
    - codebus-app/src/components/workspace/ChatWidget.test.tsx
    - codebus-app/src/components/workspace/ChatTranscript.test.tsx
    - codebus-app/src/components/workspace/ChatInput.test.tsx
    - codebus-app/src/store/chat.ts（useChatStore：session_id / transcript / activeTurn / widget UI state / token累計 / onboarded flag）
    - codebus-app/src/store/chat.test.ts
    - codebus-app/src/hooks/useChatShortcut.ts（Cmd+K / Ctrl+K toggle）
    - codebus-app/src/hooks/useChatShortcut.test.tsx
    - codebus-app/src-tauri/src/ipc/chats.rs（spawn_chat_turn / cancel_chat_turn + chat-stream emit + ActiveRuns 整合）
    - codebus-app/src-tauri/tests/chats_ipc.rs（IPC integration tests via mock_claude）
  - Modified:
    - codebus-app/src/components/workspace/Workspace.tsx（mount ChatWidget 在 Workspace 層級、Cmd+K shortcut wiring、promote flow 切 Goals tab + 跳 RunDetailRunning）
    - codebus-app/src/lib/ipc.ts（新增 spawn_chat_turn / cancel_chat_turn typed wrappers + ChatTurnRunId / ChatStreamEvent types + IpcCommandName union 延伸）
    - codebus-app/src/lib/ipc.test.ts（新 IPC wrapper type test cases）
    - codebus-app/src-tauri/src/ipc/mod.rs（register chat commands + extend REGISTERED_COMMANDS list）
    - codebus-app/src-tauri/src/state/active_runs.rs（如需區分 chat / goal mode 則 schema 微調；目前 RunId 已含 mode prefix 應可重用，待 design.md 確認）
    - codebus-app/src/components/workspace/WikiTab.tsx（新增 external `loadPage(slug)` trigger pathway 給 chat citation link 用 — 可能已存在，待 design 確認）
    - codebus-app/src/i18n/messages.ts（新增 chat widget 相關 i18n key：onboarding hint、placeholder、stop button、new chat、undo toast 等，雙語 en / tw）
    - codebus-app/src/App.tsx（如 Cmd+K shortcut hook 在 App 層 register 才需動；若在 Workspace 內 register 則此檔不動）
  - Removed: (none)
