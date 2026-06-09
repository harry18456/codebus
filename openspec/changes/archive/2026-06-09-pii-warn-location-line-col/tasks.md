## 1. byte offset → line:col 轉換

- [x] 1.1 在 codebus-core/src/vault/raw_sync.rs 新增私有 helper `byte_offset_to_line_col(content: &str, offset: usize) -> (usize, usize)`，回傳 1-based `(line, col)`：line = offset 之前 `\n` 的數量 + 1；col = 該行起點到 offset 之間的 Unicode scalar 數量 + 1。offset 超過 `content.len()` 時夾到 `content.len()`，不得 panic。**驗證**：同檔新增單元測試 `byte_offset_to_line_col_*`，至少涵蓋（a）第一行 offset 8 → `(1, 9)`；（b）`a\nb\ncontact ...` 第 3 行 → `(3, 9)`；（c）offset 0 → `(1, 1)`；（d）多位元組字元（如中文）前的 offset 其 col 以 scalar 計非 byte 計。`cargo test -p codebus-core byte_offset_to_line_col` 全綠。

## 2. warn 輸出改用 line:col（實作 Warn Sink Location Format）

- [x] 2.1 實作 Warn Sink Location Format：修改 codebus-core/src/vault/raw_sync.rs 寫 warn 行的 `writeln!`（現為 `"pii warn: {} at {}:{}"` 帶 `m.start`），改成呼叫 1.1 helper 取得 `(line, col)`，輸出 `"pii warn: {} at {}:{}:{}"` 帶 `pattern_name, rel_str, line, col`。`m.start`/`m.end` 結構欄位不得更動（masking 仍用 byte offset）。**驗證**：以含 `contact alice@example.com\n` 的來源跑 sync，warn 行等於 `pii warn: email at docs.md:1:9`，不再出現原始 byte offset。
- [x] 2.2 更新同檔描述 warn 行格式的 doc 註解（現寫 `... at <relative_path>:<byte_offset>`），改為 `... at <relative_path>:<line>:<col>`，與實際輸出一致。**驗證**：grep 該檔註解不再含 `byte_offset` 字樣於 warn 格式描述處。

## 3. 既有測試對齊

- [x] 3.1 更新 codebus-core/src/vault/raw_sync.rs 內既有斷言 warn 格式的測試，使其鎖定新的 `line:col` 輸出而非只檢查前綴：將 `docs.md` 案例斷言為精確等於 `pii warn: email at docs.md:1:9`；`logs.txt`（`key1=alice@example.com\nkey2=192.168.1.1\n`）案例補上對應的 `:1:` / `:2:` 行號斷言。`aws-access-key`、非 UTF-8、Skip/Warn policy 等既有測試維持通過。**驗證**：`cargo test -p codebus-core raw_sync` 全綠。

## 4. 整體驗證

- [x] 4.1 跑 `cargo test -p codebus-core` 與 `cargo clippy -p codebus-core`，確認無測試失敗、無新增 clippy 警告。**驗證**：兩指令皆乾淨結束（clippy 以既有 baseline 為準，不得新增警告）。
