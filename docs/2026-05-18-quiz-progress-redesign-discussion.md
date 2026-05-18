# Quiz GUI 互動與持久化模型重設計 — 討論紀錄

> 日期：2026-05-18。`/spectra-discuss` 產出。狀態：**結論已收斂**，待 `/spectra-propose` 接續。
> 觸發：fix-app-quiz 收尾後，user 指出現有 quiz 模型不符直覺（點 history 只看到 raw md、答案裸露、無進度、不能續答/重考）。

## 1. 現況（grounded，已查證）

- **生成的 quiz md** = 不可變紀錄：`<vault>/.codebus/quiz/<slug>/<quiz_id>.md`，frontmatter（`quiz_id` / `trigger` / `topic`|`target_page` / `planned_pages` / `generation_token_usage` / `events_log`）+ body（`## Q1..QN` / `## Answer:` / `## Explanation:`）。**無任何作答 / 進度欄位。**
- `QuizAttemptMeta`（`ipc/quiz.rs`）：slug / quiz_id / trigger / topic / target_page / events_log / path。**無題數、已答數、分數。**
- `QuizAnswering`：純 in-memory `useState`（idx/selected/revealed/correctCount/done）。**不持久化**；離開即丟。
- history 清單 = 這些靜態紀錄；點 attempt = `<pre>` 原樣秀 md（連 `## Answer:` 露出）。
- `quiz` spec（v3-app-quiz，已 archive 並 sync 進 main specs）「Quiz Storage Layout and Retry Semantics」明訂：**每 attempt 一個 timestamped 不可變檔、retry 永不覆蓋**。
- 無 `openspec/LANGUAGE.md`。

→ 現況**忠實實作了 v3-app-quiz 原設計**（生成→一次答完→summary→history 看靜態紀錄）。user 想要的「quiz 是有 N 題、可續答、列表顯示進度」**從未被設計**。這不是 bug，是模型差異。

## 2. 核心抉擇：作答進度存哪

| 方案 | 進度存放 | Pros | Cons |
|---|---|---|---|
| A. 寫回原 attempt md | frontmatter 加 `answers` / `answered` / `score` | 單一檔；history scan 已讀 frontmatter | **違反** archived `quiz` spec「attempt 不可變、retry 永不覆蓋」→ 須重開該 spec；reopened 易並發污染 |
| **B. 不可變 md + progress sidecar** | `<quiz_id>.progress.json`（同目錄） | 保留 v3 不可變契約；生成內容 vs 使用者作答 生命週期分離；vault 可攜、CLI 可讀；無 sidecar = 未開始（0/N） | 每 attempt 兩檔；history scan 要多讀 sidecar |
| C. 純前端 localStorage | app 端 | 不動檔格式 | 清資料即失、不隨 vault、CLI 不可見 → **否決** |

**決定：B**。理由（關鍵取捨）：v3-app-quiz 的儲存契約剛 sync 進 main specs 且刻意設計成「不可變、可回看每次 AI 出了什麼」；A 會破壞該契約並要重開已 archive 的 spec。生成內容（agent 事實）與使用者作答（可變 session 狀態）本就是不同生命週期，分離最乾淨，且 sidecar 保 vault 可攜 / CLI 可讀。

## 3. 收斂後的模型

### 儲存
- `<slug>/<quiz_id>.md` — 生成內容，**不可變**（維持現狀）。
- `<slug>/<quiz_id>.progress.json` — 可變，schema：
  - `schema_version`（forward-compat，沿用 codebus config tolerance 精神）
  - `total`（題數，由 md 解析）
  - `answers`: `[{ q: <1..N>, selected: "A|B|C|D", correct: bool }]`（依作答順序）
  - `answered_count` / `correct_count`
  - `status`: `in_progress` | `completed`
  - `started_at` / `completed_at`（RFC3339）
  - 缺 sidecar = 「未開始」（answered 0 / total = 解析 md 得）

