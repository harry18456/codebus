## Context

`quiz-content-verify`（archived）在 `run_quiz_generate` 內就地實作了「獨立模型判內容是否恰當 → 有界 repair」。`run_goal`（`verb/goal.rs`）流程：spawn goal agent（`GOAL_TOOLSET` 含 Write/Edit，**直接寫多個 wiki 頁**，內容來自 PII-filtered `raw/code/`）→ `wiki_changed_since_last_commit` → fix 迴圈（`run_fix_loop`：lint + trust-agent，結構品質）→ `auto_commit`。**已有結構把關，沒有「內容忠於源碼/切題/歸位」的獨立 AI 驗收**。使用者明示要 goal 也要＝第 2 個真實 consumer 成立，共用機制正當（不再投機）。`.spectra.yaml`：tdd、audit、parallel_tasks、locale tw。

## Goals / Non-Goals

**Goals**

- 把 quiz 既有 verify→repair 編排**下沉成共用 `verb::content_verify` core**；quiz/goal 各注入「取待驗內容 / 套用修復」適配。**quiz 對外行為不變**（refactor，非行為變更）。
- `run_goal` 在 **fix 迴圈後、`auto_commit` 前** 加可選 content-verify 階段（`goal.content_verify` config 閘、預設 false）。
- goal「恰當」契約 = 固定 3 缺陷：**unfaithful**、**off-goal**、**taxonomy-misplaced**。
- 殘餘 best-effort（warning、不還原頁、exit 不變、auto_commit 照常）。
- **CLI `codebus goal` 與 GUI goal flow 都實作**（使用者明示，行為對等）。

**Non-Goals**

- 不做 B1（goal agent 自審）；要獨立第二模型。
- 不做開放式品質評分／文風／完整度；契約只認 3 種有限可判缺陷。
- **不把 PII 漏遮當缺陷**（leaked-sensitive 已否決：PII 沒處理好是 PII filter 的問題，分離關注）。
- 不改決定性 `codebus lint` / fix 迴圈、不改 quiz 對外行為、不改前端 parseQuiz、不加 UI 徽章、不新增 subcommand/IPC。
- 真 claude e2e 依 quiz 系列慣例 deferred（自動化 + mock + 抽樣真跑）。

## Decisions

### D1：抽共用 `verb::content_verify` core（quiz 行為不變地重用）

新模組 `codebus_core::verb::content_verify`：
- `ContentReview { Ok, Flagged(Vec<u32>) }`（泛化自 `QuizContentReview`，`frontmatter_value()` 格式不變：`ok` / `flagged [1, 3]`）。
- `ContentDefect { qnum/id, kind, suggestion }` + `parse_content_defects(text) -> Option<Vec<ContentDefect>>`（由 quiz.rs 原樣搬出：`CONTENT_OK`→`Some(vec![])`、`Q<n> | type | sug`→defects、皆無→`None`）。
- `run_content_verify_loop`：擁有「verify→（有缺陷則）repair→re-verify、硬 cap=3、到頂保留最佳、best-effort」編排；caller 注入兩個閉包：(a) `verify(current) -> Result<Option<Vec<ContentDefect>>, VerbError>`（跑獨立 verify spawn + 解析）、(b) `repair(current, &defects) -> Result<String/(), VerbError>`（跑 repair spawn 並套用）。
- **quiz 重構**：`run_quiz_generate` 改呼叫 shared core，adapter：verify=回傳字串、repair=regenerate body；`QuizContentReview` 變 `ContentReview` re-export/alias。**外部行為（events、`content_review` frontmatter、cap、best-effort）逐項保持，既有 quiz 測試全綠為驗收門檻**。

否決 B（goal 先獨立做）：2 consumer 已成立、越晚抽越難、兩份平行邏輯會分歧（anti-pattern #1 的反面：此時不抽才是錯）。

### D2：goal「恰當」契約 = 固定 3 缺陷

