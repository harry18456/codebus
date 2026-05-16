## Context

Workspace 的 `Quiz` tab 目前只渲染 placeholder。Quiz 是 codebus 的核心差異化（author 透過自動生成測驗自我驗證對 wiki 的理解）。本 change 把 quiz 做成標準三件套（verb library + skill bundle + CLI thin wrapper），與 goal/query/fix/chat 一致。

關鍵前置事實（已由 spike 驗證，見 `docs/2026-05-15-v3-app-quiz-discussion.md`）：

- Claude CLI sandbox 是 tool-level 而非 path-level（chat-verb spike 結論）；`--tools Read,Glob,Grep` 給 read 權限但無 native path allowlist。
- Vault 結構為 `<vault>/raw/code/`（source mirror）與 `<vault>/wiki/`（wiki 內容）兩個 sibling 目錄。
- Foundation 已建立 `app.*` config namespace（`AppConfig Namespace Isolation` requirement），含 `app.quiz.pass_threshold`（50–100，default 80）與 `app.quiz.default_length`（3–10，default 5），且明文五個既有 CLI verb 不得讀 `app.*`。
- 既有 verb library pattern：library 不讀 config，caller 注入參數；CLI 是 one-shot thin wrapper（chat 例外為 REPL，quiz 不是）。

Spike 涵蓋 19 個 spawn（fixture vault `docs/spike-artifacts/quiz-fixture-vault/`），結論：scope/no-match marker 機制穩定（scope 4/4、no-match 2/2）；raw/ 存取 0/11 planning spawn（SKILL prompt-only enforce 足夠）；quiz-md schema 完整但 LLM 自編 quiz_id/topic 不可信、code fence 不一致；同 input retry 題目高度重複且 negative-context 無法根治（小 wiki 廣度上限）。

## Goals / Non-Goals

### Goals

- `codebus quiz` verb 三件套，CLI 與 GUI 共用 `run_quiz` library。
- `+ New quiz`：goal-text 輸入 → AI 規劃 wiki scope → user 確認清單 → 出題；plan 與 generate 兩 spawn 全程 live stream（reuse 既有 agent activity-stream rendering）。
- `[Quiz me on this]`（wiki preview 觸發）：跳過規劃，直接以 target page + 1-hop 作 scope。
- 出題只讀 `wiki/`，禁讀 `raw/`，以 SKILL prompt enforce。
- 每次出題產獨立 timestamped md，retry 不覆蓋舊檔。
- `quiz.default_length` 搬至共用 `quiz.*` config namespace，CLI 與 app 共讀。

### Non-Goals

- Free-text 答案 / LLM 評分（v1 評分一律 client-side 比對 `Answer` 欄位）。
- Retry 題目多樣性保證（product 決定：retry = 純 re-spawn，接受隨機，可能新可能舊）。
- Library-level 的 raw/ tool_use hook（spike 證實 prompt-only 足夠；僅在 spec 留 fallback note）。
- `pass_threshold` 搬出 `app.*`（CLI quiz 無 pass/fail 畫面，留 app-only）。
- Quiz history 排序樣式、刪除按鈕、多 attempt 收合 UI 的最終視覺定稿（屬 app-workspace 細節，spec 定行為不定像素）。
- Cross-vault quiz、間隔重複、歷史圖表（roadmap out-of-scope）。

## Decisions

### D1：Two-spawn（plan + generate），非 single-spawn agentic

`+ New quiz` 需要「user 在規劃後、出題前確認 scope」。Claude CLI 單一 spawn 跑完即結束、無法 mid-spawn 暫停等 user。因此採兩個獨立 spawn：plan spawn emit `[CODEBUS_QUIZ_SCOPE]` 後結束，CLI/GUI 顯示清單，user 確認，再跑 generate spawn。替代方案（single-spawn agentic）被否決：無法插入 confirm gate。`[Quiz me on this]` 路徑 target page 已知，跳過 plan spawn，直接 generate。

### D2：run_quiz 是 one-shot library，question_count caller-injected

`run_quiz(repo, QuizOptions { scope, question_count }, on_event, cancel)` 回傳 `Result<QuizReport, VerbError>`。`QuizScope` 為 `Page { target }` 或 `Goal { text }`。Library 不讀 config（對齊既有 verb library pattern）。`question_count` 由 caller 注入：app caller 讀共用 `quiz.default_length`，CLI caller 用 `--count` flag（未給則讀 `quiz.default_length`）。`VerbLifecycleEvent` 新增 `QuizScopePlanned { pages }` 與 `QuizNoMatch { reason }` variant（既有 module doc 明示 MAY be extended），讓 CLI/GUI 在 plan spawn stream 中即時收到 scope。

