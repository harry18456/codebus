<!--
Each task description states the behavior/contract delivered AND the verification
target. File paths are supporting context, never the task itself.
Cross-reference targets:
- spec Requirement: "Run Detail Views — Running"
- design Decisions 1–5、Behavior、Interface / data shape、Failure modes、Acceptance criteria、Scope boundaries、Pre-apply 校準、詞彙 disambiguation
-->

## 1. Pre-apply 校準 + Bug 2 disambiguation

- [x] 1.1 完成 **Pre-apply 校準（grep + Read spec ground truth、不憑印象）**（守住 **為何兩個 sub-feature 綁同 change** 的 batch 範圍假設、避免 scope 漂移）：在 codebus-app/src/components/workspace/ChatWidget.tsx 與 codebus-app/src/components/workspace/RunDetailRunning.tsx 跑一次 grep 校準 `hasActiveGoal` wire、`collectTokens` sum 邏輯、`workspace.runDetail.tokensRunningPlaceholder` 是否已有 key collide；同步 Read 5.1 archive `chatwidget-pulse-and-cancel-move/specs/app-workspace/spec.md` 的 ODI-4 段 verbatim。完成行為：grep / Read 結果寫進 apply session chat（不開新 doc），確認 `hasActiveGoal` 仍 wire 到 `useGoalsStore.activeRun`、無 cross-wire 殘留；i18n key 未 collide。驗證：grep 與 Read 完成且結果條列回報、無 cross-wire 證據。
- [x] 1.2 完成 **詞彙 disambiguation（per project_quiz_fullscreen_wizard_view_term_disambiguation）對齊**：把 design.md 詞彙 disambiguation 表（pulse / indicator / active goal running）逐欄唸給 user 確認；行為：user 對「pulse 觸發點到底感覺錯在哪」三選項（i / ii / iii）給出明確選擇。驗證：apply session chat 內有 user 文字回覆指明 (i)（pulse 同時被 chat 觸發、需找 cross-wire）/ (ii)（path b 搬 indicator）/ (iii)（path a 加 clarity）其中一個或新 path d。
- [x] 1.3 完成 **Decision 1 · Bug 2 fix path apply 階段 disambiguation 後選定**：依 1.2 user 選擇定 path a / path b / path d；行為：apply session chat 內把選定 path 與工時上限（path a：30–60 min；path b：90–150 min；path d：先停手對齊）寫清楚。驗證：apply session 有「本 change Bug 2 走 path X、工時上限 Y min」這條 commitment 文字。

## 2. Goal token A 路徑（**Decision 2 · Goal token running 期間顯示 placeholder（A 路徑必做）**）

- [x] 2.1 [P] 新增 i18n key `workspace.runDetail.tokensRunningPlaceholder`：行為：codebus-app/src/i18n/messages.ts 的 `messages.en` 與 `messages.zh` 兩個 bundle 同時新增該 key（en value 暫定 `—` 或英文短語、zh value 暫定 `—` 或「計算中…」、final 文案 apply 階段確認）。驗證：`pnpm tsc` 仍綠（兩 bundle 鍵集合對齊由 TS keyof 守住）+ 既有 `chat.test.ts` / messages locale test 仍綠。
- [x] 2.2 RunDetailRunning token slot 條件渲染（**Run Detail Views — Running** requirement + **Behavior（user-observable）** + **Interface / data shape** 落地）：行為：當 `outcome === "running"` 且尚未收到任何 `StreamEvent::Usage`（accumulated sum === 0 且未曾顯示過非零值）時，metadata 行 token slot SHALL 顯示 `workspace.runDetail.tokensRunningPlaceholder` 的本地化值、不渲染字面 `0`；收到第一個 Usage event 後切換為實際累積整數值；後續不回退 placeholder。實作落點：codebus-app/src/components/workspace/RunDetailRunning.tsx 的 `collectTokens` 呼叫處與 token slot JSX。驗證：手動跑 `pnpm dev` 觀察 Workspace running 視圖、Done 後仍顯示完整 token。
- [x] 2.3 [P] RunDetailRunning test 新增 placeholder 與切換 case（**Acceptance criteria** 落地）：行為：在 codebus-app/src/components/workspace/RunDetailRunning.test.tsx 新增至少兩個 test case —— (a) running + 無 Usage event → token slot 不含 `0`、含 placeholder 文字 / testid；(b) running + 一筆 Usage event (input=120, output=80) → token slot 渲染 `200`。驗證：`pnpm test codebus-app/src/components/workspace/RunDetailRunning.test.tsx` 通過、含新加入 case。
- [x] 2.4 [P] RunDetailDone regression test 確認（**Scope boundaries** in-scope 守邊界）：行為：codebus-app/src/components/workspace/RunDetailDone.test.tsx 既有 token 顯示行為（用 RunLog summary）不受本 change 影響。驗證：`pnpm test codebus-app/src/components/workspace/RunDetailDone.test.tsx` 既有 case 全綠、無新增 / 無 regression。

