## Context

### 為何兩個 sub-feature 綁同 change

Bug 2（ChatWidget pulse dot 語意爭議）與 Goal token 顯示 0 是兩個獨立議題、但都集中在 Workspace UI signal cleanup 範疇，且 token 顯示問題在 chat 端（ChatTokenDisplay）與 goal 端（RunDetailRunning）都需 verify、ChatWidget bubble 與 goal running 狀態又彼此語意相關，綁同 change 可一次掃過「user 看到的 UI signal 是否誠實」這層問題。User 已決議 batch、不拆兩 change。

### Pre-apply 校準（grep + Read spec ground truth、不憑印象）

依 `project_phase_3a_blind_spots_cleanup_lessons` lesson 1 與新立規 `propose-risks-out-of-scope-must-reproduce`，propose 階段已完成以下 ground truth 校準：

**ChatWidget 現況**（ChatWidget 元件檔）：

- pulse dot 觸發 source：`useGoalsStore((s) => s.activeRun != null)` —— 完全沒 wire 到 chat state
- pulse dot markup 永遠 mount、`opacity-0` / `opacity-100` 切換、`transition-opacity duration-200`
- aria-label 條件切換 `chat.widget.aria.openChatWithActiveGoalRunning`（hasActiveGoal）vs `chat.widget.aria.openChat`
- 既有 comment 已說明 always-mounted 是為了 fade animation

**結論**：實作 wire 跟 spec ODI-4 完全一致。User prompt 三 path 之中、**path c（實作 wire 錯）已從候選排除**，propose 階段已 grep 驗。剩 path a vs path b 待 apply 階段 chat user。

**5.1 archive chatwidget-pulse-and-cancel-move 的 app-workspace spec ODI-4 原意**（已 Read verbatim）：

- Pulse dot 由 `useGoalsStore.activeRun` 觸發
- Pulse dot 寄在 collapsed bubble 右上角是 design choice（理由：chat 是不變的 surface、適合掛 background indicator）
- Pulse dot 與 promote badge 視覺並存設計，位置 + 顏色 + size 都分開
- 螢幕閱讀器 aria-label 在 active goal 期間切換 openChatWithActiveGoalRunning、明確宣告「有 goal 在跑」

**Backlog doc 與 user prompt root cause 表述差異**（已標、apply 階段對齊）：

- four-bugs-backlog doc Bug 2 段寫「cross-wiring confirmed」、「goal running 跟 chat session in-progress 互相 leak」
- User propose prompt 重新 frame 為 3 path（a/b/c）、未斷定 cross-wiring
- 實際 ChatWidget 元件沒看到 cross-wire 證據（useChatStore 與 useGoalsStore 兩 store 用法獨立）
- **這是 propose 階段必須 expose 的 narrative drift**，apply Task 1 先跟 user 對齊「root cause 到底是 cross-wiring（要找）還是 user-facing 語意 mismatch（path a 或 b）」、再選 path

**Goal token 路徑現況**：

- RunDetailRunning 的 `collectTokens` 純 sum stream Usage events
- Running 期間總和為 0（無 Usage event 抵達）
- codebus-core Claude parser 的 `result` event 攜帶 Usage、整 spawn stream 只 emit 一次（在 final result 階段）—— spec `agent-stream-rendering` 已記錄此 wire format
- codebus-core Codex parser 的 `turn.completed` 帶 Usage、可能 per-turn 累加（apply 階段 CDP smoke verify）
- RunDetailDone 用 RunLog summary、token 顯示正常
- ChatTokenDisplay 走 `useChatStore.tokensTotal`、store reducer 每次 `stream-event { kind: usage }` 更新、跟 goal 路徑獨立、但同樣受 Claude CLI 整 spawn 只 emit 一次 Usage 限制

### 詞彙 disambiguation（per project_quiz_fullscreen_wizard_view_term_disambiguation）

本 change 涉及三個詞跨 source 語意不一致、design verbatim 釐清：