### D3：raw/ scope enforce = SKILL prompt-only

Spike（11 planning spawn 含刻意誘導 raw 的 prompt）0 次 raw/ tool_use，agent 主動 refuse 並 redirect 至 wiki。決定不實作 library tool_use hook（降為 spec fallback note：若量產發現不穩再加）。SKILL `codebus-quiz` 明訂 read scope 僅 `wiki/`、禁 `raw/`/`log/`/cwd 外路徑，並要求若被要求讀 raw 則 emit `[CODEBUS_QUIZ_VIOLATION] <path>` 並停止。

### D4：Quiz md 由 caller 後處理 frontmatter

Spike 顯示 LLM 自編 quiz_id（與真實時間無關）、topic 永遠空、generation_token_usage 永遠 0，且部分輸出被 markdown code fence 包裹。決定：SKILL 規定 agent 只產 `## Q<i>.` / `## Answer:` / `## Explanation:` 與 `planned_pages` 結構，不產 quiz_id/topic/generation_token_usage；caller（CLI/library）落檔時注入真實 quiz_id（timestamp）、topic（plan 來源或 target page）、trigger（`ai_planned` 或 `wiki_preview`）、generation_token_usage（由 QuizReport 帶出）、events_log（events.jsonl 路徑）。SKILL 明文禁止用 code fence 包裹整份輸出；caller parser 仍須 tolerant strip 前後 fence 以防偶發。

### D5：Retry = 純 re-spawn，接受隨機

Spike + negative-context mini-spike 證實：同 input retry 題目高度重複，注入前次 stems 只能避開指定清單但多 retry 仍互相收斂，根因是小 wiki 可考概念總量上限（非 prompt 技巧可解）。Product 決定 retry 不做 diversity 處理，純 re-spawn `run_quiz`（同 scope）。`run_quiz` 因此不需 previous_question_stems 參數，SKILL 不需 negative-context 段。spec 須明寫 retry 不保證新題的 UX 期望（不可對 user 宣稱每次全新）。

### D6：Config migration — default_length 搬至共用 quiz 命名空間

Foundation 把 quiz length 放 `app.*` 基於「quiz 是 app-only」假設；quiz 三件套確立後 `codebus quiz` CLI 是真實 use case，假設失效。新增 `codebus-core` 的共用 `quiz.*` namespace（schema：`quiz.default_length` int 3–10 default 5，含 validation 與 forward-compat default），對齊 `lint.*`/`pii.*`/`log.*`/`claude_code.*` pattern。`app-shell` 的 `AppConfig Namespace Isolation` requirement 被 supersede：`app.*` 僅保留 `app.quiz.pass_threshold`。SettingsModal 的 Default quiz length slider 仍在 app UI，但寫入 key 改為共用 `quiz.default_length`。`pass_threshold` 不搬。

### D7：Storage — 每 attempt 獨立 timestamped md

`<vault>/.codebus/quiz/<slug>/<ISO-timestamp>.md`，`<slug>` 為 wiki_preview trigger 的 page slug 或 ai_planned trigger 的 topic slug。每次出題（含 retry）寫新檔，永不覆蓋。檔內為 frontmatter 加 N 題加每題 answer/explanation 合一。Quiz history 由檔系統掃描建立（不靠 RunLog correlate）。

### D8：測試分層對齊 codebase 慣例（apply 階段發現並收斂）

Apply task 2.3 動工時發現：`codebus-core` 既有 verb 慣例（`goal.rs:465`、`chat.rs` 全部測試、`claude_cli.rs:583` 註解明示）是 **core 層只做 unit test**（marker 解析器 / 簽章 / vault precondition shape），**mock_claude spawn happy-path 整合測試一律在 CLI 層** `codebus-cli/tests/*_flow.rs`。chat verb（最接近的 two-spawn 前例）core 測試零 spawn 整合測試。

決策（user 確認）：quiz 對齊此既有架構，不破例。

- **task 2.3（`codebus-core`）**：unit test 覆蓋 scope-marker 解析、no-match-marker 解析、fence strip、Page-vs-Goal 分支決策、vault-missing precondition（對齊 `chat.rs::extract_promote_suggestion` unit-test 模式）。解析函式設計為 `pub(crate)` 純函式以可單測。
- **task 4.2（`codebus-cli`）**：新增 `codebus-cli/tests/quiz_flow.rs`，端到端 mock_claude 整合測試 owner，覆蓋 Goal-match / Goal-no-match / fence-strip + CLI 專屬（explicit count / config fallback / no-match exit0 無檔 / 不 auto-commit）。
- **Page-scope 端到端歸屬（task 4.2 動工時收斂）**：`codebus quiz "<topic>"` 依 `cli` spec Quiz Subcommand Behavior **僅 Goal-scope**，CLI 無 Page-scope 入口（Page 是 GUI `[Quiz me on this]` 經 library `QuizScope::Page` 的路徑）。故 Page-scope 不在 `quiz_flow.rs`：其 **branch 決策**由 task 2.3 `codebus-core` unit test（`page_scope_carries_target_as_planned_page` + run_quiz Page 分支）驗證，其 **端到端 spawn** 經 library 由 GUI task 5.3（`[Quiz me on this]` → `QuizScope::Page`）驗證。

