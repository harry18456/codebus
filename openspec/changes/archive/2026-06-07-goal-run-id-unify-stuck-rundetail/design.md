## Context

goal 在 GUI 的生命週期跨越三層，各層各自持有一個「run id」字串：

- **前端 store**（`codebus-app/src/store/goals.ts`）：`spawnGoal` 取得 IPC 回傳的 id，存進 `activeRun.runId`；`Workspace` 把它存進 `selectedRunId`。`_onTerminal` 以 `goal-terminal` payload 的 id 比對 `activeRun.runId` 來清空 activeRun 並 `refreshRuns`。
- **IPC 層**（`codebus-app/src-tauri/src/ipc/goals.rs`）：`spawn_goal_with_runner` 以 `goal_run_id()` 取樣一次 `Utc::now()`（`Millis`、`:`→`-`），作為回傳值、`active_runs` key、`goal-stream` / `goal-terminal` payload 的 `run_id`。
- **verb 層**（`codebus-core/src/verb/goal.rs`）：`run_goal` 內部再以 `run_started_at` 取樣一次 `Utc::now()`（`Millis`），決定 `events-*.jsonl` 檔名 slug 與 `RunLog.started_at`。

三者本應是同一個 id，但 IPC 與 verb 是兩次獨立取樣。goal 結束後，前端以 IPC id 呼叫 `get_run_detail`，後端 `get_run_detail_impl` 以 `summaries.find(|s| s.run_id == run_id)` 比對，磁碟上的 `run_id` 來自 verb slug，兩者差幾毫秒故永遠 miss，回傳 `AppError::Invalid`。`Workspace` 載入 RunDetail 的 effect 以 `.catch(() => {})` 靜默吞掉此錯誤，`selectedDetail` 永遠維持 `null`，render 落在 `workspace.runDetail.loading` 永遠卡住。

此問題在 `cc580dc` 將兩層精度由 `Secs` 升為 `Millis` 後由「罕見」變「常態」：`Secs` 時兩次取樣多半同秒、slug 相同；`Millis` 後幾乎不可能同毫秒。

額外發現：`verb-library` spec 描述的 `run_goal` 簽章為 4 參數，但實作（經 `46d2dee` 加入 per-run timeout 後）實為 5 參數（多了 `timeout: Option<Duration>`）——spec 與實作已漂移，本 change 在改簽章時一併對齊。

## Goals / Non-Goals

**Goals:**

- goal 完成（succeeded / failed / cancelled / interrupted）後，停留在 RunDetail 的使用者能立即看到對應的終態畫面，不再卡「載入中…」。
- 前端追蹤的 run id 與磁碟持久化的 run id 在字面上完全相同，與時間戳精度無關，杜絕未來再因取樣漂移而退化。
- RunDetail 載入失敗時呈現可觀察、可重試的錯誤態，而非無限轉圈。
- spec 與實作的 `run_goal` 簽章一致。

**Non-Goals:**

- 不改 quiz / chat 的核心流程或其 run id 派生（見下方決策）。
- 不改 `RunLog.started_at` 的寫入語意（仍為 RFC 3339、spawn 前擷取）。
- 不重構 `active_runs` / orphan-detection 的整體模型，只移除「雙重取樣」這個漂移源。
- 不改任何 token / 語言 / 模型設定。

## Decisions

### IPC 取樣一次並下傳 run_goal（單一來源根治）

`spawn_goal_with_runner` 改為只取樣一次 `Utc::now()`：產生 colon 形式的 RFC 3339 字串 `started_at`，slug 形式（`:`→`-`）作為回傳值、`active_runs` key、`goal-stream` / `goal-terminal` 的 `run_id`（維持既有對外契約不變），並把 colon 形式的 `started_at` 透過新參數傳進 `run_goal`。`goal_run_id()` helper 由「取樣 + slug」拆解為「取樣得 colon 字串」與「slug 化」兩步，使同一個 colon 字串能同時餵給 slug（前端）與 verb（磁碟）。

替代方案：(a) 維持雙取樣、改前端用「最近一筆 / goal 文字 + 時間窗」模糊比對重新解析 selectedRunId——脆弱、heuristic，且 token/started_at 仍會在其他 join（orphan detection）漂移；(b) 讓 `get_run_detail` 在 exact-miss 時做 fuzzy fallback——把漂移正常化、掩蓋根因。皆否決，因為它們治標而非消除漂移源。

### run_goal 簽章新增 run_started_at 參數（Some 採用 / None 內部派生）

`run_goal` 新增結尾參數 `run_started_at: Option<String>`：`Some(s)` 時，`s` 直接作為 events sink 檔名 slug 來源與 `RunLog.started_at`（取代內部派生）；`None` 時維持現行 `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)` 內部派生。CLI 呼叫者（`codebus-cli/src/commands/goal.rs`）傳 `None`，行為不變；GUI 呼叫者（`ipc/goals.rs`）傳 `Some`。

替代方案：把「取樣」整個移出 `run_goal`、強制所有 caller 提供——會逼 CLI 也改、擴大破壞面且無實益（CLI 沒有跨層 join 需求）。以 `Option` 保留向後相容是最小破壞面。

### 前端 RunDetail 載入失敗改為可重試錯誤態

