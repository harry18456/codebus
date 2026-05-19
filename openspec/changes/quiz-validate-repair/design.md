## Context

`run_quiz_generate`（`codebus-core/src/verb/quiz.rs`）目前是單一 generate spawn → `strip_code_fence` + `strip_preamble_before_first_question` → 回傳 raw `quiz_md`。沒有任何結構驗證或 wikilink 存在性檢查；壞題只被前端 `quiz-parse.ts` 的 `parseQuiz`（容忍式、靜默跳過 malformed block）在 render 時丟掉。對比之下 `goal` 的產出早有 `codebus lint`（決定性）+ `codebus fix`（trust-agent 修復，`wiki::fix` 為 v3-fix-trust-agent 單一 spawn / agent 自身 session 內 loop / CLI 最後一次 lint 當 final verifier）。本 change 把這套 parity 補給 quiz。`.spectra.yaml`：tdd、audit、parallel_tasks、locale tw。

## Goals / Non-Goals

**Goals:**

- Quiz 產出在落檔前經過決定性結構驗證（md schema）與 `[[slug]]` 對 vault wiki index 的存在性驗證。
- 驗證失敗時，由產生 quiz 的同一 agent 在自己 session 內自我修復（trust-agent），library/CLI 事後跑同一 validator 當 final verifier。
- 整個 `generate → 自驗 → 自修 → final verify` 是同一次 run：共用 `on_event`、一份 events.jsonl、一筆 RunLog；findings 以既有 lint-finding 事件形狀呈現，CLI stdout / GUI `QuizLiveStream` / 看過程 modal 三面一致。
- 殘餘失敗 best-effort 落檔 + `validation:` frontmatter 標記 + 非致命 warning event，不丟題、不硬失敗成不落檔。

**Non-Goals:**

- **Stage 2（模型內容裁判）不在本 change 實作。** 只在本設計記錄其插入點與待釘契約；獨立 verify spawn 與「內容 ok」acceptance contract 留待後續 change（契約釘死前不寫實作）。
- 不重開 `goal`：goal 的單一無狀態 spawn 模型沒有 driving defect，不動；本 change 的 session 連續性由「對標 fix trust-agent」達成，與 goal 無關。
- 不把驗證/修復通用化成 verb-agnostic 抽象層（單一 consumer，違反 anti-pattern #1）。
- 不改 plan spawn、不改 quiz 持久化路徑/sidecar/cursor/Review（已 archive 之 quiz-attempt-progress 範圍）。
- 不改前端 `parseQuiz`：它維持 render-time 容忍 parser，非驗證權威。

## Decisions

### D1：Trust-agent 模型（鏡像 v3-fix-trust-agent），不是 CLI 外層 loop

`run_quiz_generate` 維持**單一 generate spawn**。codebus-quiz SKILL workflow 指示 agent：生成 → 用 Bash 工具呼叫決定性 validator 自驗 → 依 findings 修 → 重驗，直到乾淨或達 **SKILL 內寫明的內層 iteration 上限**；上限用盡仍有 finding 就照原樣輸出。spawn 結束後，library 端再跑**同一個 validator 一次**當 **final verifier**，其結果決定 `validation:` 標記與 warning event。

- 理由：`wiki::fix`（lint-feedback-loop）已是專案刻意選定、用以**取代** v3-lint 舊「CLI 外層 ping loop」的模型；quiz 重用同一形狀，認知一致、無需 `--continue`/session resume（chat-verb spike 已標其受限）。
- 否決：CLI 外層 loop（generate→CLI validate→re-spawn repair 餵 findings→重驗 ≤N）。即被廢棄的舊模型；且每次 re-spawn 是新 session、要連續就得 chat-style resume，goal/fix 刻意不做。
- 「session 同一個」語意：是**那一個 generate spawn 自己的 session**（做 quiz 的 agent 就是修 quiz 的 agent），不是 resume 別的 session；goal 那種「持久 session」本不存在。

### D2：決定性 validator 是權威，住 codebus-core，並以 CLI subcommand 暴露給 agent Bash

新增 `codebus quiz validate <quiz-md-file | -> [--json]`（human + JSON 雙輸出，mirror `codebus lint` 的 agent-callable 形狀）。輸入來源**檔案路徑或 stdin**（`-` 或無檔參數 → 讀 stdin）。底層為 codebus-core 一個 quiz validator 函式，輸入 quiz markdown body，輸出結構化 findings。規則兩類：

