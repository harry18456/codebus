## Context

`quiz-validate-repair`（已 archive）給 quiz 加了決定性 final-verify（schema + `[[slug]]` 存在）+ Stage-1 agent 自修，並在 design D6 預留 Stage 2 插入點：「把外部來源、同一 finding 形狀的 issues 餵回 trust-agent 自修」。本 change 實作 Stage 2 = **獨立模型判內容好壞**。決定性 validator 看不出「答案其實錯」「題目講了頁面沒有的事」「離題」。`.spectra.yaml`：tdd、audit、parallel_tasks、locale tw。

## Goals / Non-Goals

**Goals**

- **CLI 與 GUI 行為對等**：兩者皆經 `run_quiz_generate` 走同一 verify→repair 階段（使用者明示 app 跟 cli 都要做）。
- generate 後（決定性 final-verify 之後）跑一次**獨立 verify spawn**，依固定 5 項缺陷契約逐題判內容。
- verify 缺陷由 caller 編排迴圈餵進 repair spawn（reuse generate mode）修，有界（cap=3、到頂出最佳版）。
- 落檔加 `content_review: ok|flagged`（+ flagged 題號）；殘餘 best-effort（warning、不丟題、exit 不變）。
- 整個 stage 由 `quiz.content_verify` config 閘控、**預設 false**。

**Non-Goals**

- **不做 B1（出題 AI 自審）**：要獨立第二模型才有抓盲點價值（使用者原話「另一個模型」）。
- 不做難度校準、文風評分、題數檢查、「整體品質打分」——契約只認 5 種**有限可判**缺陷（避免無限改 / 不可測）。
- **不通用化到 goal**（無 trigger，投機抽象，違 anti-pattern #1）。
- **不動 lint-feedback-loop hook / sandbox**：verify 模型 read-only 無 Bash；repair 重用 generate 既有 Stage-1 sandbox。
- 不新增 subcommand / sub-action（verify 為內部 spawn，使用者不直接呼叫）；不改決定性 `codebus quiz validate`。
- 不改前端 parseQuiz、plan spawn、quiz 持久化路徑/sidecar。
- **GUI 行為對等 in scope（D8）；但不在 GUI 加 content_review 可視徽章**——比照 Stage-1 `validation` 也未在 UI 顯示，只落檔。UI 顯示另議，不在本 change。

## Decisions

### D1：獨立 verify spawn（B2），跑在 run_quiz_generate 內、config 閘控

`quiz.content_verify == true` 時，`run_quiz_generate` 在決定性 final-verify 之後，reuse `run_spawn` 跑一個**獨立** verify spawn（與 generate 不同 spawn；初期沿用 generate 的 resolved model/effort，不加新 model config — YAGNI）。否決 B1（同一 agent 自審）：自己改自己考卷抓不到盲點，且使用者明確要「另一個模型」。

### D2：「內容 ok」契約 = 固定 5 項逐題缺陷

verify 模型讀〔planned pages + 生成的 quiz（+ Goal flow 的原 topic）〕，逐題只判這 5 種、各回「該題號 + 缺陷類型 + 具體修正建議」：

1. **answer-wrong**：標記正解對不上 planned pages 的事實。
2. **out-of-scope**：題幹/選項/解釋主張了 planned pages 沒有的內容。
3. **not-exactly-one-correct**：兩個以上選項可為正解，或標記正解其實不對。
4. **degenerate-distractor**：distractor 敷衍（空白、「以上皆非」式 cop-out、明顯荒謬無鑑別度）。
5. **off-topic**：題目沒在問使用者要求的 topic。**僅 Goal flow**（有 topic 時）判；Page flow 無 topic → 跳過此項。

否決「給品質分數 / 開放式好不好」：不可測、auto-repair 會無限震盪。固定有限清單才能收斂、可 mock 測。

### D3：有界 caller 編排 verify→repair 迴圈（新增機制；修正 archived D6 的誤述）

**Grounded 更正**：archived quiz-validate-repair 的「D6 插入點 / trust-agent 接受外部 findings」只是當時的設計願景文字，**從未 ship 成 caller 端程式**——Stage-1 shipped 的 `run_quiz_generate` 沒有 caller repair 迴圈（repair 純粹是 generate agent 在「單一 spawn 內」用自己 SKILL 的 `codebus quiz validate` 自修；caller 端只做一次決定性 final-verify 後標記）。因此本 change **必然新增一個 caller 編排機制**（與本 change 早先草稿「不新增獨立 repair 機制」的措辭相反，已於此更正）。