spec `quiz` 的 4 個 Two-Shot Flow scenario 仍全被驗證，僅驗證**層級**對齊 codebase 真實架構：Goal 路徑端到端在 `quiz_flow.rs`、Page 路徑 branch 在 core unit + 端到端在 GUI library 路徑（與 27+ 既有 goal 測試、全部 chat 測試同模式）。此為 apply-time artifact 收斂，非 scope 變更。

### D9：`run_quiz` 拆 `run_quiz_plan` + `run_quiz_generate`（task 5.2 動工時收斂）

D1/D2 原本假設單一 `run_quiz(QuizScope, QuizOptions)` 內部連續跑 plan→generate。task 5.2 GUI 動工時發現：`app-workspace` Quiz Tab Plan-Confirm-Generate Flow + D1 明訂 GUI confirm gate（plan 出 scope → user 確認/改 → 才 generate），而單一連續 call **無法 mid-flight 暫停等非同步 GUI 確認**（chat 的 IPC 範本也是 per-turn 分離 call，quiz 卻把 plan/generate 綁進單 call）。

決策（user 確認，取「程式整體健康」最正解）：`run_quiz` 拆為兩個可獨立呼叫的函式：

- `run_quiz_plan(QuizPlanOptions{topic})` → `QuizPlanReport{ outcome: QuizPlanOutcome::{Scope|NoMatch}, ... }`；只跑 plan spawn，不 persist RunLog。
- `run_quiz_generate(QuizGenerateOptions{pages, question_count})` → `QuizReport{..., events_log}`；只跑 generate spawn，persist RunLog + events.jsonl。

Goal flow = `run_quiz_plan` → (Scope 時) caller 插 confirm（CLI 無 gate 直接續；GUI 等 user）→ `run_quiz_generate`。Page flow = 直接 `run_quiz_generate(pages=[target])`。`QuizScope`/`QuizOptions` 移除。

連帶收斂（已落實）：`verb-library` spec「each sub-module exports exactly one orchestration function」改為 quiz 是**唯一例外**（兩 fn），理由寫進 `verb-library` spec delta；`quiz` spec「Quiz Verb Library Function」→「Functions」（兩 fn 簽章）；`cli` commands/quiz.rs caller 改 plan→generate 兩段；core unit / verb_library_surface / quiz_flow 全數重驗綠。此為 apply-time 架構收斂以滿足 D1 confirm-gate 硬需求，非 scope 變更。

## Implementation Contract

### codebus quiz CLI

- 註冊為第八個 subcommand。Plan 模式：`codebus quiz "<topic>"`；wiki-preview 等價路徑由 GUI 走 library 不經 CLI。`--count <N>` flag override 共用 `quiz.default_length`；未給時讀該 config，config 缺失則 default 5。
- Sandbox flags 與既有 read-only verb 一致：`--tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits`。
- Read-only：不 auto-commit。Exit code：成功 0；no-match 視為成功路徑（0）並把 `[CODEBUS_QUIZ_NO_MATCH] <reason>` 印給使用者；spawn 失敗或 VerbError 非 0。
- CLI 落檔：plan 在 CLI 為印出 scope 清單後直接續跑 generate（CLI 無互動 confirm gate；confirm gate 是 GUI-only UX），落檔注入 frontmatter（見 D4）。

### run_quiz library

- 簽章如 D2。`on_event` 依序 emit：plan spawn 期間的 `VerbEvent`（thinking/tool_use）、`QuizScopePlanned { pages }` 或 `QuizNoMatch { reason }`、generate spawn 期間的 `VerbEvent`、最終 `QuizReport`。
- `QuizReport` 至少含：quiz_md（已 strip fence、未注入 caller frontmatter 的 raw 題目區塊）、planned_pages、token_usage、started_at/finished_at、agent_exit_code。
- `QuizScope::Goal` 跑 plan 加 generate 兩 spawn；`QuizScope::Page` 跳 plan、直接以 target 加 1-hop 組 generate prompt。
- raw/ 不做 library enforce（D3）；cancel 經既有 `Option<Arc<AtomicBool>>`。