1. **Schema findings**（severity error）：每題須有非空 stem、恰 4 個 choice 鍵 `A–D`、一個 `## Answer: X` 且 `X∈{A,B,C,D}`、一個 `## Explanation:`；題塊以 `## Q<n>.` 切分；可容忍空行/前後空白（對齊 `quiz-parse.ts` 既有寬鬆度，但這裡是**硬性**驗證而非靜默跳過）。
2. **Wikilink-existence findings**（severity error）：`## Explanation` 內每個 `[[slug]]` 必須能解析到 vault wiki index 既有頁（重用 `wiki::lint` 既有的 wikilink 解析 primitive；不重用 `codebus lint` CLI 入口——quiz md 在 `.codebus/quiz/` 不在 `wiki/`，故只重用解析函式不重用 lint 子命令）。

前端 `parseQuiz` 不變、非權威。

- Interface-depth：seam = codebus-core quiz validator（單一 adapter；真實行為 = schema + link 強制；deletion test：移掉它則 malformed / 斷連 quiz 靜默出貨——站得住，不是 pass-through）。

### D3：共用事件/日誌管線

generate / 自驗（agent 內，透過 quiz validate 的 stream 不直接進；agent 的 Bash tool-use 與 thought 走既有 agent stream）/ final verify 全部走 `run_quiz_generate` 既有的 `fan_out`（同時 write events.jsonl sink + 呼叫 `on_event`），維持**一次 run = 一份 events.jsonl + 一筆 RunLog**。final verify 的 findings 以**既有 lint-finding 事件形狀** emit（不新增 VerbEvent variant；對齊 `wiki::fix` 對 lint findings 的呈現），CLI renderer / GUI `QuizLiveStream` / `read_quiz_events` 看過程 modal 三面自動呈現。

- 否決（方式 B）：把 findings 包成單行文字事件——會與 agent thought 混淆、不可結構化過濾。

### D4：殘餘失敗 = best-effort 落檔 + `validation:` 標記 + warning

final verify 仍有 error finding 時：quiz **照常落檔**（caller frontmatter 內加 `validation:` 欄位，值 `ok` / `failed`，並可帶 finding 摘要），同時 emit 一條非致命 warning event；`run_quiz_generate` 回傳成功（`QuizReport` 反映 `validation` 狀態），不 panic、不回 error、不丟任何題塊。

- 理由：對齊既有容忍哲學（no-match exit 0、部分 quiz 仍可答、run-log 寫失敗 non-fatal）。硬失敗會弄丟一份大致 OK 的 quiz；靜默放行就是今天的 bug。
- audit：`validation` 預設值與缺值語意安全——缺 `validation` 欄位視為「未驗（legacy/外部產生）」而非「已驗通過」，讀取端不得把 absent 當 ok。

### D5：SKILL 是 agent 端 schema source of truth，validator 回饋具體 findings 不重述 schema

codebus-quiz SKILL workflow 內描述 quiz 格式與自驗/自修步驟（agent 端）；core validator 是**決定性權威**，回饋 path/題號/rule 的**具體 finding**，**不**在任何 prompt 重述整份 schema（避免 roadmap anti-pattern #2 schema 雙投遞）。SKILL 不得內嵌 validator 規則細節的平行副本——只引用「跑 `codebus quiz validate` 並依其 findings 修正」。

### D6：Stage 2 插入點（本 change 不實作）

Stage 1 的「把 findings 餵回 agent 自修」路徑設計成**可接受外部來源的 findings**（同一 finding 形狀），使未來 Stage 2 的獨立 model-verify spawn（可帶不同 model/effort）能把其 issues 經同一機制餵回 trust-agent，無需重構 Stage 1。

- **OPEN QUESTION（deferred，實作 Stage 2 前必須釘）**：「內容 ok」的最小 acceptance contract。建議錨點：(1) 忠於 planned pages、不主張超出 scope 頁的內容（no hallucination beyond scope）；(2) answer key 站得住（與題幹/解釋一致）。難度評估 **不納入**（YAGNI）。在契約釘死前不得開 Stage 2 實作；亦明確**不**通用化到 goal（無 trigger 不投機抽象）。

### Bash sandbox

codebus-quiz agent 的 generate spawn toolset 增加 Bash，且**僅** hard-gate 到 `Bash(codebus quiz validate *)`（沿用 lint-feedback-loop 的 PreToolUse hook 安裝機制與 `<vault>/.claude/settings.json` 寫入；whitelist 常數比照 `FIX_BASH_WHITELIST`）。永遠擋 WebFetch/WebSearch/Task/MCP 等不變。

