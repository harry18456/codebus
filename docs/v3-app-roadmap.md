# codebus-app v1 Roadmap

CLI 主線（`docs/v3-roadmap.md`）2026-05-10 全 ship 後，app 層 v1 切成 5 條序列化 change。每一條都假設前一條已 archive；不是平行可換序。

> **2026-05-12 update**：原本 #3 `v3-app-quiz-cmdk` 把 Quiz 跟 Cmd+K query 捆一起。實機進入 #2 設計階段時討論發現 Cmd+K query 跟 #2 的 goal-stream 基建本質一樣（都 spawn codebus verb + 接 stream-json + render thought / tool calls / result），讓 query 緊跟 goal、把 Quiz 切到後一條 — (a) 兩條都更聚焦、(b) Cmd+K query 早 land 給 user 一個立即可用的問答 UI、(c) Quiz 可重用 cmdk 的 stream + citation 基建。Stage A 額外 ship 的 `stage-b-app-endpoint-settings` 也算 #1 之後的 Settings 補完，沒列在主序列裡（屬於 foundation 的 follow-up patch）。

## Sequence

| # | Change | Scope (one line) | Depends on |
|---|---|---|---|
| 1 | `v3-app-foundation` | Tauri shell + IPC bridge（5 commands） + Lobby（populated + empty） + Settings modal（7 fields） + Workspace stub + design system foundation（Tailwind v4 token / shadcn primitives） | — |
| 2 | `v3-app-workspace-goal` | Vault Workspace 真內容：sidebar Goals/Wiki/Quiz tabs + Wiki preview (Milkdown) + Goal flow（live stream + 結束狀態） | foundation 的 IPC contract / route store / Workspace stub |
| 3 | `v3-app-query-cmdk` | Cmd+K spotlight query 抽屜（streaming + 引用）— spawn `codebus query` + 重用 goal-stream 渲染管線 + spotlight UX（Ctrl/Cmd+K 喚出、搜尋框、即時 stream、引用 link 可點回 wiki preview） | workspace-goal 的 wiki page model / stream rendering pipeline |
| 4 | `v3-app-quiz` | Quiz flow（pending / reviewing 兩態 + md 持久化） + 從 wiki page 觸發 quiz / 答題評分 / 結果寫回 md frontmatter | query-cmdk 的 wiki rendering / app-state 持久化 pattern |
| 5 | `v3-app-polish-ship` | Release build / installer / auto-update / icon 視覺再優化 / E2E test infra / **跨平台驗證（含 v3-app-foundation / workspace-goal / query-cmdk / quiz 各自 acceptance checklist 在 macOS / Linux 重跑）** | 前四條都 ship |

序列的 「依賴」一欄列的是該 change **行為層** 必須先存在的東西；artifact 層每條 change 都各自 own 一份 spec / design / tasks。

## Cross-platform policy

開發階段一律以 **Windows MSVC** 為主，每條 change 的 acceptance checklist 只在 Windows 上必跑必過。macOS / Linux 的手動回歸驗證集中到最後一條 change（`v3-app-polish-ship`）一次掃完，作為 release gate 的一部分。

理由：
1. 主要開發機是 Windows，每條 change 都要求三平台驗證 dev velocity 損失過大
2. 跨平台 build artifact / installer 本來就排在 polish-ship，順手把手動驗收一起做才不會驗兩次
3. polish-ship 才會建 E2E test infra，到時候 cross-platform 也可能變部分自動，與其在每條 change 重複 manual 驗證不如等基建好

各 change 的 tasks.md 在 §13 不另列 macOS / Linux acceptance 條目（如 `v3-app-foundation` 13.2 已改為「在 roadmap 登記 deferral」的 documentation 任務）；polish-ship 屆時負責統整。

## 為什麼切 5 條而不是一條

7 週工作量。單一巨大 change 的歷史教訓：apply 失焦、review 不可行、in-flight spec drift。本 roadmap 的切點來自 2026-05-11 brainstorming session（原本 4 條 / 2026-05-12 把 quiz-cmdk 拆成 query-cmdk + quiz 兩條），每一條落點都是「換到下一條時，前一條跑得起來的 demo」（不是「實作了某個檔案」），所以 archive 任一條後都可以對外展示一個可用的 app 子集。

## Out of scope（全部 v1 範圍以外）

下列 item 在 v1 五條 change **皆不做**，未來走獨立 change 評估：

- 多 AI provider 選擇 UI（Claude CLI 是唯一選項）
- Light theme / theme toggle（hard-coded dark）
- Language switcher UI（auto-detect system locale）
- Per-vault settings override
- Quest banner / progress bar / "graduated" / "mastered" / "learned" 任何 page-level state
- Tutorial slideshow / 投影片模式 / 教學 md 生成
- Telemetry / analytics / crash reporting
- Quiz 歷史圖表 / 間隔重複（spaced repetition）
- 多 goal 並行（v1 always at most 1 running goal）
- 分享 / 匯出 / public wiki publish
