<!--
Traceability: every task names the behavior delivered, its verification target,
and the spec requirement + design decision it satisfies. Design decisions are
D1..D7 plus the "run_quiz library", "GUI Quiz flow", "Scope boundaries",
"Goals / Non-Goals" sections of design.md.
-->

## 1. 共用 quiz config namespace 與 foundation 遷移

- [x] 1.1 [P] 新增 `codebus-core` 共用 `quiz.*` config schema：`quiz.default_length`（int 3–10，default 5），含 range validation 與 missing→5 forward-compat。實現 design D6 與 spec `quiz` / Shared Quiz Config Namespace。驗證：`codebus-core` 新增單元測試覆蓋「缺鍵回 5」「越界 reject」「合法值原樣讀回」三案並通過。
- [x] 1.2 `codebus-app` tauri config 先讀共用 `quiz.default_length`；缺鍵但存在舊 `app.quiz.default_length` 時採舊值且下次 save 寫入新鍵（one-time 遷移）；`AppQuizConfig` 移除 `default_length`、保留 `pass_threshold`。實現 design D6 與 spec `app-shell` / AppConfig Namespace Isolation。驗證：tauri config 測試覆蓋「純新鍵」「僅舊鍵→遷移」「皆無→5」三案並通過。依賴 1.1。
- [x] 1.3 [P] SettingsModal 的「Default quiz length」slider 綁定共用 `quiz.default_length`，存檔 round-trip 後重開顯示同值。實現 design D6 與 spec `app-shell` / AppConfig Namespace Isolation。驗證：`SettingsModal.test` 新增 slider 寫入共用鍵 round-trip 斷言並通過。依賴 1.2。

## 2. quiz verb library（run_quiz library，design D1/D2）

- [x] 2.1 [P] `VerbLifecycleEvent` 新增 `QuizScopePlanned { pages: Vec<String> }` 與 `QuizNoMatch { reason: String }` variant；既有 variant 與 serde 行為不變。實現 design D2/run_quiz library 與 spec `quiz` / Quiz Verb Library Functions。驗證：既有 verb event 測試全綠 + 新 variant 序列化測試通過。
- [x] 2.2 定義 `codebus_core::verb::quiz` 模組與 `QuizOptions`、`QuizScope::{Page,Goal}`、`QuizReport`，維持 verb-library 五 sub-modules surface。實現 design D2 與 spec `verb-library` / Verb Library Module Surface 及 `quiz` / Quiz Verb Library Functions。驗證：downstream `use codebus_core::verb::quiz::{run_quiz,QuizOptions,QuizReport,QuizScope};` compile-smoke 測試通過。依賴 2.1。
- [x] 2.3 實作 `run_quiz`：`Goal` 跑 plan→generate 兩 spawn、`Page` 跳 plan（target + 1-hop）；解析 `[CODEBUS_QUIZ_SCOPE]`/`[CODEBUS_QUIZ_NO_MATCH]` 首行；去除偶發前後 code fence；`on_event` 依序 plan events→scope/no-match→generate events→report；cancel 用既有 `Option<Arc<AtomicBool>>`；library 不讀 config。實現 design D1（two-spawn）、D3（raw scope enforce）、D4（caller frontmatter）、D8（測試分層）與 spec `quiz` / Quiz Verb Two-Shot Flow、Quiz Read Scope Enforcement、Quiz Markdown Schema and Caller Frontmatter Injection。驗證（對齊 codebase 慣例 — core 層 unit test、端到端 spawn 在 CLI 層 task 4.2，見 design D8）：`codebus-core` unit test 覆蓋 scope-marker 解析、no-match-marker 解析、fence strip、Page-vs-Goal 分支決策、vault-missing precondition 並通過（對齊 `chat.rs` 的 `extract_promote_suggestion` unit-test 模式）。依賴 2.2。
- [x] 2.4 [P] `codebus_core::verb` parent 導出 quiz 子模組，init 仍不在 verb 下。實現 design D2 與 spec `verb-library` / Verb Library Module Surface。驗證：verb-library spec compile scenario 測試通過。依賴 2.2。

