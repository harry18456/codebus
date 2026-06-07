## Why

GUI 在 spawn 一個 goal 後停在 RunDetail 等待，goal 完成或失敗時右側內容區會永遠卡在「載入中…」（`workspace.runDetail.loading`），使用者再也看不到結果，必須切 tab 或回 Lobby 再從清單重新點進去才能看到 RunDetail。

根因是「前端追蹤 run 的 id」與「寫到磁碟的 run id」來自兩個各自獨立的 `chrono::Utc::now()` 取樣：IPC 層的 `goal_run_id()` 取一次（回傳前端，作為 `selectedRunId` / `activeRun.runId` / `active_runs` key / `goal-terminal` payload），verb 層的 `run_started_at` 又取一次（決定 `events-*.jsonl` 檔名 slug 與 `runs-*.jsonl` 的 `RunLog.started_at`）。`spawn_goal` 呼叫 `run_goal` 時並未把 id 往下傳。commit `cc580dc` 把兩邊精度從 `SecondsFormat::Secs` 升為 `Millis` 後，兩次取樣幾乎不可能落在同一毫秒，slug 必然不同；goal 結束後前端以 IPC id 呼叫 `get_run_detail` 在磁碟（verb slug）永遠找不到對應 run 而被 reject，`Workspace` 的 `.catch(() => {})` 又把錯誤靜默吞掉，於是 `selectedDetail` 永遠是 `null`、永遠卡住。`Secs` 時代兩次取樣多半落在同一秒故 slug 相同、僅跨秒邊界偶發 miss（舊評估「機率很小」）；毫秒化把這個罕見 miss 翻成了常態 miss，是一個被 `cc580dc` 引入的 regression。

## What Changes

- **統一 goal 的 run id 來源（根治）**：IPC 層只取一次時間戳，以 slug 形式（`:` → `-`）回傳前端並作為 `active_runs` key 與 terminal payload，同時把對應的 colon RFC 3339 形式往下傳進 `run_goal`，讓 `events-*.jsonl` 檔名與 `RunLog.started_at` 使用與前端字面完全相同的 id。兩邊不再各自取樣，與時間戳精度完全無關、零漂移空間。
- **`run_goal` 簽章新增一個 caller 提供的 started-at 參數**：`Some` 時直接採用、`None` 時維持現行內部 `Utc::now()` 派生（CLI 路徑不變）。**BREAKING**（公開函式簽章變更，僅 2 個 in-repo 呼叫者）。同時把 verb-library spec 內已與實作漂移的 `run_goal` 簽章（缺少現存的 `timeout` 參數）一併對齊。
- **前端防禦縱深**：`Workspace` 載入 RunDetail 失敗時不再 `.catch(() => {})` 靜默吞掉，而是呈現可重試的錯誤態，避免任何未來的 id 對不上時再次退化成無限轉圈。
- **改寫 app-workspace spec 的「Precision Alignment Invariant」NOTE**：原 NOTE 的前提（三個值都用相同 `Millis` 精度派生即可保證相等）是錯的——相同精度不代表兩個不同時刻的取樣值相等。改寫為「單一 run id 由 IPC 取樣一次並往下傳給 verb」的模型，並補一個「spawn → 完成後 `selectedRunId` 仍能解析到磁碟 run」的回歸 scenario。

## Non-Goals

- 不改 quiz / chat 的核心流程。quiz 雖有同類的 IPC（`Millis`）vs verb（`Secs`）取樣不一致，但 quiz / chat 前端沒有「完成後以 run id 載 RunDetail」這條路徑（`get_run_detail` 只在 goals store 使用），不會出現本 bug 的卡載入症狀；其影響上限是 orphan / interrupted 標籤，嚴重度較低，留待另開 change，本 change 僅在 design 記錄此決策。
- 不改任何固定的 token / 語言 / 模型設定。
- 不改 `run-log` 的 `RunLog.started_at` 寫入語意（仍為 spawn 前擷取的 RFC 3339）；變的只是該時間戳的「來源」可由 caller 提供。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-workspace`: `spawn_goal` 的 `RunId` 派生改為「取樣一次並下傳 `run_goal`」；改寫 Precision Alignment Invariant NOTE 為單一來源模型；新增 spawn 後完成仍能載入 RunDetail 的回歸 scenario。
- `verb-library`: `run_goal` 簽章新增 caller 提供的 started-at 參數（`Some` 採用 / `None` 內部派生），並對齊現存 `timeout` 參數漂移。

## Impact

- Affected specs: `app-workspace`, `verb-library`
- Affected code:
  - Modified:
    - codebus-core/src/verb/goal.rs
    - codebus-app/src-tauri/src/ipc/goals.rs
    - codebus-cli/src/commands/goal.rs
    - codebus-app/src/components/workspace/Workspace.tsx
  - New: (none)
  - Removed: (none)
