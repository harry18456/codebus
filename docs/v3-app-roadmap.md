# codebus-app v1 Roadmap

CLI 主線（`docs/v3-roadmap.md`）2026-05-10 全 ship 後，app 層 v1 切成 8 條序列化 change（foundation + A + B + chat + C + D + E + F；foundation 已 archive、A `v3-goal-library` 已 archive 2026-05-13）。每一條都假設前一條已 archive；不是平行可換序。

> **2026-05-12 update**：原本 #3 `v3-app-quiz-cmdk` 把 Quiz 跟 Cmd+K query 捆一起。實機進入 #2 設計階段時討論發現 Cmd+K query 跟 #2 的 goal-stream 基建本質一樣（都 spawn codebus verb + 接 stream-json + render thought / tool calls / result），讓 query 緊跟 goal、把 Quiz 切到後一條 — (a) 兩條都更聚焦、(b) Cmd+K query 早 land 給 user 一個立即可用的問答 UI、(c) Quiz 可重用 cmdk 的 stream + citation 基建。Stage A 額外 ship 的 `stage-b-app-endpoint-settings` 也算 #1 之後的 Settings 補完，沒列在主序列裡（屬於 foundation 的 follow-up patch）。
>
> **2026-05-12 update (2)**：`v3-app-workspace-goal` 動工前 spectra-discuss 發現倚賴的 CLI 側基建有 2 個未做的洞 — (1) `codebus_core::agent::invoke()` stream render 跟 invoke 綁死沒 callback hook，GUI 無法 reuse；(2) run-log 只存 summary、stream events 沒持久化，Goals overview list / completed goal timeline / cancel UX 都缺資料來源。必須先以兩條獨立 prerequisite change 補完再做 GUI。**5 條序列 → 6 條**：最前面插 A `v3-goal-library` + B `v3-run-log-events`，原本 #2-#5 變 C-F。完整討論結論 / Q1/Q2/Q3 trade-offs / cancel & interrupted UX 設計見 `docs/2026-05-12-v3-app-workspace-goal-discussion.md`。
>
> **2026-05-13 update (3)**：B propose 前 user push back §4.6 Cmd+K 「soft single-shot mode」— 想做 multi-turn chat + agent 提示 promote 成 goal。Claude CLI sandbox 是 spawn-time hard gate（chat 不能 mid-session 切寫 wiki），所以新增 verb `chat` 跟 query / goal 並存。**6 條序列 → 8 條**：B 跟 C 之間插 `v3-chat-verb`；D 從 `v3-app-query-cmdk` 改名 `v3-app-chat-cmdk` + scope 改成 multi-turn。B 與 chat 正交、不推遲。完整分析（Claude CLI 三項硬限制 / per-turn library 設計 / promote 機制 / spike 風險 / 動工順序）見 `docs/2026-05-13-chat-verb-discussion.md`。

## Sequence

