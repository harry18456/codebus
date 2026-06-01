## Why

codebus 的 raw mirror 對超過 5 MiB 的檔案會跳過（不寫進 `.codebus/raw/code/`），並 bump `oversized_skipped_files` 計數、對 warn sink 輸出一行警告。但 warn 只進到 operator 的 stderr，**讀 `.codebus/raw/code/` 的 agent 對這些檔完全無感**——它連「這裡有一個 8 MiB 的 `dist/bundle.js`」都不知道，蓋架構頁時可能漏掉有結構意義的大檔（大型資料集、vendored 資產）。本變更把被跳過的大檔以一份低噪音的 manifest 暴露給 agent，作為結構訊號。

## What Changes

- 在 raw mirror 寫入路徑（`codebus-core` 的 `sync_with_scanner_into`）收集本次 sync 中所有因超過 5 MiB 而被跳過的檔，並在 sync 結束時寫出**一份彙整 manifest** 到 agent 讀得到的位置 `.codebus/raw/code/_codebus-oversized.md`。
- manifest 內容：一個 header（說明這些檔內容已省略、>5 MiB、僅供結構認知），其後一行一檔＝相對路徑（forward-slash 正規化）＋位元組數。entry 依路徑排序以確保跨平台輸出穩定。
- **只在本次 sync 至少有一個 oversized 檔時才寫 manifest**；本次沒有 oversized 就不留檔。idempotency 由既有行為保證：`sync_with_scanner_into` 開頭會 `remove_dir_all` 整個 `raw_code_dir` 再重建，因此「重 sync 覆蓋舊 manifest」「從有變無刪掉舊 manifest」都自然成立，不需新增刪除邏輯。
- 不破壞既有 surface：既有的 `mirror skip: oversized ...` warn line 與 `oversized_skipped_files` 計數（operator surface）保留不動，manifest 是**額外**的 agent surface。

## Non-Goals

- 不做 per-file stub（避免污染檔樹、避免誘使 agent 逐檔 Read 必然失敗）；只做單一彙整 manifest。
- 不改變 5 MiB 門檻、不改變跳過行為本身、不把大檔內容（即使截斷）寫進 mirror。
- 不動 drift-detection 的 `walk_source_for_signal`（它走 source repo、無 warn sink，行為與 manifest 無關）。
- 不為 manifest 內容跑 PII scanner：manifest 只含相對路徑與位元組數、不含檔案內容，沒有需要掃描的內容。
- 不抽象化：單一 call site，不引入 trait 或可插拔的 manifest writer。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `vault`: 「Raw Mirror with PII Scanner」requirement 新增：當本次 sync 有 oversized 跳過時，SHALL 在 `.codebus/raw/code/_codebus-oversized.md` 寫出一份列出每個被跳過檔（相對路徑＋位元組數）的 manifest；無 oversized 時 SHALL NOT 留下該檔。既有的 warn line／counter 行為不變。

## Impact

- Affected specs: `vault`（Raw Mirror with PII Scanner requirement，新增 manifest 行為與 scenario）
- Affected code:
  - Modified: codebus-core/src/vault/raw_sync.rs（在 `sync_with_scanner_into` 收集 oversized 條目並於結尾寫出 manifest；新增 manifest 格式化 helper 與單元測試）
  - New: 無新檔（manifest 是執行期產生於各 vault 的 `.codebus/raw/code/`，非原始碼）
  - Removed: 無
