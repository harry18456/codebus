## Problem

PR #1 review 的 T6 core-quality-review 抓到三條 bug：F1（PII overlap）、F2（>5 MiB 靜默排除）、F3（`changed_paths_under` 含刪除頁）。F1 已隨 PR #1 archive、F4 + 威脅 C 已 archive 為 `agent-hook-hardening`。**F2 與 F3 是 T6 review 殘餘的最後兩條，本 change 一次收掉**。

### F2：>5 MiB 檔案被靜默排除出 raw mirror

`codebus-core/src/vault/raw_sync.rs::sync_with_scanner_into` 在 `MAX_FILE_BYTES = 5 MiB` 上方的檔案直接 `continue`——**不複製、不發 warn、不在 `SyncSummary` 計數**。後果：大型 generated source / data fixture 在 raw mirror 不見、`goal`/`query` 看不到、使用者無從得知為何某檔「沒被文件化」。Silent gap 對信任度傷害大於資料安全（無安全後果）。

### F3：`changed_paths_under` 把刪除頁也算 changed

`codebus-core/src/git/nested_repo.rs::changed_paths_under` 用 `git diff --name-only <base> -- <subdir>` **不帶 diff-filter**——`git diff` 預設**含 deleted 路徑**（以舊路徑）。唯一 production caller 是 `verb/goal.rs::run_goal` 的 content-verify 階段（`goal.rs:440`）；deleted 頁面被傳給 verify spawn 後，agent 嘗試 `Read` 已不存在的檔 → I/O error。`verb-library` spec.md:450 明寫「diffing ... created or modified pages」——spec 已對、code 沒對齊。

額外：`changed_paths_under` **無既有 unit test**（`codebus-core/src/git/nested_repo.rs::tests` 只覆蓋 `init_nested_repo` / `auto_commit`）。本 change 順手補一整套 test。

## Root Cause

### F2
`sync_with_scanner_into` 在大檔處只 `continue` 跳過，沒接 `warn_sink`（雖然函式已有 `warn_sink: W` 參數可用）也沒更新 `SyncSummary`。`vault` spec.md:109 "Raw Mirror with PII Scanner" 條款只寫 "SHALL skip files larger than 5 mebibytes"，沒要求 stderr / counter——spec 與 code 都漏了透明度面向。

### F3
`changed_paths_under` 使用 `git diff --name-only` 不指定 `--diff-filter`，git 預設輸出 A/C/D/M/R/T/U/X/B 全部變更類型。「created or modified」的設計意圖（spec.md:450）依賴 caller 自己過濾，但 caller 沒過濾。Test 缺失放大這個 silent regression。

## Proposed Solution

### F2（code + spec）

**Code（`codebus-core/src/vault/raw_sync.rs`）**：
- `SyncSummary` 加新欄位 `oversized_skipped_files: usize`（與既有 `pii_skipped_files` 等命名一致）。
- `sync_with_scanner_into` 在 `meta.len() > MAX_FILE_BYTES` 分支：
  - 寫一行到 `warn_sink`：`mirror skip: oversized at <rel_path> (<N> bytes > 5 MiB limit)`（forward-slash 路徑、與既有 `pii warn:` 平行）
  - `summary.oversized_skipped_files += 1`
- **不動 `walk_source_for_signal`（line 147）**——drift detection 是內部 (file_count, bytes) 計算、無 sink、非 user-facing；warn 加在 sync 路徑就好。兩處 skip 條件相同，source_signal file_count 仍一致。

**Spec（`vault` 的 `Raw Mirror with PII Scanner` MODIFIED）**：
- requirement 大段補入 oversized warn 與 counter 的契約。
- 既有 scenario "Mirror skips files exceeding the size limit" **MODIFIED**：保留 skip 行為宣告，**新增** stderr 行 + counter increment assertion。
- 新增 1 個 scenario 涵蓋 counter 計數正確（兩個 oversized 檔產生 2 條 warn 行與 counter=2）。

### F3（code + test only，no spec change）

**Code（`codebus-core/src/git/nested_repo.rs`）**：
- `changed_paths_under` 的 `git diff` 加 `--diff-filter=ACMR`（白名單：Added / Copied / Modified / Renamed）——明示意圖、未來新增 git filter type（如 T/U/X/B）不會誤包進。
- 不動 untracked path 的 `ls-files --others` 那一段（untracked = 新檔，本來就該包含）。