verify 模型讀〔本次變更的 wiki 頁 + 本次 goal 文字 + 對應 `raw/code/` 源碼〕，逐頁只判：

1. **unfaithful**：頁面主張的事實在 `raw/code/` 找不到依據（幻想/與源碼矛盾）。
2. **off-goal**：頁面內容與本次 `goal` 無關（不該為這次 goal 產出）。
3. **taxonomy-misplaced**：內容放錯頁型/資料夾（如 process 內容寫進 concepts 頁）。

逐頁輸出 `<wiki相對路徑> | <defect-type> | <修正建議>`，全無缺陷輸出 `CONTENT_OK`。否決開放評分（不可測、無限震盪）。

### D3：goal 整合位置與變更頁偵測

位置：`run_fix_loop` 之後、`auto_commit` 之前。變更頁偵測：vault 是 nested git repo（`<vault>/.git`）；在 goal agent spawn **之前**記錄 vault `HEAD`（pre-run rev），content-verify 階段用 `git -C <vault> diff --name-only <pre-run rev> -- wiki/` 取本次新增/修改的 wiki 頁清單（空清單＝無頁變更，直接視為 ok、不 spawn）。verify spawn read-only，但 toolset 需可讀 `raw/code/`（unfaithful 要對源碼核實——與 quiz verify「wiki-only」不同）。repair spawn = 帶 Write 的修頁 agent（比照 `GOAL_TOOLSET`），餵缺陷、只改被點名頁、就地改磁碟；之後 re-verify。事件走既有 `fan_out`（events.jsonl + on_event）。

### D4：殘餘 best-effort（鏡像 quiz D4）

到 cap 仍有缺陷：**不還原任何頁**；emit 非致命 warning；`GoalReport` 加 content-review 狀態欄位；`run_goal` 不因內容缺陷回 `VerbError`、exit 不變；**`auto_commit` 照常執行**（部分修復後的 wiki 仍正常 commit——content-verify 永不擋 commit）。audit：狀態欄缺值 = 未驗，讀取端不得當 ok。

### D5：config `goal.content_verify` + SKILL verify mode

新增 `codebus_core::config::goal`（仿 `config::quiz`：top-level `goal.content_verify: bool`，缺省 false，forward-compat 容錯）。codebus-goal SKILL 加 `verify:` mode（比照 codebus-quiz Mode C）：描述 3 缺陷契約、逐頁輸出格式、明示可讀 `raw/code/` 供 grounding、不重述決定性 lint 規則。決定性 `codebus lint`/fix 不動。

### D6：CLI + GUI 對等（皆於本 change 實作）

`codebus-cli/src/commands/goal.rs`：解析 `goal.content_verify`（core loader）+ 把本次 goal 文字一併傳入 `run_goal`（off-goal 需 goal 文字——goal 流程本就有）。`codebus-app/src-tauri/src/ipc/goals.rs`：同源解析 + 串入（GUI 行為對等）。`GoalOptions`（或等效輸入）加 `content_verify` 旗。**不新增 subcommand/IPC、不加 UI 徽章**（比照 quiz `content_review` 落檔不顯示）。

**Interface-depth**：seam = 共用 `verb::content_verify`（單一 orchestrator；quiz/goal 注入 verify/repair 閉包；深度＝獨立 LLM 判斷 + 有界 repair 編排，非 pass-through；deletion test：移掉 → quiz 與 goal 皆只剩結構閘）。此 change 動到「已 ship 的 quiz」內部，故行為保持＝既有 quiz 全測試綠是硬門檻。

## Implementation Contract