## 3. codebus-quiz SKILL bundle（design D3/D4）

- [x] 3.1 [P] 以 spike v0 為基底產出 production `codebus-quiz/SKILL.md`：wiki-only read scope、禁 raw/log/cwd 外、`plan:`/`generate:` 兩 mode、三 marker、quiz-md 結構、禁 code fence、禁 agent 產 quiz_id/topic/generation_token_usage、Language Override。實現 design D3/D4 與 spec `skill-bundles` / Quiz Skill Bundle Content。驗證：內容 review 對照 `quiz` 與 `skill-bundles` spec 逐條核對。
- [x] 3.2 init 把 `codebus-quiz` 納入 vault-internal 五 bundle（及 opt-in repo-root 五 bundle byte-identical），不建 `codebus-lint`、不寫 user-global。實現 design D3 與 spec `skill-bundles` / Skill Bundle Layout。驗證：`skill-bundles` init scenario 整合測試（default 五 bundle、--with-repo-root-skills 雙位置、byte-identical）通過。依賴 3.1。

## 4. codebus quiz CLI subcommand（design D2）

- [x] 4.1 註冊第八個 subcommand `quiz`，binary 與 per-subcommand `--help`/`--version` 可用，`codebus quiz --help` 描述產測驗並列 `--count`。實現 spec `cli` / Subcommand Registration。驗證：`cli_routing` 測試斷言八 subcommand 與 quiz help 文案並通過。
- [x] 4.2 實作 `codebus quiz "<topic>"`：呼叫 `run_quiz`（`QuizScope::Goal`）；`--count` 3–10，缺則讀 `quiz.default_length`，再缺則 5；sandbox flags `--tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits`；read-only 不 auto-commit；無互動 confirm gate（印 scope 後直接 generate）；落檔注入 caller frontmatter；no-match 印 reason、不落檔、exit 0；spawn 失敗 exit 非 0。實現 design D2/D4/D8 與 spec `cli` / Quiz Subcommand Behavior 及 `quiz` / Quiz Verb Two-Shot Flow、Quiz Markdown Schema and Caller Frontmatter Injection。驗證（此層為端到端 spawn 整合測試的 owner，見 design D8）：新增 `codebus-cli/tests/quiz_flow.rs` mock_claude 整合測試，覆蓋 explicit count、config fallback、no-match exit0 無檔、不 auto-commit，AND 端到端 Goal-match / Goal-no-match / fence-strip 並通過（Page-scope 無 CLI 入口 — branch 歸 task 2.3 core unit、端到端歸 GUI task 5.3，見 design D8）。依賴 2.3、1.1。

## 5. GUI quiz flow（design GUI Quiz flow / Scope boundaries）

- [x] 5.1 Workspace `Quiz` tab 改渲染 quiz history 列表與 `+ New quiz`，移除「Coming soon」placeholder。實現 design GUI Quiz flow 與 spec `app-workspace` / Workspace Layout and Tab Navigation。驗證：app-workspace scenario 測試（無 placeholder 文案、出現 history + new-quiz 控制）通過。
- [x] 5.2 `+ New quiz`：topic 輸入→plan spawn live stream→收 `QuizScopePlanned` 顯示 wiki 清單 + `[改]`/`[確認]`，未確認不啟 generate；`QuizNoMatch` 顯示 reason 不出題。經 Tauri IPC（`spawn_quiz_plan`/`spawn_quiz_generate`）接線。實現 design D1/GUI Quiz flow 與 spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow 及 `app-workspace` / Tauri IPC Commands for Quiz Plan and Generate Lifecycle。驗證：前端整合測試覆蓋「確認前不 generate」「no-match 不出題不落檔」並通過。依賴 5.1、2.3。
- [x] 5.3 [P] `[Quiz me on this]` 僅在 wiki 內容頁 preview 顯示（`index.md`/`log.md` 不顯示），啟動跳過 plan 直接 generate（target + 1-hop）。實現 design GUI Quiz flow 與 spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow。驗證：前端測試覆蓋「nav 頁不顯示」「內容頁走 Page scope」並通過。依賴 5.1、2.3。
- [x] 5.4 答題視圖：一題一畫面四選項，送出後 client-side 比對 `Answer`（不 spawn）顯示對錯與 `Explanation`，錯題加 `[← Back to wiki page]`；末題 summary 以 `app.quiz.pass_threshold` client-side 算 pass/fail。實現 design GUI Quiz flow 與 spec `app-workspace` / Quiz Answering and Summary。驗證：前端測試覆蓋「正解不 spawn」「錯題出現 wiki 返回」「80% threshold 4/5 pass」並通過。依賴 5.2。
- [x] 5.5 [P] Quiz history：掃 `<vault>/.codebus/quiz/` 依 slug 分組列 attempt，每列 view-generation-log 開該次 generate spawn 的 events.jsonl，點 row 開該 attempt md。實現 design D7/GUI Quiz flow 與 spec `app-workspace` / Quiz History List 及 `quiz` / Quiz Storage Layout and Retry Semantics。驗證：前端測試覆蓋「同 slug 兩次 retry 兩列各自題目」「view-log 開 events timeline」並通過。依賴 5.1。