| 詞 | User backlog doc 表述 | User propose prompt 表述 | 實作 ground truth |
|---|---|---|---|
| **pulse / 橘點** | 「ChatWidget bubble 在 chat 回應時就顯示橘點」（暗示 chat 觸發） | 「collapsed bubble 右上角 dot、bound to hasActiveGoal」（path a/b/c 之中 c 排除） | testid `chat-widget-active-goal-pulse`、`hasActiveGoal = useGoalsStore(s => s.activeRun != null)` |
| **indicator** | 未顯式用此詞 | path b「搬 ambient goal indicator 到 non-chat 位置」 | spec ODI-4 用「accent-coloured pulse dot indicator」描述 |
| **active goal running** | 「之前有修復類似的問題（使用 chat、UI 會顯示有 goal 正在跑）」 | 「pulse = goal ambient signal、寄在 chat bubble 是 design choice」 | `useGoalsStore.activeRun !== null` 為 source of truth、vault-scoped、切 vault 時 reset |

**Apply Task 1 必須 chat user 確認**：

- User 看到 pulse dot 出現的時機、到底感覺「錯」在哪？
  - (i) 跑 chat 時 dot 不該亮 —— 但實作沒有這樣 wire；user 觀察可能是 chat 同時 trigger 了 goal、或記錯 trigger source
  - (ii) 跑 goal 時 dot 不該寄在 chat bubble —— 屬 path b（搬 indicator）
  - (iii) 跑 goal 時 dot 該亮、但 user-facing 解釋不夠清楚 —— 屬 path a（加 clarity）

## Goals / Non-Goals

**Goals:**

- Bug 2：透過 apply 階段 disambiguation，把 user 對 ChatWidget pulse dot 的「感覺錯」收束到具體 path（a 或 b），並落實對應 fix
- Goal token：running 期間不顯示誤導的「0 tokens」，user 看到本地化 placeholder（如「—」/「計算中…」）
- 跨 provider 行為驗證：Codex `turn.completed` 帶 Usage 的 per-turn 累加假設、CDP smoke 驗（順手）
- Backlog doc 同步歸檔，避免 stale docs

**Non-Goals:**

- 不動 ChatWidget bubble 三 modes（bubble / floating / centered modal）
- 不改 stream parser / StreamEvent schema、不改 Claude CLI Usage emit 行為（B 路徑 follow-up）
- 不做 estimated tokens（D 路徑）
- 不改 chat session in-progress 跟 goal running 兩 state 的 wire 關係（兩 state 已獨立、不交叉）
- 不拆兩個 change

## Decisions

### Decision 1 · Bug 2 fix path apply 階段 disambiguation 後選定

Propose 階段不憑印象決定 fix path，apply Task 1 透過 chat 跟 user 對齊三 path 後選一條動工。三 path 工時 + spec 影響範圍：

| Path | 含意 | Code 動 | Spec 動 | 工時 |
|---|---|---|---|---|
| **a · 加 clarity** | spec 即實作 = 正確、user-facing 解釋強化（aria-label / tooltip / docs） | 最小、可能只動 i18n value（不改 key 命名）+ 加 `title` 屬性 / tooltip | 無（implementation aligns spec） | 30-60 min |
| **b · 搬 indicator** | pulse dot 從 chat bubble 搬離、改掛 non-chat surface（Goals tab nav icon / BottomStrip / Workspace 側欄） | Workspace layout + ChatWidget 移除 pulse markup + 新增 indicator 元件 + test 同步 | `app-workspace` spec 5.1 ODI-4 段重寫（pulse dot 位置定義 + scenario 全改） | 90-150 min |
| **c · rewire（排除）** | 改 `hasActiveGoal` source（如 bound `chat-running`） | propose 階段 grep 已確認實作沒 wire 錯、path c 不存在 root cause | N/A | N/A |

**選**：path 留到 apply Task 1，**不在 propose 階段做選擇**。User prompt 已明確要求 chat user disambiguation。

**否**：propose 階段強行選 path a（看似最小）—— 違反「不憑印象決定」原則，且 user 對「感覺錯」的具體型態未確認，跳級可能修錯方向。

**否**：propose 階段強行選 path b（最大改）—— 同樣未確認 user 預期、可能過度改動 + spec rewrite 工時暴增。

### Decision 2 · Goal token running 期間顯示 placeholder（A 路徑必做）