| # | Change | Scope (one line) | Depends on |
|---|---|---|---|
| 1 | `v3-app-foundation` | Tauri shell + IPC bridge（5 commands） + Lobby（populated + empty） + Settings modal（7 fields） + Workspace stub + design system foundation（Tailwind v4 token / shadcn primitives） | — |
| A | `v3-goal-library` | 3 個 spawn verb（goal / query / fix）orchestration 搬進 `codebus_core::verb::*`；`agent::invoke()` 加 `on_event` callback 與 `Option<Arc<AtomicBool>>` cancel signal；`run_goal` / `run_query` / `run_fix` 同樣接 callback + cancel；CLI 三個 commands 變 thin wrapper byte-equivalent（鏡像 foundation 的 `init::run_init` pattern）。lint 已 library 不動；cancel 用 `AtomicBool` polling 不引入 tokio。 | — |
| B | `v3-run-log-events` | RunLog schema 加 `outcome`（`succeeded` / `failed` / `cancelled`）；per-run events.jsonl 持久化（`<vault>/.codebus/log/events-<started_at_slug>.jsonl`）；cancel path 寫 `outcome=cancelled` 且不 auto-commit；GUI-spawned runs 強制寫（忽略 `log.sink: none`） | A |
| chat | `v3-chat-verb` | 新 CLI verb `codebus chat`（multi-turn read-only REPL）+ `codebus_core::verb::chat::run_chat_turn` library + `codebus-chat/SKILL.md` bundle + RunLog 加 `session_id: Option<String>`；CLI REPL 累積 transcript，`/goal "..."` in-REPL command 重 spawn `codebus goal` 帶 transcript 當 context；spawn-time sandbox 鎖 Read/Glob/Grep（mid-session 切寫 wiki 不可行）；先 spike claude `--continue` / session_id / sandbox 互動。Pattern 與 spike 細節見 `docs/2026-05-13-chat-verb-discussion.md` | B |
| C | `v3-app-workspace-goal` | Vault Workspace 真內容：sidebar Goals/Wiki/Quiz tabs + Wiki preview (Milkdown) + Goal flow（modal + inline mini-stream + running / done / cancelled / interrupted detail view 含 `[Retry with same goal]`） | foundation + A + B |
| D | `v3-app-chat-cmdk` | Cmd+K spotlight chat 抽屜（multi-turn + streaming + 引用 + `[Promote to goal]` 按鈕）— 重用 chat 的 `run_chat_turn` + C 的 stream rendering pipeline + spotlight UX；翻轉 design doc §4.6.3 原 「soft single-shot」決定 | C + chat |
| E | `v3-app-quiz` | Quiz flow（pending / reviewing 兩態 + md 持久化） + 從 wiki page 觸發 quiz / 答題評分 / 結果寫回 md frontmatter | D |
| F | `v3-app-polish-ship` | Release build / installer / auto-update / icon 視覺再優化 / E2E test infra / **跨平台驗證（含 foundation / A / B / C / D / E 各自 acceptance checklist 在 macOS / Linux 重跑）** | A-E 全 ship |

序列的 「依賴」一欄列的是該 change **行為層** 必須先存在的東西；artifact 層每條 change 都各自 own 一份 spec / design / tasks。

## Cross-platform policy

開發階段一律以 **Windows MSVC** 為主，每條 change 的 acceptance checklist 只在 Windows 上必跑必過。macOS / Linux 的手動回歸驗證集中到最後一條 change（`v3-app-polish-ship`）一次掃完，作為 release gate 的一部分。

理由：
1. 主要開發機是 Windows，每條 change 都要求三平台驗證 dev velocity 損失過大
2. 跨平台 build artifact / installer 本來就排在 polish-ship，順手把手動驗收一起做才不會驗兩次
3. polish-ship 才會建 E2E test infra，到時候 cross-platform 也可能變部分自動，與其在每條 change 重複 manual 驗證不如等基建好

各 change 的 tasks.md 在 §13 不另列 macOS / Linux acceptance 條目（如 `v3-app-foundation` 13.2 已改為「在 roadmap 登記 deferral」的 documentation 任務）；polish-ship 屆時負責統整。

### Deferred acceptance registry

各 change 在此登記其延後到 `v3-app-polish-ship` 的 macOS / Linux 手動驗收範圍：

- **`v3-app-quiz` (E)** — macOS / Linux 手動驗收 deferred to `v3-app-polish-ship`。polish-ship 屆時需在 macOS + Linux 重跑：(1) CLI `codebus quiz "<topic>"` 端到端（plan→generate→落檔 `<vault>/.codebus/quiz/<slug>/<id>.md` 含 caller frontmatter；no-match exit 0 不落檔；retry 非破壞兩檔）；(2) GUI Quiz tab plan-confirm-generate flow（topic 輸入→plan live stream→scope 確認 gate→generate→一題一畫面 client-side 評分→summary pass/fail by `app.quiz.pass_threshold`）；(3) wiki preview `[Quiz me on this]` Page flow（nav 頁不顯示、內容頁跳 plan 直接 generate）；(4) Quiz history（掃 `.codebus/quiz/` 依 slug group、retry 兩 row、`[看過程]` events.jsonl）；(5) 共用 `quiz.default_length` config 與 `app.*` namespace isolation（CLI 不讀 app.*）。Windows MSVC 上述皆已於本 change 必跑必過（Rust core / CLI / Tauri / vitest 全綠）。