## 3. Bug 2 fix path 落實（依 1.3 選定 path）

- [x] 3.1 ~~Path a 落實 —— 加 clarity~~ **SKIPPED（apply Task 1.3 選定 path b、path a 不適用）**：原任務描述：行為：ChatWidget collapsed bubble pulse dot 出現時、user 透過 hover tooltip 與 aria-label SHALL 看到/聽到清楚「Active goal running」語意（aria-label 既有 key `chat.widget.aria.openChatWithActiveGoalRunning` value 強化；可能新增 `title` 屬性在 pulse dot 元素上）。實作落點：codebus-app/src/components/workspace/ChatWidget.tsx 的 pulse dot `<span>` 與 collapsed `<button>` aria-label 處、codebus-app/src/i18n/messages.ts 對應 key value。驗證：codebus-app/src/components/workspace/ChatWidget.test.tsx 加 case 驗 `title` 或加強後 aria-label substring 存在、`pnpm test ChatWidget.test.tsx` 通過。
- [x] 3.2 Path b 落實 —— 搬 indicator（若 1.3 選定 path b）（**Chat Widget Layout and Two-State Toggle** + **Workspace Sidebar Nav Row Visual Contract** 兩 requirement 同步調）：行為：collapsed bubble 不再渲染 `data-testid="chat-widget-active-goal-pulse"`；新 ambient indicator 在 non-chat surface（位置由 1.3 chat user 對齊：Goals tab nav icon / BottomStrip / Workspace 側欄）SHALL 在 `useGoalsStore.activeRun !== null` 期間顯示 accent 點 / icon、null 時消失。實作落點：codebus-app/src/components/workspace/ChatWidget.tsx 移除 pulse markup、codebus-app/src/components/workspace/Workspace.tsx 或選定 surface 元件加新 indicator 元素 + testid + aria-label、codebus-app/src/i18n/messages.ts 對應 key。驗證：ChatWidget.test.tsx 移除既有 pulse case 並改為「pulse dot SHALL NOT render」case；新 indicator 元件加 RTL render test 驗 testid 出現/消失條件；`pnpm test` 通過。
- [x] 3.3 Path b 落實時 spec 5.1 ODI-4 段更新（**Decision 5 · Spec 改動範圍延後到 apply 階段定**）：行為：若 1.3 選 path b、在 openspec/changes/chatwidget-pulse-and-goal-token-display/specs/app-workspace/spec.md 新增一段 MODIFIED Requirements 包住「Chat Widget Layout and Two-State Toggle」requirement 全文、把 pulse dot 相關段落改寫到新 surface；若 1.3 選 path a、本 task 標 skipped 並在 apply chat 記原因。驗證：`spectra analyze chatwidget-pulse-and-goal-token-display --json` 對 spec 改動無 Critical finding；spec 行文 SHALL 與 path b 實作元件 testid 對齊。
- [x] 3.4 ~~Path a 落實後 expose narrative drift 收尾~~ **SKIPPED（apply Task 1.3 選定 path b、本任務只在 path a 條件下動工；backlog doc 的「cross-wiring confirmed」narrative 改在 task 7.1 archive 標記時順手加 footnote）**：原任務描述：行為：若 1.3 選 path a、把 docs/2026-05-28-four-bugs-backlog.md Bug 2 段「cross-wiring confirmed」表述用一行 footnote 標明「2026-05-28 disambiguation 後判定為 user-facing 語意問題、非實作 cross-wire；fix shape = path a」。驗證：grep `cross-wiring confirmed` 在 backlog doc 後可看到對應 footnote / strikethrough、不留 stale narrative。

## 4. Codex per-turn Usage verify（**Decision 3 · Codex per-turn Usage verify only（C 路徑驗證、不動程式）**）