Running 期間 `outcome === "running" && tokensSum === 0` 時 token display 改顯示本地化「—」字面（或「計算中…」依 i18n bundle 選定）。Done 後恢復原有 token 數字顯示。

**選**：A 路徑（隱藏誤導 0）。

**理由**：

- Claude CLI Usage event 在 final result 才 emit、running 期間 sum 結構上必為 0
- 顯示「0 tokens」會誤導 user 為「沒花 token」、實際是「還沒收到 Usage event」
- placeholder 是中性占位符、不暗示精確值、user-facing 誠實
- 純 frontend 動、工時 < 30 min

**否**：B 路徑（Claude CLI incremental usage flag）—— 需查 Claude docs 確認 flag 是否存在、屬 follow-up
**否**：D 路徑（estimated tokens）—— accuracy risk 高、user 明確排除

### Decision 3 · Codex per-turn Usage verify only（C 路徑驗證、不動程式）

Codex parser 已從 `turn.completed` emit Usage、若 codex 真的 per-turn 帶 Usage、`collectTokens` sum 行為會自動 per-turn 改善、無需動程式。

**選**：CDP smoke 跑 codex provider goal、觀察 Usage event 累加行為。

**驗證點**：

- 跑 codex provider goal（multi-turn）
- 監測 RunDetailRunning token display：是否從 placeholder → 第一 turn 完後 → 第二 turn 完後逐步增加
- 若是 → C 自動解、A 邏輯不需特別處理 codex 路徑
- 若否 → 開 follow-up doc、本 change scope 不擴

**否**：動 codex parser 邏輯 —— 出 scope、應屬 multi-provider backlog

### Decision 4 · ChatTokenDisplay 同 pattern check（不擴 scope）

ChatTokenDisplay 走 `useChatStore.tokensTotal`、零態顯示「0 ↑」（per 既有 `Chat Token Usage Display` spec）。**本 change 不動 chat 端**、理由：

- Chat 端「0 ↑」是 spec 規範的「fresh-session zero state」、屬刻意設計
- Chat 的 zero state 跟 goal running「等 Usage event」語意不同（chat 是「還沒開始 chat」、goal running 是「跑中但 Usage 還沒抵達」）
- 改 chat 端會破壞既有 `Chat Token Usage Display` spec 與 widget header stable layout 設計

**選**：本 change 只動 RunDetailRunning 路徑、不動 ChatTokenDisplay。Apply Task 結尾 CDP smoke 順手對 chat 端視覺驗一下沒被連帶影響即可。

### Decision 5 · Spec 改動範圍延後到 apply 階段定

Spec `app-workspace` 是否需 modify 視 Bug 2 path 選擇：

- Path a → spec 不動、純 implementation
- Path b → spec ODI-4 段重寫 + 對應 scenarios 全改

Goal token 部分：`Run Detail Views — Running` requirement 加一條 NOTE「running 期間 token display SHALL NOT show literal 0 when no Usage event received yet; SHALL render localized placeholder until first Usage event arrives」+ 對應 scenario。**這條 spec NOTE 不視 Bug 2 path 選擇都做**。

**選**：spec 在 propose 階段先寫 Goal token NOTE + scenario（必做部分）；Bug 2 部分留條件 wording、apply 階段視 path 補完或留 spec 不動。

## Implementation Contract

### Behavior（user-observable）

**Bug 2（apply 階段 disambiguation 後選定 path）**：

- Path a 落實：collapsed bubble pulse dot 出現時、user 透過 hover tooltip / aria-label SHALL 看到/聽到清楚的「goal running」語意說明；UI 結構不變
- Path b 落實：collapsed bubble SHALL NOT 渲染 pulse dot；對應 non-chat surface SHALL 在 `useGoalsStore.activeRun !== null` 期間顯示 ambient goal indicator

**Goal token（A 路徑必做）**：

- RunDetailRunning token display 在 `outcome === running && tokensSum === 0` 時 SHALL 顯示本地化 placeholder、SHALL NOT 顯示字面「0 tokens」
- 第一個 Usage event 抵達後 SHALL 顯示真實累積值
- Done 狀態（RunDetailDone）顯示行為不變（用 RunLog summary）