## 6. 跨層驗證與 spec 遷移收尾（design D5/D6/Scope boundaries）

- [x] 6.1 `app-shell` 的 AppConfig Namespace Isolation supersede 落實：`app.*` 僅含 `pass_threshold`、CLI（含 quiz）不讀 `app.*`、quiz 題數走共用 namespace，無回歸。實現 design D6/Scope boundaries 與 spec `app-shell` / AppConfig Namespace Isolation。驗證：既有 app-shell namespace isolation 測試調整為新斷言且通過。依賴 1.2、4.2。
- [x] 6.2 retry 純 re-spawn 端到端驗證：同 scope 連跑兩次產兩個不覆蓋 timestamped 檔，系統不傳前次 stems、不宣稱必為新題。實現 design D5 與 spec `quiz` / Quiz Storage Layout and Retry Semantics。驗證：整合測試斷言兩檔並存、前次內容不變、第二次未注入前題記錄並通過。依賴 2.3、5.4。
- [x] 6.3 在 `docs/v3-app-roadmap.md` 登記 E `v3-app-quiz` 的 macOS/Linux 手動驗收 deferral（依 Cross-platform policy 集中到 polish-ship）。對齊 design Goals / Non-Goals 的跨平台範圍。驗證：roadmap 文件 review 確認該 deferral 條目存在且指向 polish-ship。

## Traceability

### Design decision → implementing tasks

| Design decision | Tasks |
| --- | --- |
| D1：Two-spawn（plan + generate），非 single-spawn agentic | 2.3, 5.2 |
| D2：run_quiz 是 one-shot library，question_count caller-injected | 2.1, 2.2, 2.4, 4.2 |
| D3：raw/ scope enforce = SKILL prompt-only | 2.3, 3.1, 3.2 |
| D4：Quiz md 由 caller 後處理 frontmatter | 2.3, 3.1, 4.2 |
| D5：Retry = 純 re-spawn，接受隨機 | 6.2 |
| D6：Config migration — default_length 搬至共用 quiz 命名空間 | 1.1, 1.2, 1.3, 6.1 |
| D7：Storage — 每 attempt 獨立 timestamped md | 5.5 |
| D8：測試分層對齊 codebase 慣例（apply 階段發現並收斂） | 2.3, 4.2 |
| D9：`run_quiz` 拆 `run_quiz_plan` + `run_quiz_generate`（task 5.2 動工時收斂） | 5.2 |
| GUI Quiz flow（app-workspace） | 5.1, 5.2, 5.3, 5.4, 5.5 |

### Requirement → implementing tasks

| Spec / Requirement | Tasks |
| --- | --- |
| `app-workspace` / Tauri IPC Commands for Quiz Plan and Generate Lifecycle | 5.2, 5.5 |
| `quiz` / Quiz Verb Library Functions | 2.1, 2.2, 2.4 |
