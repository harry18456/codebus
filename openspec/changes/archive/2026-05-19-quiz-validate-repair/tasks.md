<!--
Traceability：每 task 標交付行為、驗證目標、對應 design 決策與 spec requirement（逐字）。TDD：(RED) 先寫失敗測試，(GREEN) 才實作到綠。audit：validator 規則容錯、`validation:` 缺值語意安全。檔案重疊（codebus-core verb/quiz.rs、validator 模組、skill_bundle、cli）且有資料依賴（SKILL/whitelist 引用 cli sub-action、整合依賴 validator），故序列、無 [P]。
-->

## 1. 決定性 quiz validator（codebus-core）（design D2/D5；spec `quiz` / Quiz Output Validation and Repair）

- [x] 1.1 (RED) 在 codebus-core 新增 quiz validator 模組的單元測試：schema findings——題塊缺非空 stem、非恰 4 個 `A–D` choice、缺 `## Answer: X`(X∈ABCD)、缺 `## Explanation:` 各產 1 筆 `error` finding；容忍空行/前後空白不誤報；wikilink-existence——`## Explanation` 內 `[[slug]]` 對 vault wiki index 不存在產 `error`、存在不產。測試先 fail（validator 函式未存在）。對應 design D2 與 spec `quiz` / Quiz Output Validation and Repair。
- [x] 1.2 (GREEN) 實作 validator 函式：輸入 quiz md body，輸出結構化 findings（含 rule/severity/題號/訊息）；wikilink-existence 重用 `wiki::lint` 既有 wikilink 解析 primitive（不重用 `codebus lint` CLI 入口）。驗證：1.1 全綠；`cargo test -p codebus-core` 不回歸。對應 design D2/D5 與 spec `quiz` / Quiz Output Validation and Repair。依賴 1.1。

## 2. `codebus quiz validate` sub-action（codebus-cli）（design D2；spec `cli` / Quiz Validate Sub-Action Behavior、Subcommand Registration）

- [x] 2.1 (RED) 在 codebus-cli 新增 sub-action 測試：`codebus quiz validate <file>` 乾淨檔 human「0 issues」exit 0；有壞題/斷連列每筆（題號+rule+訊息）exit 1；`--json` 輸出 findings 陣列（每筆含 `rule`/`severity`/題號/訊息）exit 1；`codebus quiz --help` 列出 `validate` sub-action 且 `codebus --help` 仍恰八個 top-level subcommand。測試先 fail（sub-action 未註冊）。對應 spec `cli` / Quiz Validate Sub-Action Behavior + Subcommand Registration。
- [x] 2.2 (GREEN) 註冊 `quiz validate` sub-action，呼叫與 library final-verify 共用的同一 validator 函式（task 1.2）；human/json 輸出與 exit code 依契約。驗證：2.1 全綠；`codebus --help` 仍八個 subcommand 無第九；`cargo test -p codebus-cli` 不回歸。對應 spec `cli` / Quiz Validate Sub-Action Behavior、Subcommand Registration。依賴 1.2。

## 3. Generate agent 自驗 enablement：stdin + Bash sandbox whitelist（codebus-cli + codebus-core agent config）（design D2/「Bash sandbox」；spec `cli` / Quiz Validate Sub-Action Behavior）

- [x] 3.1 (RED) 測試兩部分：(a) `codebus quiz validate -`（或無檔參數）從 stdin 讀 body，套用與 file 模式相同 findings 輸出與 exit-code 契約（乾淨 exit0、有 finding exit1、`--json` 同形狀）；(b) codebus-quiz generate spawn 的 agent toolset 含 Bash 且 hard-gate 常數為 `Bash(codebus quiz validate *)`（比照 `FIX_BASH_WHITELIST` 形狀），generate toolset **不**加 Write/Edit，非 `codebus quiz validate ...` 的 Bash 被 PreToolUse hook 擋、WebFetch/WebSearch/Task/MCP 仍永遠擋。測試先 fail（stdin 模式未支援、whitelist 常數/wiring 未存在）。對應 design D2/「Bash sandbox」與 spec `cli` / Quiz Validate Sub-Action Behavior（含 stdin scenario）。
- [x] 3.2 (GREEN) (a) 在 `quiz validate` sub-action 加 stdin 來源（`-`/無檔參數讀 stdin），與 file 模式共用同一 validator 與 exit-code 路徑；(b) 新增 quiz Bash whitelist 常數（`QUIZ_BASH_WHITELIST` / `QUIZ_GENERATE_TOOLSET`）並參數化 `run_spawn`，generate spawn 傳 `Some(QUIZ_BASH_WHITELIST)` + generate toolset（含 Bash、不含 Write/Edit），plan spawn 維持 `QUIZ_TOOLSET` 唯讀無 Bash。驗證：3.1 全綠；`cargo test -p codebus-core -p codebus-cli` 既有 sandbox/sub-action 不回歸。對應 design D2/「Bash sandbox」與 spec `cli` / Quiz Validate Sub-Action Behavior。依賴 2.2。
- [x] 3.3 (RED) `codebus hook check-bash` 測試：收到 `codebus quiz validate -`／`<abs>/codebus quiz validate draft.md --json` → allow（exit 0、無 decision JSON）；`codebus quiz "topic"`（generate 形式，非 validate）與 `codebus fix ...` → block；既有 `codebus lint *` allow 與其他 block 行為不回歸。測試先 fail（hook 寫死只准 lint）。對應 spec `lint-feedback-loop` / Fix Bash Hook Installation（新增 quiz validate allow 場景）。
- [x] 3.4 (GREEN) 擴充 `codebus-cli` `codebus hook check-bash` 的 allow 規則：argv 為 `codebus lint ...` **或** `codebus quiz validate ...` 時 allow，其餘照舊 block（嚴格 argv 比對、fail-closed 不變）。驗證：3.3 全綠；`cargo test -p codebus-cli` 既有 hook/lint 測試不回歸。對應 spec `lint-feedback-loop` / Fix Bash Hook Installation。依賴 3.2。