agent 自驗時把 context 裡的草稿**經 stdin 管進** `codebus quiz validate -`（非寫 scratch 檔）。**選此走 stdin 的理由是省流程／去冗餘**：quiz generate 的產出是 agent context 裡的一段文字、最終 emit 給 caller 落檔（caller 補 frontmatter、單一 persist 路徑不變）；若改走「agent 寫 scratch 檔 → 驗檔 → 仍 emit → caller 再落檔」會多一層 scratch 檔生命週期＋寫一次又 emit 一次的雙寫冗餘。stdin 剛好對上「草稿在 context」這形狀，零 scratch。**注意：此處不採「最小權限／安全」作為理由**——goal/fix 本就帶 un-gated vault Write，quiz 走 stdin 純粹是流程較簡潔，不是安全考量；generate toolset 因此維持不加 Write（沒有要寫 scratch 檔的需求，不是為了權限）。

## Implementation Contract

- **新 CLI 行為**：`codebus quiz validate <quiz-md-file | ->`——輸入為**檔案路徑或 stdin**（`-` 或無檔參數讀 stdin）。結構/斷連無問題時 human 輸出「0 issues」、exit 0；有問題列出每筆（題號 + rule + 訊息）、exit 1；setup 錯誤（無 vault / 檔不可讀）exit 2。`--json` 輸出機器可讀 findings 陣列（含 `rule`、`severity`、題號、訊息）、同 exit-code 契約。供 agent Bash（stdin 自驗）與 library final-verify 共用同一 validator 函式。
- **庫行為**：`run_quiz_generate` 結束 generate spawn 後，對 fence/preamble-stripped body 跑 validator 一次；findings 以既有 lint-finding 事件形狀經 `fan_out` emit（events.jsonl + on_event 同步）。回傳的 `QuizReport` 新增驗證狀態欄位（`ok` / `failed`）供 GUI/CLI 顯示。
- **持久化資料形狀**：落檔 quiz 的 caller frontmatter 新增 `validation:` 欄位（`ok` | `failed`）。缺欄位 = 未驗，不得當 ok。
- **失敗模式**：validator 內部錯誤或 wiki index 不可讀 → 視為非致命，emit warning event、`validation: failed`、quiz 仍落檔；`run_quiz_generate` 不因驗證而回 `VerbError`。spawn 本身失敗/cancel 維持既有語意不變。
- **SKILL 契約**：codebus-quiz workflow 含「生成 → 草稿經 stdin 管給 `codebus quiz validate -` 自驗 → 依 findings 修 → 重驗，最多 N 次（N 寫在 SKILL）→ emit 最終 body」；不重述 schema 細節。
- **驗收**：(1) `cargo test -p codebus-core` validator 單元（schema 各壞態、wikilink 存在/不存在、容忍空白）；(2) `cargo test -p codebus-cli` `codebus quiz validate` sub-action human/json/exit code **與 stdin 模式**；(3) mock-spawn 測試證明 `run_quiz_generate` 殘餘失敗時 best-effort 落檔 + `validation: failed` + warning event 且不丟題；(4) events.jsonl 含 finding 事件、與既有 lint-finding 形狀一致；(5) `cargo test`（tauri）+ `npx vitest run` + `npm run typecheck` 無回歸。
- **In scope**：Stage 1 全部（validator + CLI 子命令 + trust-agent SKILL workflow + run_quiz_generate final-verify/marker/events + Bash sandbox）。**Out of scope**：Stage 2 model-verify 實作、其 acceptance contract、goal 通用化、前端 parseQuiz、plan spawn、quiz 持久化/sidecar/Review。

## Risks / Trade-offs

- Agent 自修內層迭代會增加 generate spawn 的 token / 時間。緩解：SKILL 寫明硬上限；用盡即輸出 + `validation:` 標記，不無限跑。
- `validation:` frontmatter 是新欄位；舊 quiz 無此欄位 → 讀取端須把 absent 當「未驗」非「ok」（audit 已涵蓋），不得 break 既有 quiz 開啟。
- validator 與 SKILL 描述若漂移會造成 agent 永遠修不乾淨。緩解：validator 是唯一權威、SKILL 只引用不複製規則（D5）；新增規則時測試同步。
- D4 best-effort 放行壞 quiz 是刻意取捨（可答 > 硬失敗），靠 `validation: failed` 標記 + warning 讓失敗可診斷而非靜默。