- [x] 4.1 [P] CDP smoke 跑 codex provider goal 驗 per-turn Usage（**Behavior（user-observable）** 跨 provider 條款落地）：行為：用 codebus-app/scripts/cdp.mjs 連 WebView2 9222、開 vault、切 codex provider、跑 multi-turn goal、觀察 RunDetailRunning token slot 是否從 placeholder → 第一 turn 完後 → 第二 turn 完後逐步增加 OR 始終 placeholder 直到 Done。驗證：截圖 + 文字觀察寫入 codebus-app/scripts/.pulse-and-token-smoke/codex-per-turn.md；結果為「per-turn 累加」則 Decision 3 假設成立、無後續工；結果為「整 spawn 一次」則開 follow-up doc（task 4.2 條件動工）。
- [x] 4.2 Follow-up doc（條件：4.1 結果為「否」）：行為：在 docs/ 新增 `2026-05-28-codex-per-turn-usage-followup.md` 記錄 codex `turn.completed` 實際 Usage emit 行為、為何 per-turn 累加假設不成立、未來若要 per-turn 累積要動哪些位置（codex_parser.rs / collectTokens）；若 4.1 結果為「是」、本 task 標 skipped。驗證：doc 存在或 task 標 skipped 並記原因。

## 5. ChatTokenDisplay regression（**Decision 4 · ChatTokenDisplay 同 pattern check（不擴 scope）**）

- [x] 5.1 [P] ChatTokenDisplay 既有「0 ↑」zero state 行為 regression check：行為：本 change 完成後 ChatTokenDisplay 在 chat fresh session（無 Usage event）期間 SHALL 仍顯示 `0 ↑`（既有 `Chat Token Usage Display` spec 行為），SHALL NOT 被 Goal token placeholder 邏輯誤改。驗證：codebus-app/src/components/workspace/ChatTokenDisplay 既有 test 全綠（無新增 / 無 regression）+ CDP smoke 開 ChatWidget expanded 視覺確認 `0 ↑` 仍存在 + 截圖存 codebus-app/scripts/.pulse-and-token-smoke/chat-token-display.png。

## 6. CDP smoke 真實驗證（**Acceptance criteria** 落地、per project_cdp_smoke_webview2_pitfalls 5 雷）

- [x] 6.1 Claude provider goal CDP smoke：行為：開 vault、跑 Claude goal、RunDetailRunning token slot 整 running 期間始終顯示 `workspace.runDetail.tokensRunningPlaceholder` 本地化值（不顯示 `0`）、Done 後恢復完整 token。驗證：截圖兩張（running 時 + Done 時）存 codebus-app/scripts/.pulse-and-token-smoke/claude-{running,done}.png + 視覺確認。
- [x] 6.2 Bug 2 path 對應 CDP smoke（依 1.3 選定）：行為：path a 則 hover ChatWidget pulse dot 看到 tooltip / aria-label 清楚「Active goal running」語意；path b 則 ChatWidget bubble 上 SHALL NOT 看到 pulse dot、改在新 indicator surface 看到。驗證：截圖兩張存 codebus-app/scripts/.pulse-and-token-smoke/bug2-{before,after}.png + DevTools aria-label 屬性截圖（path a）或新 indicator testid DOM 截圖（path b）。
- [x] 6.3 [P] reduced-motion + WebView2 CSSOM fallback 驗（per project_cdp_smoke_webview2_pitfalls lesson 1）：行為：path b 落實時、新 indicator 動畫 SHALL 在 reduced-motion 下走 instant 切換；path a 落實時、既有 pulse dot fade behavior 不退化。驗證：CDP CSSOM probe 對應 transition-duration 規則、截圖存 codebus-app/scripts/.pulse-and-token-smoke/reduced-motion.png + 觀察寫進 smoke session log。

## 7. 收尾：backlog doc archive 標記、validate（**Failure modes** + **Scope boundaries** 守邊界）

- [x] 7.1 [P] docs/2026-05-28-four-bugs-backlog.md Bug 2 段標 archived：行為：在 Bug 2 段首加一行 `> **Status: archived 2026-05-28** — fix landed in openspec/changes/archive/2026-05-28-chatwidget-pulse-and-goal-token-display/`；不改既有原文。驗證：grep `archived 2026-05-28` 在該 doc 第二區段（Bug 2）出現 1 次、Bug 1 / Bug 3 仍未標 archived。
- [x] 7.2 [P] docs/2026-05-28-goal-token-display-streaming-todo.md 標 archived：行為：在文件最上方加一行 archived 標頭 + change 連結。驗證：grep `archived 2026-05-28` 在該 doc 第一行附近出現 1 次。
- [x] 7.3 `spectra validate chatwidget-pulse-and-goal-token-display` 與 `spectra analyze chatwidget-pulse-and-goal-token-display --json` 全綠：行為：所有 artifacts、tasks 完成標記、spec delta 對齊。驗證：validate exit code 0、analyze 無 Critical / Warning（Suggestion 可接受）。