## 4. `run_quiz_generate` final-verify 整合（codebus-core/src/verb/quiz.rs）（design D1/D3/D4；spec `quiz` / Quiz Output Validation and Repair）

- [x] 4.1 (RED) mock-spawn 測試：generate 後跑 validator 一次作 final verifier；findings 經既有 `fan_out`（同一 events.jsonl + on_event）以既有 lint-finding 事件形狀 emit；乾淨→落檔 frontmatter `validation: ok` 且無 warning event；殘餘 error→落檔 `validation: failed` + 非致命 warning event + 不丟任何題塊 + `run_quiz_generate` 不回 `VerbError`；wiki index 不可讀→warning + `validation: failed` + 不失敗；一次 run 僅一筆 RunLog；`QuizReport` 含驗證狀態欄位。測試先 fail。對應 design D1/D3/D4 與 spec `quiz` / Quiz Output Validation and Repair。
- [x] 4.2 (GREEN) 在 `run_quiz_generate` 整合上述：fence/preamble-strip 後跑 task 1.2 validator 一次、findings 走 fan_out、依結果寫 caller frontmatter `validation:`（缺值語意 = 未驗、讀取端不得當 ok）、殘餘失敗 best-effort 落檔 + warning、validator infra 錯誤非致命、`QuizReport` 補驗證狀態欄位。驗證：4.1 全綠；`cargo test -p codebus-core -p codebus-cli` 不回歸。對應 design D1/D3/D4 與 spec `quiz` / Quiz Output Validation and Repair。依賴 1.2、3.2。

## 5. codebus-quiz SKILL bundle 內容（codebus-core skill bundle）（design D1/D5；spec `skill-bundles` / Quiz Skill Bundle Content）

- [x] 5.1 (RED) skill-bundle materialization 測試：`codebus-quiz/SKILL.md` 的 `generate:` mode 含「呼叫 `codebus quiz validate` 自驗 → 依 findings 修 → 重驗，最多 N 次（N 明寫於 body）→ 達上限輸出當前最佳 body」；body 引用 validator 為結構/citation 權威且**不**含 validator 規則定義的重述副本；既有 plan/generate/read-scope 行為不回歸。測試先 fail。對應 design D1/D5 與 spec `skill-bundles` / Quiz Skill Bundle Content。
- [x] 5.2 (GREEN) 更新 codebus-quiz SKILL 內容加上有界自驗/自修 loop（引用 `codebus quiz validate`、明寫內層 cap、不重述 schema 規則），保留既有 plan/generate/read-scope/violation/語言規則。驗證：5.1 全綠；既有 skill-bundle 測試不回歸。對應 design D1/D5 與 spec `skill-bundles` / Quiz Skill Bundle Content。依賴 2.2。

## 6. 全域回歸 sweep（design「In scope」/「Out of scope」/「Implementation Contract」）

- [x] 6.1 全域回歸：`cargo test -p codebus-core -p codebus-cli`、`cargo test --manifest-path codebus-app/src-tauri/Cargo.toml`、`cd codebus-app && npx vitest run && npm run typecheck` 彙總 0 failed；確認未回歸既有 quiz 兩段式/persist/sidecar/Review、plan spawn、前端 parseQuiz、goal/fix；確認 Stage 2 model-verify 與 goal 通用化確實未實作（Non-Goals 守住）；確認 hook 改動未回歸既有 `codebus lint *` allow 與 fail-closed block。對應 design Goals / Non-Goals / In scope / Implementation Contract。依賴 1.2、2.2、3.2、3.4、4.2、5.2。

## Traceability

| Design topic | Tasks |
| --- | --- |
| D1：Trust-agent 模型（鏡像 v3-fix-trust-agent），不是 CLI 外層 loop | 4.1, 4.2, 5.1, 5.2 |
| D2：決定性 validator 是權威，住 codebus-core，並以 CLI subcommand 暴露給 agent Bash | 1.1, 1.2, 2.1, 2.2 |
| D3：共用事件/日誌管線 | 4.1, 4.2 |
| D4：殘餘失敗 = best-effort 落檔 + `validation:` 標記 + warning | 4.1, 4.2 |
| D5：SKILL 是 agent 端 schema source of truth，validator 回饋具體 findings 不重述 schema | 1.2, 5.1, 5.2 |
| D6：Stage 2 插入點（本 change 不實作） | 6.1 |
| Bash sandbox | 3.1, 3.2, 3.3, 3.4 |
| Implementation Contract | 1.2, 2.2, 3.2, 3.4, 4.2, 5.2, 6.1 |
| Goals | 6.1 |
| Non-Goals | 6.1 |
| In scope | 6.1 |
| Out of scope | 6.1 |

## Spec requirement coverage

| Spec requirement | Tasks |
| --- | --- |
| Quiz Output Validation and Repair | 1.1, 1.2, 4.1, 4.2 |
| Quiz Validate Sub-Action Behavior | 2.1, 2.2, 3.1, 3.2 |
| Subcommand Registration | 2.1, 2.2 |
| Fix Bash Hook Installation | 3.3, 3.4 |
| Quiz Skill Bundle Content | 5.1, 5.2 |
