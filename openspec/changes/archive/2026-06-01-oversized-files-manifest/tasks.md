# Tasks

說明：本變更僅觸及 `codebus-core/src/vault/raw_sync.rs` 單一檔案，故所有任務序列相依、不標記 `[P]`。遵循 TDD（先 RED 後 GREEN）。所有 scenario 對應 spec capability `vault` 的 `Requirement: Raw Mirror with PII Scanner`。

## 1. 測試先行（RED）

- [x] 1.1 在 raw_sync.rs 的 `#[cfg(test)] mod tests` 新增測試 `oversized_skip_writes_manifest_with_path_and_size`：sync 跳過一個 > `MAX_FILE_BYTES` 的 `dist/bundle.js` 與一個 `small.txt`。行為斷言：`raw_code_dir/_codebus-oversized.md` 存在、內容含 forward-slash 路徑 `dist/bundle.js` 與其 byte count、不含被跳過檔的任何內容、`small.txt` 仍被鏡像、`summary.oversized_skipped_files == 1`。對應 `Requirement: Raw Mirror with PII Scanner` 的 scenario「Oversized skip writes an agent-visible manifest listing path and size」。驗證：`cargo test -p codebus-core oversized_skip_writes_manifest_with_path_and_size` 先跑 SHALL FAIL（manifest 尚未實作）。
- [x] 1.2 新增測試 `multiple_oversized_listed_sorted_by_path`：sync 跳過 `vendor/big.tar` 與 `assets/dataset.csv` 兩個 oversized 檔。行為斷言：manifest 含兩個 entry、每個 entry 配 forward-slash 路徑＋byte count、且 `assets/dataset.csv` 在 `vendor/big.tar` 之前（entry 依路徑排序）。對應 scenario 的 `##### Example: two oversized files listed in path order`。驗證：`cargo test -p codebus-core multiple_oversized_listed_sorted_by_path` 先跑 SHALL FAIL。
- [x] 1.3 新增測試 `no_oversized_leaves_no_manifest`：sync 無任何 > `MAX_FILE_BYTES` 檔。行為斷言：sync 完成後 `raw_code_dir/_codebus-oversized.md` 不存在。對應 scenario「No oversized files leaves no manifest」。驗證：`cargo test -p codebus-core no_oversized_leaves_no_manifest`（此為守恆測試，實作前後皆 SHALL PASS，鎖定「無 oversized 不留檔」契約）。
- [x] 1.4 新增測試 `stale_manifest_removed_on_oversized_free_resync`：先對某 raw dir sync 一個 oversized 檔，再對同一 raw dir 第二次 sync（來源已無 oversized 檔）。行為斷言：第二次 sync 後 `raw_code_dir/_codebus-oversized.md` 不存在（前一輪的 stale manifest SHALL NOT persist）。對應 scenario「A later oversized-free sync does not leave a stale manifest」。驗證：`cargo test -p codebus-core stale_manifest_removed_on_oversized_free_resync`，鎖定 idempotency 契約。

## 2. 實作（GREEN）

- [x] 2.1 依決策「在 walk 期間收集 oversized 條目、迴圈結束後寫單一 manifest」與「manifest 是額外 surface，既有 counter／warn line 不動」：在 `sync_with_scanner_into` 作用域新增 `Vec<(String, u64)>`，於既有 oversized 分支（meta.len() > MAX_FILE_BYTES）在保留既有 `oversized_skipped_files += 1` 與 `writeln!(warn_sink, "mirror skip: oversized ...")`（含其 best-effort 吞錯語意）之外，多 push 一筆 `(rel_str.clone(), meta.len())`。行為：oversized 跳過行為與既有 operator surface 逐一不變，新增收集無副作用。驗證：既有測試 `files_over_5_mib_are_skipped`、`oversized_file_warn_line_includes_byte_count_and_increments_counter`、`multiple_oversized_files_aggregate_counter_and_warns`、`oversized_skip_survives_failing_warn_sink` 全數仍 PASS（不回歸）。
- [x] 2.2 依決策「manifest 內容格式：header＋每檔一行（路徑＋bytes），依路徑排序」：新增格式化 helper（如 `fn format_oversized_manifest(entries: &[(String, u64)]) -> String`），輸出以 header 起（說明 content omitted、超過 5 MiB、listed for structural awareness），其後一行一檔＝forward-slash 路徑＋bytes；helper 內先依路徑排序再格式化，確保跨平台輸出穩定。行為：給定一組 entry 產出確定性、不含檔案內容的 Markdown 文字。驗證：由測試 1.1／1.2 斷言內容含路徑＋bytes 且排序正確。
- [x] 2.3 依決策「manifest 位置與命名：`raw_code_dir/_codebus-oversized.md`」與「idempotency 由既有 `remove_dir_all` 保證、不新增刪除邏輯」：新增 manifest 檔名 `const`（`_codebus-oversized.md`），在 walk 迴圈結束、`Ok(summary)` 之前，若收集的 Vec 非空，best-effort `fs::write` manifest 到 `raw_code_dir.join(<const>)`（寫入失敗 SHALL 吞錯、不 abort sync，對齊既有 oversized warn-line 哲學，並加註解說明）；不新增任何刪除舊 manifest 的程式碼（目的目錄開頭已 `remove_dir_all` 全量重建）。同處加一行註解記錄「source 根同名檔會被覆蓋」之可接受取捨。行為：有 oversized 才寫、無則不留、stale 自然不殘留。驗證：測試 1.1／1.3／1.4 全 PASS。

## 3. 驗證與收尾

- [x] 3.1 全套測試：`cargo test -p codebus-core` 全綠（新增 1.1–1.4 與既有 oversized／PII／nested-git 測試皆不回歸）。
- [x] 3.2 建置與 lint：`cargo build` 成功且 `cargo clippy --workspace` 無新 warning（對齊既有 baseline）。