- **`fix-app-quiz`** — `v3-app-quiz` archive 後的 Windows 人工驗收補做 + compliance 修正容器。實況（誠實登記，取代 v3-app-quiz「Windows 皆已必跑必過」的過度宣稱）：
  - **CLI 區塊**：由 assistant 端到端實跑驗證（真 claude spawn，throwaway vault `quiz-e2e`）—— plan→generate→落檔 caller frontmatter、no-match exit0 不落檔、retry 非破壞兩檔、`--count`/`quiz.default_length` fallback、不 auto-commit、`events_log` 絕對路徑、body 無 preamble。**Pass。**
  - **GUI 區塊**：本 change 期間以**互動式人工驗收**進行（user 實機 `cargo tauri dev`），共抓出並修復 7 個 defect（#1 header 碰撞 / #2 +New quiz 無反應 / #3 plan-marker 過脆+不可診斷 / #4 generate preamble 漏檔 / #5 plan/generate 未 live render / #6 view-log 改 attempt-modal / #7 +New quiz 進 quiz 內隱藏），全部 TDD 修正並有自動測試覆蓋（core 452 / cli 123 / vitest 353 全綠）。
  - **Deferred (a) — RESOLVED 2026-05-19**：完整 GUI checklist 由 user 實機 `cargo tauri dev` 跑過合併 sweep（quiz-attempt-progress + fix-quiz-ux-wiring 的 redesign 驗收一併做，見下方 `quiz-attempt-progress` / `fix-quiz-ux-wiring` 兩條 GUI 區塊）。**Windows Pass。** 原延後理由（即將被 redesign 取代）已不成立——redesign 已 ship 且本次 sweep 即針對 redesign 後的 UI。
  - **Deferred (b)**：macOS / Linux 手動驗收仍 deferred to `v3-app-polish-ship`（沿用上面 v3-app-quiz (E) 五區塊範圍，含 fix-app-quiz 的修正）。

- **`quiz-attempt-progress`** — `v3-app-quiz` / `fix-app-quiz` 之後的 quiz 進度持久化 redesign（不可變 attempt md + sibling `<id>.progress.json` sidecar；history 徽章/路由；completed→QuizReview 取代 raw md；`重做此份`）。實況（誠實登記）：
  - **自動測試範圍（Windows MSVC，本 change 必跑必過）**：core sidecar 容錯讀+atomic write 單元（`quiz_progress.rs` 5 案：缺檔/壞檔/round-trip/未知 key+新 schema_version/atomic 覆寫無 .tmp 殘留）；Tauri `read_quiz_progress`/`write_quiz_progress` containment + round-trip + registry 23→25；vitest QuizAnswering 每題持久化+resume、QuizTab history 徽章衍生/狀態路由/`重做此份` 不 spawn、QuizReview 逐題 user-choice vs 正解+解釋+看過程 modal。彙總 0 failed：`cargo test -p codebus-core -p codebus-cli`、`cargo test`（tauri）、`npx vitest run`（361 passed）、`npm run typecheck`（乾淨）。
  - **GUI 互動驗收 — Pass 2026-05-19**：user 實機 `cargo tauri dev`（throwaway vault `quiz-e2e`，完整重啟）跑過整輪互動 sweep，**全 pass**：答題中途「← History」離開非破壞 → history 點回 attempt **接續在未答題**（題目不變、已答保留）→ 答完 → completed 點開進 **QuizReview**（逐題 user-choice vs 正解 + 解釋，非 raw md）→ 解釋 `[[wikilink]]` 可點跳 wiki → 「看過程」開 generation log modal → 「重做此份」同題重答**不 spawn**、非破壞。與 fix-quiz-ux-wiring 合併於同一 sweep（見下條）。
  - **macOS / Linux**：手動驗收 deferred to `v3-app-polish-ship`。特別項：sidecar atomic write 的 `fs::rename` 覆寫語意在 Windows 已有測試覆蓋，macOS/Linux 需於 polish-ship 一併實機確認（沿用 v3-app-quiz (E) 五區塊範圍 + 本 change 的 sidecar/Review/resume 行為）。

