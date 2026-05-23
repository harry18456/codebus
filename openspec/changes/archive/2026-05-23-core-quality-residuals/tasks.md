<!--
Each task description states the behavior delivered AND the verification target.
File paths appear only as supporting locator context, never as the task itself.
-->

## 1. F2 — Oversized file warn + counter（Raw Mirror with PII Scanner）

- [x] 1.1 RED：為 `Raw Mirror with PII Scanner` 要求的「oversized skip 發 stderr 行 + counter 計數」契約在 `codebus-core/src/vault/raw_sync.rs` 既有 test 模組新增 2 條測試——(a) 單一 oversized 檔產生一行符合 `mirror skip: oversized at <rel_path> (<N> bytes > 5 MiB limit)` 格式的 warn AND `summary.oversized_skipped_files == 1`；(b) 兩個 oversized 檔產生兩行 warn AND counter 等於 2 AND 同批次的小檔仍被 mirror。完成行為：2 條新測試 FAIL（counter 欄位不存在 / warn 行不存在）。驗證方式：`cargo test -p codebus-core vault::raw_sync` 顯示新增測試 failed AND 既有測試仍 pass。
- [x] 1.2 RED：更新既有 `files_over_5_mib_are_skipped` 測試補上 stderr warn assertion（既有只 assert 檔案不存在於 raw mirror，無 sink assertion）；改用 `sync_with_scanner_into` + `Vec<u8>` 接 warn sink 並驗證內容。完成行為：原 test 變成同時驗 skip + warn + counter 三件事。驗證方式：執行該 test 顯示 FAIL（counter 欄位 / warn 行未實作）。
- [x] 1.3 GREEN：在 `codebus-core/src/vault/raw_sync.rs` `SyncSummary` 加 `oversized_skipped_files: usize` 欄位；`sync_with_scanner_into` 在 `meta.len() > MAX_FILE_BYTES` 分支內寫一行 `mirror skip: oversized at <rel_path> (<N> bytes > 5 MiB limit)` 到 `warn_sink`（路徑用既有 `rel.to_string_lossy().replace('\\', "/")` 規範化），然後 `summary.oversized_skipped_files += 1`，再 `continue`。完成行為：1.1 + 1.2 所有測試 pass，既有測試仍 pass。驗證方式：`cargo test -p codebus-core vault::raw_sync` 全綠。
- [x] 1.4 確認 `walk_source_for_signal`（drift detection 路徑）保持原狀——既有測試 `walk_source_for_signal_skips_nested_dot_git` 仍 pass AND `source_signal.file_count` / `total_bytes` 計算不變（與 spec scenario "Source signal walk silently skips oversized files without warn" 對齊）。完成行為：walk 路徑無 warn / 無 counter 變動，但仍排除 oversized 檔。驗證方式：`cargo test -p codebus-core vault::raw_sync::tests::walk_source_for_signal_skips_nested_dot_git` 綠 + `git diff` 範圍肉眼 review 確認 walk_source_for_signal 函式體無改動。

## 2. F3 — changed_paths_under 排除 deleted 路徑 + 補 test 模組

- [x] 2.1 RED：在 `codebus-core/src/git/nested_repo.rs` `tests` 模組新增 `changed_paths_under` 測試模組（既有完全沒有），涵蓋——(a) 新增檔（A）→ 包含；(b) 修改檔（M）→ 包含；(c) 重命名檔（R，git mv）→ 包含；(d) **刪除檔（D）→ 排除**（F3 fix 的核心 assertion）；(e) 未追蹤新檔 → 包含（既有 `ls-files --others` 行為）；(f) 空 diff → 回傳空 list；(g) subdir filter 正確（diff 不會包含 subdir 之外的變更）。每條 test 用 `init_nested_repo` + `auto_commit` 鋪一個 base commit 後執行對應操作再呼叫 `changed_paths_under`。完成行為：5-7 條新測試，其中 (d) 因 code 尚未加 diff-filter 而 FAIL，其他理應 pass（A/M/untracked/empty 既有行為已 OK）。驗證方式：`cargo test -p codebus-core git::nested_repo` 顯示 (d) deleted-excluded 測試 failed AND 其他新測試 pass。
- [x] 2.2 GREEN：實作「changed_paths_under 用 `--diff-filter=ACMR` 白名單」決議——`codebus-core/src/git/nested_repo.rs::changed_paths_under` 把 `capture_git(vault_root, &["diff", "--name-only", base, "--", subdir])` 改成 `capture_git(vault_root, &["diff", "--name-only", "--diff-filter=ACMR", base, "--", subdir])`。`ls-files --others` 那行不動（untracked 仍要含）。完成行為：2.1 所有新測試 pass，既有 init/auto_commit 測試仍 pass。驗證方式：`cargo test -p codebus-core git::nested_repo` 全綠。
- [x] 2.3 確認 `verb-library` spec.md `Goal Content Verify` 條款（line 450 附近）的「diffing the vault git repository ... created or modified pages」意圖與本次修法一致——不改 spec，但在 PR 描述記錄「code 對齊既有 spec 意圖」這個 framing。完成行為：spec 不動但有對齊紀錄。驗證方式：`grep -n "created or modified" openspec/specs/verb-library/spec.md` 命中該段 + PR description 含此對齊聲明。

## 3. 整合與最終驗證

- [x] 3.1 全工作區測試：`cargo test --workspace` 全綠，含既有 PII / vault / verb / git 測試與本 change 新增的 oversized + changed_paths_under 測試。完成行為：整個 workspace 通過。驗證方式：CLI 輸出 0 failures。
- [x] 3.2 spectra validate：`spectra validate core-quality-residuals` 通過——spec/tasks 一致性、無 forbidden words、Scenario 格式皆正確。完成行為：validate 0 errors 0 warnings。驗證方式：CLI 輸出。
- [x] 3.3 手動 sanity（F2）：在臨時目錄建一個 6 MiB 二進位檔 + 一個小檔，跑 `codebus init` 對該目錄，確認 stderr 出現一行 `mirror skip: oversized at <path> (...)` 並包含位元組數與 `> 5 MiB limit` 字樣，且 raw mirror 內只有小檔。完成行為：F2 修法在真實 binary 下可觀察。驗證方式：終端輸出 paste 到 PR 描述。
- [x] 3.4 手動 sanity（F3）：在臨時 vault 內建 base commit、刪一個 wiki 頁、再 commit，呼叫 `changed_paths_under` 確認回傳 list 不含被刪頁。可透過 cargo test 既有 unit test 達成（task 2.1 的 deleted-excluded 案例），不需獨立 binary 驗證——`changed_paths_under` 為 library 內部函式，無 CLI surface。完成行為：F3 修法的核心 assertion 由 unit test 驗證。驗證方式：task 2.1 deleted-excluded test pass 即覆蓋。