**跨 provider 行為**：

- Claude provider：running 期間始終顯示 placeholder（直到 final result Usage event），Done 後顯示完整 token
- Codex provider：若 `turn.completed` 真 per-turn 帶 Usage、running 期間第一 turn 完後 placeholder 變數字、後續 turn 累加；若否、行為跟 Claude 一致

### Interface / data shape

- 新增 i18n key：`workspace.runDetail.tokensRunningPlaceholder`（en、zh 兩個 locale 同步加入 `messages.en` 與 `messages.zh`）—— **key 命名 propose 階段暫定、apply Task 動工前 grep verify**
- 修改：RunDetailRunning token display 條件渲染邏輯（純 frontend、無 props 變動）
- 不動：useGoalsStore shape、StreamEvent Usage schema、collectTokens 函式簽名（只動條件渲染處）
- 不動：ChatTokenDisplay、useChatStore.tokensTotal、chat 端任何 store / component

**Bug 2 path a 落實時**（可能）：

- 新增/修改 i18n key：`chat.widget.aria.openChatWithActiveGoalRunning` value 強化（不改 key 命名 per 既有 5.1 archive 教訓）
- 可能新增 `title` 屬性在 pulse dot 上、給 mouse hover tooltip

**Bug 2 path b 落實時**（可能）：

- 移除 `chat-widget-active-goal-pulse` testid 元素
- 移除 `chat.widget.aria.openChatWithActiveGoalRunning` key（或保留作 fallback）
- 新增非 chat surface 的 ambient indicator 元件（testid + i18n 命名 apply Task 確認）

### Failure modes

- useGoalsStore 未初始化 / activeRun undefined → coerce false、placeholder 不渲染（既有 graceful degrade pattern）
- tokensSum 計算過程中 stream event 異常 → 維持顯示 placeholder（不 crash）
- i18n key missing → fallback 鍵名顯示（既有 i18n 行為）
- CDP smoke codex per-turn Usage verify 結果為「否」→ 開 follow-up doc、本 change scope 不擴

### Acceptance criteria

1. `pnpm tsc` 綠
2. `pnpm test` 綠，含新 test：
   - RunDetailRunning test：`outcome === running` + 無 Usage event 時 token display 不渲染「0」字面、改渲染 placeholder
   - RunDetailRunning test：第一 Usage event 抵達後 token display 變實際值
   - RunDetailDone 既有 test 不變（用 RunLog summary path）
   - Bug 2 視 path：path a 則 ChatWidget aria-label / tooltip 加新 test case；path b 則 ChatWidget pulse dot 移除 + 新 indicator 渲染 + 對應 scenario test
3. **真實 CDP smoke**（per `project_cdp_smoke_webview2_pitfalls` 5 雷）：
   - **Claude provider**：開 vault → 跑 goal → RunDetailRunning token display 顯示 placeholder（不是「0」）→ Done 後顯示完整 token
   - **Codex provider**：跑 codex goal、觀察 token display 是否 per-turn 累加（驗 Decision 3 假設）
   - **Bug 2 視 path 驗**：
     - Path a：跑 goal、hover ChatWidget pulse dot 看到 tooltip / aria-label 清楚說明 goal running
     - Path b：跑 goal、ChatWidget bubble 上 SHALL NOT 看到 pulse dot、改在新 indicator 位置看到
   - **Chat 端 regression check**：跑 chat session、ChatTokenDisplay 行為仍顯示「0 ↑」（既有 spec 行為、本 change 不動）
   - 截圖存 codebus-app/scripts/.pulse-and-token-smoke/
4. **Backlog doc 更新**：
   - docs/2026-05-28-four-bugs-backlog.md Bug 2 段補 archived 記號
   - docs/2026-05-28-goal-token-display-streaming-todo.md 文件本身標 archived
5. **Spec 對齊**：`app-workspace` spec `Run Detail Views — Running` 段加 token display placeholder NOTE + scenario；Bug 2 path b 則 ODI-4 段重寫

### Scope boundaries

**In scope**:

