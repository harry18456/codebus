## 1. (A) stderr sandbox-denial 分類合流（verb-library: Sandbox Denial Signal Observability）

- [x] 1.1 [P] 為 stderr 行級 denial 分類寫**失敗**單元測試（RED）：把逐行分類抽成可測 helper（吃一個 in-memory reader、回傳命中 `is_sandbox_denial` 的行數，並依 forward 旗標決定轉發/丟棄非命中行）。測試斷言：含一條 curated marker 的 buffer → count==1；`CODEBUS_FORWARD_AGENT_STDERR` 關閉時 count 仍==1（分類獨立於轉發）。驗證：`cargo test -p codebus-core` 新測試先紅。
- [x] 1.2 實作該 helper 並接進 `agent::invoke`：stderr 背景執行緒改逐行讀、回傳 denial 行數於其 `JoinHandle`，主迴圈在既有 join 點把它加總進 `sandbox_denial_count` 後放上 `InvokeReport`；stdout `ToolResult` 來源不變、兩來源相加不去重。行為：stderr-only denial 也計數、outcome 不變（落實 verb-library 的 Sandbox Denial Signal Observability requirement）。驗證：1.1 轉綠且既有 `is_sandbox_denial` / denial 既有測試仍綠。
- [x] 1.3 經 mock spawn binary 寫整合測試：子程序頂層 exit 0、僅在 stderr 印一條 denial marker → `RunLog.sandbox_denial_count` > 0、`outcome` 為 `succeeded`、發出一行 `warning: sandbox-denial`。驗證：`cargo test -p codebus-cli` 對應測試綠（依賴 1.2）。

## 2. (B) vault gate 完整性 lint 規則（lint-feedback-loop: Vault Gate Integrity Check）

- [x] 2.1 [P] 從 `vault::settings` 匯出「必要 hook 期望集」（`Bash`→`codebus hook check-bash`、`Read`→`codebus hook check-read` 兩對 matcher→command），讓 `DEFAULT_SETTINGS_JSON` 與新規則共用單一來源；加 drift-guard 單元測試斷言 `DEFAULT_SETTINGS_JSON` 解析後含且僅含這兩對必要 hook。驗證：`cargo test -p codebus-core` settings 測試綠。
- [x] 2.2 [P] `VaultContext` 新增 `vault_root` 欄位並由 lint 編排層（`wiki::lint`）在 build 時帶入（wiki_root 的父）；既有五條 wiki 規則行為與簽章不受影響。驗證：`cargo build -p codebus-core` + 既有 lint 單元/整合測試全綠。
- [x] 2.3 為 `VaultGateIntegrityRule` 寫**失敗**單元測試（RED），逐一對應 spec scenario：兩 hook 完整→0 issue；`PreToolUse` 清空→1 個 `error`/rule=`vault-gate-integrity`；缺 Bash hook→error 且 message 點名；缺 Read hook→error 且 message 點名；保留兩 hook + user 額外鍵→0 issue；settings 檔缺失/JSON 損毀→error。驗證：`cargo test -p codebus-core` 新測試先紅。
- [x] 2.4 實作 `VaultGateIntegrityRule`（讀 `vault_root/.claude/settings.json`、比對 2.1 的期望集、依失敗條件回 `error` issue、rule id `vault-gate-integrity`、只讀不寫）並在 `factory.rs` 註冊進預設規則集。行為：gate 被改空/缺 hook 時 lint 報一條 error（落實 lint-feedback-loop 的 Vault Gate Integrity Check requirement）。驗證：2.3 全綠 + lint read-only 既有測試仍綠。
- [x] 2.5 為 gate issue 的 path 呈現寫測試後實作輸出層分支：`text` 格式逐字呈現 `.claude/settings.json`（不套 `wiki/` 前綴）、`json` 格式 path 為設定檔絕對路徑；error 計入 `error_count`。驗證：`cargo test -p codebus-core` 對 text/JSON 兩格式的 output 測試綠。
- [x] 2.6 經整合測試：對 `PreToolUse` 被清空的 vault 跑 `codebus lint --format json` → 輸出含 rule=`vault-gate-integrity` 的 error；確認 `codebus fix` 既有 lint precheck 會帶到此 issue。驗證：`cargo test -p codebus-cli` lint/fix flow 對應測試綠（依賴 2.4）。

## 3. 文件與最終驗證

- [x] 3.1 更新 `docs/security.md` §6：把「stderr-only sandbox denial 計 0、誤標 succeeded」與「vault `.claude/settings.json` 可被改、無偵測」兩條從「缺口」改述為「(A) stderr denial 已計入 `sandbox_denial_count`、(B) `codebus lint` 以 `vault-gate-integrity` 規則偵測（偵測非預防）」。驗證：內容 review + `grep` 確認兩段已更新、無殘留「無偵測」表述。
- [x] 3.2 全工作區驗證：`cargo test -p codebus-core` + `cargo test -p codebus-cli` 全綠、`cargo clippy --workspace` 無**新增** warning。驗證：三道指令輸出乾淨。