**Test（同檔 `tests` 模組）**：補既有缺口，新增覆蓋：
- 新建 + 修改檔案 → 包含（A/M）
- 重命名檔案 → 包含（R）
- **刪除檔案 → 排除**（D）— 這是 F3 fix 的核心 assertion
- 未追蹤新檔 → 包含（untracked path）
- 空 diff → 回傳空 list
- subdir filter 正確（diff 範圍受 `subdir` 限制）

**Spec**：`verb-library` spec.md:450 已寫「diffing the vault git repository ... restricted to the wiki/ subtree ... created or modified」——意圖 spec 已對，本 change 是 code↔spec alignment fix，**不改 spec**。

## Non-Goals (optional)

- 不擴 `walk_source_for_signal` 加 warn（drift detection 內部計算，無 user-facing surface；加會被迫改 signature）。
- 不改變 `MAX_FILE_BYTES = 5 MiB` 上限或讓它可配置——本 change 僅補透明度，不調整 policy。
- 不擴 F3 的 diff-filter 為 type-change（T）/ unmerged（U）/ unknown（X）/ broken（B）類型納入考量——wiki 不會產生這些 git 變更類型；白名單 `ACMR` 已涵蓋所有合理 case。
- 不改 `changed_paths_under` 的回傳形狀（仍是 sorted dedup 字串 list）。
- 不擴 F3 修法到 content-verify 流程本身的 error handling——僅修源頭路徑列表正確性。
- 不引入 git command builder 抽象——既有 `capture_git` 直接傳 argv 即可。

## Success Criteria

- `cargo test -p codebus-core vault::raw_sync` 既有測試 + 新增 oversized 測試（含 warn + counter assertion）全綠。
- `cargo test -p codebus-core git::nested_repo` 既有 init/auto_commit 測試 + 新增 `changed_paths_under` 測試模組（A/M/R/D/untracked/empty/subdir 共 5-7 條）全綠。
- `cargo test --workspace` 全綠。
- `spectra validate core-quality-residuals` 通過。
- `vault` spec 的 `Raw Mirror with PII Scanner` 條款包含 oversized warn 與 counter 文字；既有 scenario 更新含 stderr 與 counter assertion；新增 counter increment scenario。
- 手動 sanity：在含 6 MiB 檔的 source repo 跑 `codebus init`，stderr 出現 `mirror skip: oversized at <path> (...)` 字樣。

## Impact

- Affected specs:
  - `vault`（`Raw Mirror with PII Scanner` MODIFIED — 條款補 oversized warn + counter、scenario 既有更新 + 新增 1 條）
- Affected code:
  - Modified:
    - codebus-core/src/vault/raw_sync.rs（`SyncSummary` 新欄位 + `sync_with_scanner_into` warn line + counter increment）
    - codebus-core/src/git/nested_repo.rs（`changed_paths_under` 加 `--diff-filter=ACMR` + 新 test 模組）
- Tests:
  - codebus-core/src/vault/raw_sync.rs 既有 test 模組擴增 2-3 條（oversized warn 行格式、counter 計數、既有 `files_over_5_mib_are_skipped` 更新含 warn assertion）
  - codebus-core/src/git/nested_repo.rs 既有 test 模組新增 `changed_paths_under` 整套（5-7 條 A/M/R/D/untracked/empty/subdir）
- 不影響：
  - `walk_source_for_signal`（drift detection 計算路徑保持原狀）
  - PII filter 邏輯、PII 計數欄位
  - codex backend、claude backend、hook 子命令（與此次工作無交集）
  - codebus-app GUI（透過 tauri 命令呼叫 verb library，回傳的 `SyncSummary` 結構為 additive 加欄位、不破壞既有 field 讀取）
- 跨平台：兩條修改皆字串級 predicate 與 git CLI 旗標，無 OS-specific syscall。Windows / macOS / Linux 行為一致（依賴 git CLI 的 portable 行為）。
- 解鎖：本 change merge 後，PR #1 review 開出的 6 條（F1 / F2 / F3 / F4 / D5 / 威脅 C）全數收尾——T6/T7/T8/T9 review 殘餘 backlog 清零。