- RunDetailRunning token display 條件渲染（A 路徑必做）
- RunDetailRunning test 加 placeholder / 第一 Usage event 切換 test
- codebus-app/src/i18n/messages.ts 新增 `workspace.runDetail.tokensRunningPlaceholder`（en + zh）
- Bug 2 視 path：可能動 ChatWidget / ChatWidget test / Workspace 元件（path b 才動）/ messages.ts 相關 key
- `app-workspace` spec：`Run Detail Views — Running` 加 placeholder NOTE + scenario；視 path 動 ODI-4 段
- Archive 階段 update 兩 backlog doc

**Out of scope**:

- ChatTokenDisplay / useChatStore.tokensTotal 任何改動
- Stream parser / StreamEvent schema / Claude CLI / codex CLI 任何行為改動
- Estimated tokens 邏輯（D 路徑）
- Claude CLI incremental usage flag 研究（B 路徑、follow-up）
- ChatWidget bubble 三 modes / RunDetailRunning 其他 layout 改動
- RunDetailInterrupted / RunDetailCancelled token display 行為（既有 spec、不動）
- Chat session in-progress 跟 goal running 兩 state 的 wire 關係改動

## Risks / Trade-offs

- **[Risk] Apply Task 1 disambiguation 拖長**（user 對 path 選擇猶豫 / 三 path 都不滿意）→ Mitigation：propose 階段已 expose narrative drift（backlog doc vs user prompt vs ground truth）、disambiguation 表三欄齊全、user 可三選一或提 path d；若 path d 出現、apply 階段先 chat 對齊再評估是否擴 scope
- **[Risk] Path b 工時暴增（搬 indicator 涉及 layout + spec rewrite）**→ Mitigation：propose 已列工時 90-150 min、apply 階段若實際超 180 min stop 找 user 對齊
- **[Risk] Codex per-turn Usage verify 結果為「否」**（codex `turn.completed` 不是 per-turn 帶 Usage、或 emit 行為跟 Claude 一致）→ Mitigation：本 change A 路徑邏輯不依賴 codex 行為、verify 結果為「否」只影響 follow-up doc 內容、不影響本 change 收尾
- **[Risk] i18n key 命名 collide 既有 key**（如 `workspace.runDetail.tokensRunningPlaceholder` 已存在）→ Mitigation：apply Task 動工前 grep `codebus-app/src/i18n/messages.ts` 確認、調整命名（per `feedback_spectra_propose_grep_naming_first`）
- **[Risk] Bug 2 path b 動 Workspace layout 破壞既有 BottomStrip / sidebar 位置**→ Mitigation：path b 落實時 CDP smoke 驗其他既有元素未位移、layout 改動 limit 在 indicator 寄生 surface 內
- **[Risk] Backlog doc archive 記號錯誤覆蓋既有內容**→ Mitigation：archive 階段先 Read 兩 doc verbatim、只加標頭「Status: archived YYYY-MM-DD」+ change 連結、不改原文
- **[Trade-off] 不做 B（Claude CLI incremental usage）+ 不做 D（estimated）**→ 接受 user 在 Claude provider running 期間始終看不到 mid-flight cost 進度（顯示 placeholder 而非數字）；理由是誠實 > 估算 + B 需研究 Claude docs（follow-up）
- **[Trade-off] Bug 2 path 留到 apply 階段選定** → 接受 propose artifact 對 Bug 2 fix shape 留條件 wording、spec 對應段需 apply 階段補完；理由是不憑印象決定 fix shape 是 user 明確要求

## Migration Plan

無 migration（純 frontend UX 調整、不動資料 schema / 後端 wire format）。

## Open Questions

1. **Apply Task 1 disambiguation 結果**：Bug 2 走 path a 還是 path b？（apply 階段 chat user 對齊）
2. **i18n key 命名**：`workspace.runDetail.tokensRunningPlaceholder` 是否 collide 既有 key？（apply 動工前 grep verify）
3. **Codex per-turn Usage 行為**：`turn.completed` 是否 per-turn 帶 Usage 並累加？（apply CDP smoke verify）
4. **Path b 落實時新 indicator 位置**：Goals tab nav icon / BottomStrip / Workspace 側欄 哪個 surface？（apply 階段 chat user 對齊或 propose path b 時補設計）
