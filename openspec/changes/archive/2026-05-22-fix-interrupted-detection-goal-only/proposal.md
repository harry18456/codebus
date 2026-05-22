## Why

`list_runs` 的中斷偵測（interrupted detection）由 app-workspace spec `Interrupted Run Detection` 規範，但實作有兩個源自該需求的缺陷：

1. **非 goal verb 被誤判為 goal**：spec 明文「中斷偵測只適用 goal-mode run」，但 `list_runs_impl` 對任何「有 `events-<slug>.jsonl` 但無對應 `runs-*.jsonl` row」的孤兒 events 檔，一律合成 `mode="goal"` 的虛擬 interrupted 紀錄——因為 events 檔名不帶 verb 資訊，實作無法區分。後果：chat / query / fix / quiz 在「進行中」時（events 已建立、RunLog row 未寫），被 watcher 觸發的 `refreshRuns` 撈進來、暫時以空 goal 文字（前端 fallback `(no goal text)`、icon `⚠`）出現在 Goals 列表，待其結束寫入 `mode != "goal"` 的 RunLog row 後才消失。這是使用者在 app 用 chat 對話時看到 goal 欄位暫時出現 `(no goal text)` 的直接原因。

2. **進行中的 goal 在列表被顯示為中斷**：spec 用「events 有、RunLog 無」當中斷判據，此判據無法分辨「正在執行」與「已中斷」。當使用者在 app 自行啟動的 goal 仍在執行時切回 Goals 列表，列表的 `refreshRuns` 會把磁碟合成的 `interrupted` 紀錄覆蓋掉樂觀插入的 `running` 紀錄，使該 goal 顯示為 `⚠`（中斷）而非 `🚌`（進行中）。

## What Changes

- **後端（缺陷 1）**：修正 `list_runs_impl` 合成虛擬 interrupted 紀錄的判據——只有當孤兒 events 檔可被辨識為 goal-mode run 時才合成。辨識判據為「events 檔開頭事件含 `VerbBanner::Goal`」，只有 `goal` verb 會發出該 banner，chat / query / fix / quiz 不會。無法辨識為 goal 的孤兒 events 檔 SHALL NOT 合成任何虛擬紀錄，因此不再出現在 Goals 列表。
- **後端 spec（缺陷 1）**：更新 app-workspace `Interrupted Run Detection` 需求，把「goal-mode only」這個既有意圖落實為可驗證的偵測判據（以 `VerbBanner::Goal` 是否存在判定），並補上 scenario 涵蓋非 goal verb 孤兒 events 不得合成。
- **前端（缺陷 2）**：`useGoalsStore.refreshRuns` 在以 `list_runs` 結果覆寫 `runs` 時，若存在 `activeRun`，SHALL 以該進行中 run 的 `running` 摘要覆蓋同 `run_id` 的磁碟合成紀錄，使使用者自行啟動且仍在執行的 goal 在 Goals 列表維持 `running` 顯示。此為前端狀態合併修正，不更動 `list_runs` IPC 契約。

## Non-Goals

- 不重構 events 檔命名或加入 verb 前綴（會牽動 `events-log` / 各 verb 的 sink 契約，超出本次根因修復範圍）。
- 不改變 CLI / 其他行程啟動、且本 app 無對應 `activeRun` 的進行中 goal 之顯示——此情況下 app 無從得知該行程仍在執行，顯示為 `interrupted` 仍符合 spec 既有意圖。
- 不為 chat / query / fix / quiz 新增各自的中斷偵測（維持 spec 既有「v1 不在範圍」立場）。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-workspace`: `Interrupted Run Detection` 需求改為以「events 檔是否含 `VerbBanner::Goal`」作為合成虛擬 interrupted 紀錄的判據，使非 goal verb 的孤兒 events 檔不再被合成為 goal 列。

## Impact

- Affected specs: `app-workspace`
- Affected code:
  - Modified:
    - codebus-app/src-tauri/src/ipc/goals.rs
    - codebus-app/src/store/goals.ts