### 語意（重考 vs 續答 vs 看解答）— 三者明確區分
- **續答 Resume**：sidecar `in_progress` → 從第一題未答處接續。
- **看解答 Review**：sidecar `completed` → 唯讀模式逐題顯示「使用者選擇 vs 正解 + 解釋」（不可再答）。
- **重做此份 Redo-this**：對**同一份生成 quiz**（同題目）重置該 attempt 的 sidecar（刪/歸零），重新作答。
- **重出新的 Retry-new**：= `+ New quiz` 同主題 → 走 plan/generate 出**新題目**的新 attempt（沿用 v3-app-quiz D5「retry = plain re-spawn」，**不變**）。

→ 「重考」拆成兩個明確 affordance：Redo-this（同題重做）與 Retry-new（新題）。避免語意混淆。

### History 清單
每列：`topic/target · quiz_id · 狀態徽章`
- 未開始：`未開始 · 0/N`
- 作答中：`作答中 · X/N`
- 已完成：`已完成 · X/N · score% · pass|fail`

點列行為依狀態：
- 未開始 / 作答中 → 進**答題（續答）**流程
- 已完成 → 進**看解答（review）**模式（內含「重做此份」「看過程」入口）

`+ New quiz` 維持只在 history/idle（fix-app-quiz #7 已定）。

## 4. Interface depth check（觸發：新 storage 抽象 + 新 IPC）

1. **Seam**：progress sidecar 的讀寫單元放 **codebus-core**（與 `persist_quiz` 同層，CLI/GUI 單一真相），GUI 經新 IPC `read_quiz_progress` / `write_quiz_progress`（`ipc/quiz.rs`，registry +2）。
2. **Adapter count**：一個 — core 的 progress (de)serialize + 原子寫；IPC 僅薄包 + containment guard（reuse `read_quiz_attempt` 樹外拒絕）。
3. **Depth**：真行為 — schema 解析/容錯、status/score 計算、原子寫、樹內路徑守衛。非純轉發。
4. **Deletion test**：刪掉它 → 無續答/進度/review、history 無 X/N。會壞且有意義 → 抽象成立。

## 5. 對既有 spec 的影響（會動契約 → 須新 change，不可只 ingest）

- `quiz`：新增 progress sidecar 儲存契約（與不可變 md 並存；定義 schema、缺 sidecar 語意）。
- `app-workspace`：「Quiz History List」改為「依狀態顯示進度徽章 + 點列依狀態進 續答/review」；「Quiz Answering and Summary」加 進度持久化 / resume / review / redo-this 行為；新 IPC registry 數量（23 → 25）。
- 屬 **v3-app-quiz 之後的功能演進**，非 bug fix → 開新 change（建議名 `quiz-attempt-progress`），不重開已 archive 的 v3-app-quiz / fix-app-quiz。

## 結論

**Decision**：採「不可變生成 md + 可變 progress sidecar（`<quiz_id>.progress.json`）」模型；history 依狀態（未開始/作答中/已完成 X/N·score）顯示並路由（續答 / review）；「重考」拆為 Redo-this（同題重做、重置 sidecar）與 Retry-new（`+ New quiz` 同主題出新題，沿用 v3 retry=re-spawn）。progress 讀寫單元置 codebus-core，GUI 經新 IPC。

**Rationale**：保留 v3-app-quiz 已 archive 的「attempt 不可變」儲存契約（A 方案會破壞之）；生成事實 vs 使用者作答 生命週期分離；vault 可攜 / CLI 可讀。

**Next step**：`/spectra-propose quiz-attempt-progress`（feature change），以本討論結論為輸入。**不**動已 archive 的 v3-app-quiz / fix-app-quiz。

**Open（propose 時定）**：score 是否寫入 sidecar 或每次由 answers 重算（傾向重算，單一真相）；review 模式要不要也允許「只看錯題」；progress.json 原子寫策略（temp+rename）。