機制：`run_quiz_generate` 在決定性 final-verify 之後（`content_verify==true` 時）跑一個 caller 編排迴圈：

1. 跑**獨立 verify spawn**（Mode C `verify:`，read-only，無 Bash）→ stdout 解析為 `CONTENT_OK` 或數行 `Q<n> | <defect-type> | <suggestion>`。
2. 無缺陷 → 跳出（`content_review: ok`）。
3. 有缺陷 → 跑一個 **repair spawn**：reuse 既有 `generate:` mode（不新增 SKILL mode，避免超出 120 行 cap），prompt payload 夾帶〔同一 pages + count + 前一版 quiz body + verify 缺陷清單 + 「保留無缺陷題、只改被點名題、維持題數」指示〕→ 取其新 body（仍經 generate agent 自己的結構自修）。
4. 對 repair 後 body 再跑 verify。重複 2–4，**硬上限 cap = 3 輪**（與 Stage-1 SKILL 自修 cap 數值一致，但這是 caller 計數的外層迴圈，非 SKILL 內層）；到頂出當前最佳 body、不再循環。

verify/repair 的事件皆走既有 `fan_out`（events.jsonl + on_event）。此迴圈是**獨立模型當裁判（B2）+ caller 編排有界 repair**，非 Stage-1 的 intra-spawn 自修（那是 B1，已否決）。

### D4：殘餘 best-effort（鏡像 Stage-1 D4）

到 cap 仍有缺陷：quiz 照常落檔，caller frontmatter 加 `content_review: flagged` 並列 flagged 題號（全清則 `content_review: ok`）；emit 一條非致命 warning；**不丟任何題**；`run_quiz_generate` 不因內容缺陷回 `VerbError`、exit code 不變。audit：缺 `content_review` 欄位 = 未做內容驗（config 關或舊檔），讀取端**不得**當 `ok`。

### D5：config 閘 `quiz.content_verify`，預設 false，config-only

新增 `quiz.content_verify: bool`（預設 false）。CLI 讀此鍵（比照 `quiz.default_length`，永不讀 `app.*`）。**不加 `--content-verify` flag**（YAGNI；日後要再加不遲）。預設關：既有使用者不被默默多收 verify+repair 的 spawn 成本。

### D6：topic 串接

off-topic（D2 #5）需使用者原 topic。`run_quiz_generate` 目前只收 `QuizGenerateOptions { pages, question_count }`、拿不到 topic（plan/generate 拆開）。新增把 topic 以 `Option<String>` 串入（Goal flow caller 傳 `Some(topic)`；Page flow 傳 `None` → verify 跳過 #5，其餘 4 項照判）。

### D7：SKILL 新增 `verify:` mode；決定性 validator 不動

codebus-quiz SKILL 加第三 mode `verify:`，描述 5 項缺陷契約與「逐題輸出 題號+類型+修正建議」格式；引用契約為權威、不重述決定性 schema 規則（沿用 Stage-1 D5 反 schema 雙投遞）。`codebus quiz validate`（決定性）維持只管 schema+wikilink。verify 模型 toolset = read-only（Read/Glob/Grep），無 Bash、無 hook（故不碰 lint-feedback-loop）。

### D8：GUI 行為對等——`spawn_quiz_generate` IPC 接 config + 串 topic

GUI 經 `codebus-app` 的 `spawn_quiz_generate` IPC → `run_quiz_generate`。本 change 讓 GUI 與 CLI **行為對等**（使用者明示）：

- `spawn_quiz_generate`（`#[tauri::command]`）解析共用 `quiz.content_verify`（用 core 的 `default_config_path` + `load_quiz_config`，與 CLI `resolve_content_verify` 同源；載入錯誤保守 false），把 `content_verify` 傳進 `spawn_quiz_generate_with_runner`。
- `spawn_quiz_generate_with_runner` 由既有 `trigger` 參數導出 originating topic：`QuizTriggerArg::AiPlanned { topic }` → `Some(topic)`（Goal flow，off-topic 可判）；`QuizTriggerArg::WikiPreview { .. }` → `None`（Page flow，跳過 off-topic、其餘 4 項照判）。據此建 `QuizGenerateOptions { content_verify, topic, .. }`（取代先前寫死的 `false / None`）。
- 不在 GUI 加 content_review 可視元素（比照 `validation` 亦未在 UI 顯示，只落檔；UI 顯示另議）。verify 模型沙箱與 CLI 同（read-only、無 Bash、不碰 hook）。