- **`fix-quiz-ux-wiring`** — `quiz-attempt-progress` 之後修 5 項 v3-app-quiz / fix-app-quiz 既有缺口（D1 答題/summary 返回鈕、D2 已 active Quiz 分頁再點回 history、D3 啟動載入 config 不需開 Settings、D4 出題數接 shared/legacy `quiz.default_length` clamp 3..10、D5 plan-marker 行內前言容忍）。實況（誠實登記）：
  - **自動測試（Windows MSVC，本 change 必跑必過）**：彙總 0 failed —— `cargo test -p codebus-core -p codebus-cli`、`cargo test`（tauri manifest，順手補一行 archived `quiz-attempt-progress` 漏掉的 `cursor: None,` 測試 initializer 才能編譯）、`npx vitest run`（380 passed）、`npm run typecheck`（乾淨）。commit `685f78f` 實作 + `3c5a9c8` archive（2026-05-19）。
  - **GUI 互動驗收 — Pass 2026-05-19**：user 實機 `cargo tauri dev`（vault `quiz-e2e`，config `app.quiz.default_length:10`/`pass_threshold:75`，全程不開 Settings），合併 sweep Journey A–D **全 pass**：D4 新 quiz 出 **10 題**（非 5）；D1 答題中＋summary 皆有「← History」且非破壞、不 spawn；D2 已 active Quiz 分頁再點回 history、進行中不破壞；D3 答對 8/10=80% summary 顯示**通過**（75 門檻、未開 Settings 即生效）；D5 happy（JWT 題目 → SCOPE）＋ no-match（酸種麵包 → 顯示理由不落檔）；wiki page flow `[Quiz me on this]` 內容頁可觸發、nav 頁隱藏未回歸。
  - **macOS / Linux**：手動驗收 deferred to `v3-app-polish-ship`（沿用 v3-app-quiz (E) 五區塊 + 本 change 的 D1–D5 行為）。

## 為什麼切 8 條而不是一條

7-8 週工作量。單一巨大 change 的歷史教訓：apply 失焦、review 不可行、in-flight spec drift。本 roadmap 的切點來自 2026-05-11 brainstorming session（原本 4 條 / 2026-05-12 把 quiz-cmdk 拆成 query-cmdk + quiz 兩條 / 2026-05-12 #2 動工前發現 CLI 缺基建再前插 A + B 兩條 / 2026-05-13 B propose 前 user push back single-shot query 再前插 chat 一條，總計 8 條），每一條落點都是「換到下一條時，前一條跑得起來的 demo」（不是「實作了某個檔案」），所以 archive 任一條後都可以對外展示一個可用的 app 子集。

A / B / chat 三條 CLI-side prerequisite 雖然不直接 ship GUI 功能，但都是「換到 C / D 時前置基建可用」的 demo：A archive 後 `codebus goal "..."` CLI 行為 byte-equivalent（refactor 不破舊行為）；B archive 後 CLI user 多看到 events.jsonl 與 RunLog outcome 欄位（GUI 還沒做 UI 但 CLI 已能驗 events 串流到磁碟）；chat archive 後 CLI user 可以跑 `codebus chat` REPL 探索 + `/goal` promote（GUI 還沒做 Cmd+K 但 CLI 已驗證 multi-turn 機制 + SKILL.md 已 tune 過）。

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