### codebus-quiz SKILL bundle

- 部署於 `<repo>/.codebus/.claude/skills/codebus-quiz/SKILL.md`（CLI-spawn discovery）與 `<repo>/.claude/skills/codebus-quiz/SKILL.md`（user-direct discovery），write-if-missing 保留 user 客製。
- 內容契約：read scope 僅 `wiki/`；兩 mode（`plan:` 與 `generate:`）；`[CODEBUS_QUIZ_SCOPE] <wiki path>, ...`（first line，2–5 pages，最相關者首位）；無對應頁時 `[CODEBUS_QUIZ_NO_MATCH] <reason>`；raw/ 被要求時 `[CODEBUS_QUIZ_VIOLATION] <path>`；generate 輸出 `## Q<i>.` 加 4 choices `A)`–`D)` 加 `## Answer: X` 加 `## Explanation:`（引 `[[slug]]`）；禁 code fence 包整份；marker 與結構恆英文，題幹/選項/解釋隨 wiki 頁語言（Language Override）；agent 不產 quiz_id/topic/generation_token_usage。

### GUI Quiz flow（app-workspace）

- `+ New quiz` 進 topic input，跑 plan spawn（live stream），收到 `QuizScopePlanned` 顯示 wiki 清單加 `[改]`/`[確認]`，確認後跑 generate spawn（live stream），進一題一畫面（A–D 選項、選後 reveal 正解加 explanation、錯題顯示 `[← Back to wiki page]`），到 summary（分數加通過與否依 `app.quiz.pass_threshold` client-side 判定），到 history。
- `QuizNoMatch` 顯示 reason，不進出題。
- `[Quiz me on this]`（wiki preview footer，僅內容頁顯示，`index.md`/`log.md` 不顯示）跳過規劃直接 generate flow。
- Quiz history（sidebar Quiz tab）：依 page/topic group 列 attempt，每列 `[看過程]` 開該 attempt 的 events.jsonl timeline。

### Scope boundaries

- 在 scope：quiz verb/library/SKILL/CLI、GUI quiz flow 兩觸發點、`quiz.*` config namespace 與 app-shell supersede、events.jsonl 落地（reuse 既有）、storage layout。
- 不在 scope：events timeline component 本身的新實作（reuse agent-stream-rendering 既有）、retry 多樣性、free-text 評分、history 視覺最終稿、cross-vault。

## Risks / Trade-offs

- [小 wiki retry 題目重複] → 已知且接受（D5）；spec 明寫 UX 期望，不誤導 user。
- [LLM 偶發 code fence 包裹輸出] → SKILL 明文禁止加 caller parser tolerant strip 雙層。
- [SKILL prompt-only raw enforce 在量產更大 vault 可能鬆動] → spec 留 library tool_use hook 作 fallback note；events.jsonl 可事後稽核 tool_use path。
- [config migration 破壞既有 app.quiz.default_length] → 見 Migration Plan；採讀舊值一次性遷移避免使用者設定遺失。
- [動 archived foundation spec（app-shell）] → 由本 change 顯式 own，spec delta 以 supersede 方式處理、不靜默改寫；app-shell scenario 中 pass_threshold 斷言保留、default_length 斷言移至新 quiz capability。

## Migration Plan

1. 新增 `codebus-core` 共用 `quiz.*` config schema（含 validation 加 missing→default 5）。
2. `codebus-app` tauri config 讀取改為：先讀共用 `quiz.default_length`；若不存在但舊 `app.quiz.default_length` 存在，採舊值並於下次 save 寫入新 key（one-time 遷移，保留使用者既有設定）；`app.*` 移除 default_length，保留 pass_threshold。
3. SettingsModal Default quiz length slider 綁定新 key。
4. `app-shell` spec delta supersede `AppConfig Namespace Isolation`；新 `quiz` capability 承接 default_length 行為與其 scenario。
5. 無 rollback DB 概念；回退方式為還原 config 讀取邏輯（純程式碼回退，使用者 config.yaml 因 one-time 遷移已含新 key，向後相容）。

## Open Questions

- Quiz history list 排序（page 字母序 / 最近 attempt 時間倒序 / 全拍平時間倒序）屬 app-workspace 視覺細節，apply 時依既有 Goals/Wiki tab 慣例對齊，不阻塞。
- Topic slug 正規化規則（中文 topic 轉 slug）apply 時定，建議 hash 後綴避免碰撞。
- `[看過程]` 呈現（modal / inline expand / 專屬 view）依 agent-stream-rendering 既有 component 能力決定，apply 第一步 inspect 後定。
