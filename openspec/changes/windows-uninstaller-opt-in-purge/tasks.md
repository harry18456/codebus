## 1. CLI `purge-keys` 命令（test-first）

- [x] 1.1 落實 design Decision「新增 `codebus config purge-keys` 而非呼叫 delete-key 兩次」，擴充 spec requirement「Config Subcommand For Keyring Management」的第四個 action：在 `codebus-cli/tests/config_subcommand.rs` 新增 integration test，設兩個 unique keyring service（claude/codex）並各 set 一把 key、跑 `config purge-keys`、斷言兩者皆變 `unset` 且命令 exit 0。驗證目標＝新測試初次執行 FAIL（命令尚未存在）。
- [x] 1.2 在同檔新增 idempotent test：對全空 keyring 跑 `config purge-keys` 斷言 exit 0、無 error；並新增 `config --help` 斷言列出第四個 action `purge-keys`。驗證目標＝兩測試先 FAIL。
- [x] 1.3 在 `codebus-cli/src/commands/config.rs` 的 `ConfigAction` 新增無參數的 `PurgeKeys` variant 與 `run_purge_keys`，落實 design Decision「keyring service 解析從 config 讀、缺則退回預設名」：claude 服務沿用既有 resolver（default `codebus-claude-azure`），codex 服務新增從 `agent.providers.codex.azure.keyring_service` 解析（缺則 `codebus-codex-azure`），各呼叫既有 `delete_azure_key` helper。驗證目標＝1.1/1.2 測試轉 PASS。
- [x] 1.4 落實 purge-keys 的 best-effort + idempotent 失敗語意：keyring backend 不可用、entry 不存在、config 缺漏/解析失敗皆靜默吞掉並 exit 0（不沿用 delete-key 對 config parse error 回 exit 2 的行為）。驗證目標＝新增「config 不存在時退回預設服務名仍 exit 0」測試 PASS，且 `cargo test -p codebus-cli --test config_subcommand` 全綠。

## 2. NSIS uninstaller opt-in purge（與第 1 組不同檔，可平行）

- [x] 2.1 [P] 擴充 spec requirement「Installer and uninstaller never touch user data」的 opt-in 行為：在 `codebus-app/src-tauri/windows/installer-hooks.nsh` 的 `NSIS_HOOK_PREUNINSTALL`，於現有 PATH 移除邏輯之後，落實 design Decision「opt-in MessageBox 加在 PREUNINSTALL、purge 序列 safe-failing」：加 `MB_YESNO`（預設 button = No）詢問是否一併移除設定與憑證，文案明示 repo 內 wiki 永不被碰；No 分支跳過整段、行為與現狀一致。驗證目標＝`.nsh` 經 `makensis` 編譯通過（沿用既有 build 路徑）+ 內容 review 確認 No 分支不動既有行為。
- [x] 2.2 [P] 在 Yes 分支落實 best-effort 三步序列：先 `nsExec` 呼 `"$INSTDIR\bin\codebus.exe" config purge-keys`（刪程式檔前）、再 `RMDir /r "$LOCALAPPDATA\com.codebus.app"`、再 `RMDir /r "$PROFILE\.codebus"`；每步忽略回傳碼，任一失敗/卡住都不中斷 uninstall。驗證目標＝`makensis` 編譯通過 + review 確認三步皆忽略 exit code。
- [x] 2.3 [P] 落實 design Decision「硬性不碰 vault .codebus/」（呼應 spec requirement「Installer and uninstaller never touch user data」的 SHALL NOT）：確認 purge 序列只觸及三個固定全域路徑、無任何檔案系統遍歷或 vault `.codebus/` 操作；確認 silent/unattended uninstall 下 `MB_YESNO` 預設分支等同 No（不 purge）。驗證目標＝內容 review 對照 design Implementation Contract 的 scope boundaries + silent 分支確認。

## 3. Spec 對齊與全量驗證

- [x] 3.1 確認 `claude-code-config` 與 `windows-distribution` 兩 delta spec 與實作一致（purge-keys 四 action、兩 provider 預設服務名、opt-in 三目標、vault 永不碰）。驗證目標＝`spectra validate windows-uninstaller-opt-in-purge` 通過、`spectra analyze` 無 Critical/Warning。
- [x] 3.2 全量回歸與編譯把關：`cargo test -p codebus-cli`、`cargo clippy --workspace`（無新增 warning）、`tauri build`（或既有 Windows installer build 驗證路徑）產出含新 hook 的 installer。驗證目標＝測試全綠、clippy 無新 warning、build 產物含更新後的 `installer-hooks.nsh`。
- [x] 3.3 標註誠實邊界：在 apply 完成回報中明列 P3 真機才能驗的項目（MessageBox 真跳且預設 No、選 Yes 後 Credential Manager 兩 entry 真消失、`%LOCALAPPDATA%\com.codebus.app` 與 `~/.codebus` 真刪、選 No 全保留），不得在本環境宣稱 purge end-to-end「能用」。驗證目標＝回報內容 review 確認邊界清楚標示。