- **共用 core**：`verb::content_verify` 提供 `ContentReview`、`parse_content_defects`（語意同 quiz 原實作）、`run_content_verify_loop`（cap=3、到頂最佳、best-effort、事件走 caller `fan_out`）。
- **quiz**：`run_quiz_generate` 改用 core；落檔 `content_review` 值、events、cap、best-effort、`content_verify=false` 全不變——以「既有 quiz 自動測試（codebus-core/cli quiz_flow）全綠且行為輸出逐項一致」為驗收。
- **goal 庫行為**：`goal.content_verify==true` 時，`run_goal` 於 fix 迴圈後 / auto_commit 前：git-diff 取本次變更 wiki 頁；無變更→直接 ok 不 spawn；有→獨立 verify spawn（read-only，可讀 `raw/code/`）判 3 缺陷→有缺陷跑 repair spawn（Write，能改頁）只修被點名頁→re-verify，外層 cap=3，到頂 best-effort。`content_verify==false`→完全不跑（行為等同今日 run_goal）。
- **資料形狀**：`GoalReport` 新增 content-review 狀態（`ok` / `flagged`+頁清單 / 未驗）。缺值不得當 ok。auto_commit 與 exit code 不受 content-verify 影響。
- **失敗模式**：verify spawn 失敗/不可解析→非致命，warning、保守標 flagged、不還原、auto_commit 照常、verb 不失敗。
- **config**：`~/.codebus/config.yaml` `goal.content_verify`（bool，缺省 false）；CLI 與 app 同源 core loader 解析；不讀 `app.*`。
- **CLI+GUI**：`codebus goal` 與 GUI goal-spawn IPC 皆解析 config 並執行；無新 subcommand/IPC/UI。
- **驗收**：(1) `cargo test -p codebus-core` config::goal（缺省 false、true/false、容錯）+ shared core 單元（parse_content_defects 各態、loop cap/best-effort）；(2) **既有 quiz 全測試綠**（行為不變回歸門檻）；(3) mock-spawn 測試（goal flow）：content_verify=false→不跑；clean→GoalReport content-review ok、auto_commit 照常；flag→≤3 輪 repair→殘餘 flagged+warning+不還原+exit 不變+commit 照常；無頁變更→直接 ok；(4) skill-bundle 測試：codebus-goal SKILL 有 `verify:` mode 含 3 契約、可讀 raw/code 說明、不重述 lint；(5) GUI tauri 測試：goal-spawn IPC 解析 config + 串入（capturing runner 斷言）；(6) `cargo test -p codebus-core -p codebus-cli`、`cargo test`（tauri）、`npx vitest run`+`npm run typecheck` 無回歸。
- **In scope**：共用 core 抽取 + quiz 行為不變重用 + goal verify+repair（3 契約、位置、best-effort）+ config::goal + codebus-goal SKILL verify mode + CLI + GUI 對等 + mock/tauri 測試。**Out of scope**：B1、開放品質評分、leaked-sensitive/PII、決定性 lint/fix、quiz 對外行為、前端 parseQuiz、UI 徽章、新 subcommand/IPC、真 claude e2e（deferred）。

## Risks / Trade-offs

- **動到已 ship 的 quiz**：抽取若改了 quiz 行為＝回歸。緩解：行為保持式 refactor，既有 quiz 全自動測試綠為硬門檻；core 單元測試覆蓋 parse/loop。
- **goal repair 成本/侵入**：repair spawn 帶 Write 改 wiki 頁，比 quiz regenerate 字串重。緩解：預設 false、cap=3、只改被點名頁、到頂即停。
- **unfaithful 要讀 raw/code**：verify 模型需讀源碼核實，read scope 比 quiz verify 廣。緩解：verify 仍 read-only（不 Write）、SKILL 明示僅為 grounding 讀取、不洩 raw 內容進輸出（只回缺陷判定）。
- **變更頁 git-diff 邊界**：偵測錯會驗錯頁。緩解：以 goal agent spawn 前的 vault HEAD 為 base、限 `-- wiki/`、空清單短路。
- **非決定性裁判震盪**：cap=3 + 到頂最佳（quiz 已驗模式）。