**Interface-depth**：seam = `verb::quiz` 的 verify+repair 段（reuse `run_spawn` 單一 adapter；深度＝獨立 LLM 判斷 + 有界 trust-agent 修，非 pass-through；deletion test：移掉只剩決定性結構閘、無內容把關 → 站得住）。GUI 端僅在既有 `spawn_quiz_generate` IPC 接點注入 config+topic，不新增 IPC seam。

## Implementation Contract

- **庫行為**：`quiz.content_verify == true` 時，`run_quiz_generate` 於決定性 final-verify 後跑獨立 verify spawn；缺陷以 Stage-1 finding 形狀經既有 `fan_out`（events.jsonl + on_event）emit，並由 caller 編排迴圈（verify→repair spawn→re-verify）修，外層硬 cap=3 輪，到頂出最佳版。`content_verify == false` → 完全不跑 verify（行為等同 Stage-1 結束）。
- **資料形狀**：落檔 caller frontmatter 新增 `content_review: ok | flagged`；`flagged` 時另列 flagged 題號。`QuizReport` 新增對應狀態欄位供 CLI/GUI 顯示。缺欄位 = 未驗，不得當 ok。
- **介面**：`run_quiz_generate` 新增「originating topic」輸入（`Option<String>`）；Goal flow（CLI `codebus quiz "<topic>"` 與 GUI Goal flow）傳 `Some`；Page flow 傳 `None`（#5 跳過）。
- **GUI 行為對等（D8）**：`codebus-app` `spawn_quiz_generate` IPC 解析共用 `quiz.content_verify`（core `load_quiz_config`，錯誤保守 false）並由 `trigger` 導出 topic（`AiPlanned`→`Some`、`WikiPreview`→`None`），注入 `QuizGenerateOptions`；GUI 落檔同樣帶 `content_review`；不新增 IPC、不加 UI 徽章。
- **config**：`~/.codebus/config.yaml` `quiz.content_verify`（bool，缺省 false）；CLI 讀取，不讀 `app.*`。
- **失敗模式**：verify spawn 失敗 / 不可解析 → 視為非致命，emit warning、`content_review: flagged`（保守，不當 ok）、quiz 仍落檔、verb 不因此失敗。generate/cancel 既有語意不變。
- **驗收**：(1) `cargo test -p codebus-core` config 載入 `quiz.content_verify`（缺省 false、true/false 解析、容錯）；(2) mock-spawn 測試（codebus-cli `quiz_flow`）：verify 全 ok behavior → 落檔 `content_review: ok`、無 warning；verify 點名某題 behavior → 觸發 ≤3 輪 repair、最終 `content_review`（清→ok / 殘留→flagged + 題號 + 非致命 warning）、題數不減、exit 0；`content_verify=false` → 不跑 verify、無 `content_review`（回歸保護）；(3) skill-bundle materialization 測試：SKILL 有 `verify:` mode 含 5 項契約、不重述決定性 schema；(4) `cargo test -p codebus-core -p codebus-cli`、`cargo test`（tauri）、`npx vitest run` + `npm run typecheck` 無回歸。
- **In scope**：上述 verify+repair+frontmatter+config+topic 串接+SKILL `verify:` mode + **GUI 行為對等（spawn_quiz_generate 接 config+topic，D8）** + mock/tauri 測試。**Out of scope**：B1、難度/文風/題數/品質評分、goal 通用化、決定性 validator、hook/sandbox、前端 parseQuiz、GUI content_review 可視徽章、真 claude e2e（依 quiz 系列慣例 deferred）。

## Risks / Trade-offs

- **成本**：開啟後每份 quiz 多一次 verify + 最多 3 輪 repair spawn（generate 已含自驗）。緩解：預設 false、硬 cap、到頂即出。
- **非決定性裁判**：LLM 判斷不穩。緩解：契約限 5 種具體可判缺陷（非開放評分）；測試只驗 wiring 用 mock，不驗 LLM 品質；殘餘走 best-effort 標記非硬擋。
- **震盪**：對模糊裁判 auto-repair 可能不收斂。緩解：cap=3 + emit-best-on-cap（同 Stage-1 已驗模式）。
- **off-topic 僅 Goal flow**：Page flow 無 topic 跳過 #5，刻意取捨（Page flow 的 scope 已由 #2 約束）。
