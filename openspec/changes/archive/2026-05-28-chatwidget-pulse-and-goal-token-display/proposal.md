## Why

兩個 ChatWidget / Goal UX 議題綁同 change 處理（同 surface、scope 相鄰、可一次到位避免暫態 visually worse）：

1. **Bug 2 · ChatWidget bubble pulse dot 觸發語意爭議**：collapsed bubble 右上角 accent pulse dot 目前依 `useGoalsStore.activeRun` 觸發（per 5.1 archive `chatwidget-pulse-and-cancel-move` ODI-4 spec）。User 回報跑 goal 時 chat bubble 出現橘點「感覺錯」（直覺以為 chat bubble 上的點代表 chat 狀態、實際代表 goal 狀態）。實際 wire 跟 spec 完全一致 → 議題是 **user-facing 語意對齊問題**，apply 階段需先 disambiguation 再選 fix shape。
2. **Goal token 跑中顯示 0**：`RunDetailRunning.tsx:148-155` `collectTokens` 純 sum stream Usage events、Claude CLI 整 spawn 只在 final result 階段 emit 一次 Usage（spec `agent-stream-rendering` 已記錄此限制）→ running 期間 sum 永遠 0、user 看到「0 tokens」誤導為「沒花 token」。Codex provider `turn.completed` 可能 per-turn 帶 Usage、行為要 verify。

## What Changes

### Bug 2 · ChatWidget pulse dot 語意對齊（apply 階段 disambiguation 後動工）

Apply 階段 Task 1 SHALL 先 chat 用戶 disambiguation 三個對立解讀（**propose 階段不憑印象決定**）：

- **Path a · spec 即實作 = 正確、加 clarity**：tooltip / aria-label / docs 強化「pulse = goal ambient signal、寄在 chat bubble 屬 design choice」的訊息傳達。Spec 不動、code 動最小（aria-label 補強或不動）。
- **Path b · 視覺位置 mismatch、搬 indicator**：把 ambient goal indicator 從 chat bubble 搬離（候選位置：Goals tab nav icon / BottomStrip / Workspace 側欄）。Spec `app-workspace` 5.1 ODI-4 段需 modify、layout 動較大。
- **Path c · 排除**：rewire `hasActiveGoal` source（如改 bound `chat-running`）。Propose 階段已 grep `ChatWidget.tsx:82` 確認實作 wire 與 spec 完全一致，**path c 不存在 root cause**，已從候選排除。

### Goal token · A（必做）+ C（順手 verify）

- **A · Running 時不顯示誤導 0**：`RunDetailRunning.tsx` token display 在 `outcome === "running" && sum === 0` 時改顯示「—」或本地化「計算中…」字面，避免 user 誤判為「沒花 token」。Done 後恢復正常顯示。
- **C · Codex per-turn Usage 驗證**：CDP smoke 跑 codex provider goal、觀察 `turn.completed` Usage 是否 per-turn 累加；若是、A 邏輯在 codex 路徑會自動「逐 turn 改善」，無需動程式；若否、開 follow-up doc。
- **B / D · defer**：Claude CLI incremental usage flag（need docs research）與 estimated tokens（accuracy risk 高）皆不做。

### Backlog doc update

- `docs/2026-05-28-four-bugs-backlog.md` Bug 2 段加 archived 記號（archive 階段更新）
- `docs/2026-05-28-goal-token-display-streaming-todo.md` 標 archived（archive 階段更新）

## Non-Goals (optional)

- 不動 ChatWidget 三 modes（bubble / floating / centered modal）—— 屬 `chatwidget-three-modes` 範圍
- 不動 backend Claude CLI 行為（B 是 follow-up doc、不在本 change scope）
- 不做 estimated tokens（D 路徑 accuracy risk 高、user 明確排除）
- 不把 chat session in-progress 跟 goal running 兩 state 重新 wire 混淆 —— `ChatTokenDisplay` 走 `useChatStore.tokensTotal`、`RunDetailRunning` 走 stream Usage、兩 path 獨立、不交叉
- 不拆兩個 change（user 已決議 batch）
- 不改 stream parser / `StreamEvent` schema —— Usage event 一次性 emit 是 spec 規範的既有行為、本 change 不改
- 不改 `cancel-button` / pulse dot testid / i18n key 命名（value-only 變動）

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-workspace`: 視 Bug 2 disambiguation 結果：path a → spec 不動（implementation-only adjustment）；path b → spec 5.1 ODI-4「Chat Widget Layout and Two-State Toggle」段重寫 pulse dot 位置定義。Goal token 部分 → `Run Detail Views — Running` 加 NOTE「running 期間 token display SHALL NOT show literal 0 when no Usage event received yet」。

## Impact

- Affected specs:
  - `app-workspace` (modified) —— Bug 2 path b 則 ODI-4 段重寫；Goal token 則 Running view 段加 NOTE
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/RunDetailRunning.tsx
    - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
    - codebus-app/src/i18n/messages.ts
    - codebus-app/src/components/workspace/ChatWidget.tsx (條件性，視 path a/b)
    - codebus-app/src/components/workspace/ChatWidget.test.tsx (條件性)
    - docs/2026-05-28-four-bugs-backlog.md (archive 階段更新)
    - docs/2026-05-28-goal-token-display-streaming-todo.md (archive 階段更新)
  - New: (none)
  - Removed: (none)
