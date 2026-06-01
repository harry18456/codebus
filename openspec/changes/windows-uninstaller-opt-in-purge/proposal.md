## Why

`windows-installer-foundation` 出貨後，uninstall 故意保留所有 user data，但留下兩樣使用者通常會想一起清掉、且手動很難找到的東西：(a) Windows Credential Manager 裡的 Azure API 憑證（claude 的 `codebus-claude-azure`、codex 的 `codebus-codex-azure`），(b) Tauri app data（`%LOCALAPPDATA%\com.codebus.app`，含 WebView2 cache）。原始 backlog Q6 的「完全清除選項」就是要在 uninstall 時提供一個 **opt-in** 提示，讓明確同意的使用者連同全域 `~/.codebus` 一起清除；預設仍維持現狀（保留，不丟 wiki/設定）。

## What Changes

- **Uninstaller 新增 opt-in purge 提示**：`NSIS_HOOK_PREUNINSTALL` 在現有移除 PATH 段之外，加一個 `MB_YESNO` MessageBox，問使用者是否一併移除 codebus 設定與已儲存憑證；文案明示「repository 內的 wiki 永不被碰」。
  - 選 **No（預設）** → 行為完全不變（只移 PATH 段 + 程式檔），維持 `windows-installer-foundation` 的安全契約。
  - 選 **Yes** → best-effort 依序清除：(1) keyring 憑證（兩個 provider 的 azure entry）、(2) `%LOCALAPPDATA%\com.codebus.app`、(3) `%USERPROFILE%\.codebus`。
- **新增 `codebus config purge-keys` 子命令**：刪除目前 CLI 無法觸及的 codex keyring entry。Grounding 證實現行 `codebus config delete-key azure` 的 `Profile` enum **只有 `azure` 一個值**，且 `resolve_keyring_service` 只解析 claude 路徑（`agent.providers.claude.azure.keyring_service`，default `codebus-claude-azure`）——**沒有任何 CLI 途徑能刪掉 codex 的 `codebus-codex-azure`**。`purge-keys` 從 config 解析兩個 provider 的 azure keyring service（無設定時退回 well-known 預設名），best-effort 全部刪除、idempotent、永遠 exit 0。Uninstaller 在刪程式檔之前呼叫它一次（PREUNINSTALL timing 保證 `codebus.exe` 仍在）。
- **best-effort 鐵律**：每個清除動作（keyring / app data / global config）失敗或卡住都 **SHALL NOT** 擋住 uninstall —— 用 `nsExec` + timeout、忽略 exit code，比照現有 PATH hook 的 safe-failing 哲學。
- **絕不** hunt 任何 repo 的 vault `.codebus/`（使用者的 wiki，可能 git-tracked，自動刪 = 越權）。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `windows-distribution`: Requirement「Installer and uninstaller never touch user data」MODIFY —— 預設仍 preserve 不變；新增「opt-in purge」語意：使用者明確同意時，uninstall 額外移除全域 `~/.codebus` + 兩個 provider 的 azure keyring 憑證 + `%LOCALAPPDATA%\com.codebus.app` app data，但 **SHALL NOT** 碰任何 vault `.codebus/`。
- `claude-code-config`: Requirement「Config Subcommand For Keyring Management」MODIFY —— 在現有 `set-key` / `get-key` / `delete-key` 之外，新增第四個 action `purge-keys`（無 profile 參數，刪除所有已知 provider 的 azure keyring entry，best-effort + idempotent）。

## Impact

- Affected specs: `windows-distribution`、`claude-code-config`
- Affected code:
  - Modified:
    - `codebus-app/src-tauri/windows/installer-hooks.nsh`（PREUNINSTALL 加 MessageBox + best-effort purge 序列）
    - `codebus-cli/src/commands/config.rs`（新增 `purge-keys` action + codex keyring service resolver）
  - New: (none — 重用既有 `codebus-core` keyring 刪除 helper)
  - Removed: (none)
- 誠實邊界（P3 真機才能驗）：MessageBox 真跳、`codebus.exe` 在刪檔前真跑、Credential Manager 兩 entry 真清掉、app data 與 `~/.codebus` 真刪。可在本環境驗：`.nsh` 經 `makensis` 編譯通過、`purge-keys` CLI 呼叫形式與 idempotent 行為（unit/integration test 走真實 keyring）、`tauri build` 仍產出含新 hook 的 installer。**不可宣稱 purge「能用」直到 P3 真機驗證。**