`Workspace` 載入 selectedDetail 的 effect 不再 `.catch(() => {})`。失敗時設定一個錯誤狀態，render 呈現可重試（retry）與返回清單的錯誤畫面，取代落回 `workspace.runDetail.loading`。此為防禦縱深：即使未來任何 id 對不上，使用者看到的是明確錯誤而非無限轉圈，符合本專案「不要把失敗 wrap 進 non-fatal」原則。

替代方案：只修 run id、不動 `.catch`——但靜默吞錯本身就是讓本 bug 難以察覺的幫兇，保留它等於留一個會再次隱藏同類問題的陷阱。

### 改寫 Precision Alignment Invariant NOTE 並補回歸測試

app-workspace spec 的「Precision Alignment Invariant」NOTE 前提錯誤（同精度 ≠ 同值），改寫為：goal 的 run id 由 IPC 取樣一次、以 slug 形式對外、以 colon 形式下傳 `run_goal`，三個消費點（`active_runs` key、events 檔名 slug、`RunLog.started_at`）源出同一次取樣故字面恆等。既有單元測試 `goal_run_id_precision_matches_verb_run_started_at_slug` 只驗長度（byte length），抓不到值漂移；新增一個回歸測試斷言「以 IPC 回傳的 run id 在 goal 終態寫盤後，`get_run_detail` / `list_runs` 能解析到同一筆 run」。

### quiz / chat 同類 drift 排除於本 change

quiz（`ipc/quiz.rs` 用 `Millis`、`verb/quiz.rs` 用 `Secs`）存在同類雙取樣不一致，但 quiz / chat 前端不存在「完成後以 run id 載 RunDetail」路徑（`get_run_detail` 僅被 goals store 使用），故不會出現本 bug 的卡載入症狀；其潛在影響上限為 orphan / interrupted 標籤，嚴重度低。為控制破壞面與 review 焦點，本 change 僅處理 goal；quiz / chat 的統一另開 change。

## Implementation Contract

**行為**：在 GUI spawn 一個 goal 並停留於其 RunDetail，當 goal 以任一終態結束時，畫面自動切換至對應終態（RunDetailDone / RunDetailInterrupted），不再停在「載入中…」。從 Goals 清單點選歷史 run 的既有行為不變。

**介面 / 資料形狀**：
- `codebus_core::verb::goal::run_goal` 簽章變為：
  ```
  pub fn run_goal(
      repo: &Path,
      options: GoalOptions,
      on_event: impl FnMut(VerbEvent),
      cancel: Option<Arc<AtomicBool>>,
      timeout: Option<std::time::Duration>,
      run_started_at: Option<String>,
  ) -> Result<GoalReport, VerbError>
  ```
  `run_started_at` 為 `Some(rfc3339_colon)` 時取代內部派生作為 events 檔名 slug 來源與 `RunLog.started_at`；`None` 時內部以 `Millis` 派生。
- IPC `spawn_goal_with_runner` 回傳值、`active_runs` key、`goal-stream` / `goal-terminal` payload 的 `run_id` 仍為 slug 形式（`:`→`-`），對外契約不變；其值與傳入 `run_goal` 的 `Some` 字串為同一次取樣。
- `goal-stream` / `goal-terminal` 的 Tauri event 名稱與 payload 結構不變。

**失敗模式**：RunDetail 載入（`get_run_detail`）失敗時，`Workspace` 呈現可重試錯誤態（含 retry 動作與返回清單），不得靜默吞錯、不得退回 loading 文案。

**驗收**：
- 新增 Rust 回歸測試：模擬 IPC 取樣一次 → 以該 id 下傳 `run_goal`（或其等價的 sink 寫盤）→ 以 IPC slug 呼叫 `get_run_detail_impl` / `list_runs_impl` 必須命中同一筆 run。
- 既有 `cargo test -p codebus-core` 與 `cargo test -p codebus-cli`（mock spawn）保持綠燈；CLI goal 路徑（傳 `None`）行為不回歸。
- 前端 Vitest：goal terminal 後 `selectedDetail` 成功載入時切到終態；`get_run_detail` reject 時呈現錯誤態而非 loading。
- 手動 / CDP smoke：GUI spawn goal、停在 detail、待其完成，畫面自動進終態。

**範圍邊界**：
- In scope：goal 的 run id 統一（IPC + verb + CLI 三呼叫點）、`run_goal` 簽章、`Workspace` RunDetail 載入錯誤態、app-workspace 與 verb-library spec 對齊、回歸測試。
- Out of scope：quiz / chat 的 run id 統一；`active_runs` / orphan-detection 模型重構；`run-log` 寫入語意。

## Risks / Trade-offs

- [改 `run_goal` 公開簽章為 BREAKING] → 僅 2 個 in-repo 呼叫者（CLI、IPC），同 change 一併更新；`Option` 結尾參數使 CLI 端僅需傳 `None`，破壞面最小；mock 與整合測試覆蓋兩條路徑。
- [順手對齊 spec 內 `timeout` 漂移可能被視為夾帶] → 不對齊則新簽章 spec 仍與實作不符、留下新的漂移；在 design 與 spec delta 明記此對齊屬必要之舉。
- [只修 goal、quiz/chat 同類問題仍在] → 已確認 quiz/chat 無「以 run id 載 RunDetail」路徑、不會出現本卡載入症狀；以 Non-Goals 與決策明確記錄，避免被誤認為遺漏。
- [前端錯誤態文案 / i18n] → 沿用既有 i18n message key 機制新增錯誤與 retry 文案，與既有 LoadingOverlay 失敗態風格對齊。

## Open Questions

(none)
