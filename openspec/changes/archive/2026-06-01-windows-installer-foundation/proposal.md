## Why

codebus 有兩個 binary（`codebus-app` Tauri GUI + `codebus` CLI），目前都靠 `cargo build` / `cargo tauri dev` 跑，**沒有 user-facing 安裝流程**：`tauri.conf.json` 的 `bundle.active=false`、沒設 `targets`、也沒把 CLI 納入產物。end user 拿到無法安裝、CLI 也不在 PATH 上。本 change 做打包地基（P1+P2）：讓 `tauri build` 產出一個含 GUI + CLI 兩個 binary 的 Windows installer，並在安裝時把 CLI 加進 per-user PATH、卸載時乾淨還原。

## What Changes

- **P1 — 開啟 bundler 並納入兩個 binary**
  - `bundle.active` 改為 `true`，`bundle.targets` 設為 `nsis`（見 design 決策 1）。
  - `bundle.icon` 從只引用 `icons/icon.png` 改為引用既有完整圖示集（`icons/icon.ico` 等檔案**已存在**於 repo，非本次新產）。
  - CLI binary（`codebus.exe`，來自 pkg `codebus-cli`）透過 `bundle.resources` 納入 installer，落在安裝目錄的 `bin/` 子資料夾（見 design 決策 2；**不**用 `externalBin`）。test-only 的 `mock-claude` binary 絕不打包。
  - 調整 `beforeBuildCommand` 先建出 release CLI 並 staging，讓 `tauri build` 取得到該 resource。
- **P2 — 安裝時加 PATH、卸載時還原**
  - 新增 NSIS installer hook（`.nsh`），在 `NSIS_HOOK_POSTINSTALL` 把安裝目錄下的 `bin/` 加進 **per-user**（HKCU）PATH；在卸載階段（`NSIS_HOOK_PREUNINSTALL`/`POSTUNINSTALL`）只移除本 installer 加入的那段，reverse 乾淨。
  - `bundle.windows.nsis.installMode` 設為 `currentUser`（per-user、免 admin）。
- **不變量（必守）**：installer / uninstaller 絕不讀寫 user 的 `~/.codebus/` 或任何 vault 的 `.codebus/` 資料；uninstall 預設保留 user data，只移除安裝檔與自己加的 PATH 段。

## Non-Goals

設計細節（NSIS vs WiX 取捨、PATH 寫法、build ordering）寫在 design.md 的 Goals/Non-Goals 段，此處不重複；本 change 的 scope 邊界亦見 design。

## Capabilities

### New Capabilities

- `windows-distribution`: codebus 的 Windows 發佈契約 —— `tauri build` 產出的 installer 必須同時含 GUI 與 CLI 兩個 binary、CLI 安裝時加進 per-user PATH、卸載時還原，且絕不碰 user 資料。

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `windows-distribution`
- Affected code:
  - New:
    - `codebus-app/src-tauri/windows/installer-hooks.nsh`（NSIS PATH hook）
  - Modified:
    - `codebus-app/src-tauri/tauri.conf.json`（bundle.active/targets/icon/resources/windows.nsis + beforeBuildCommand）
  - Removed: (none)
- Affected build：`tauri build` 流程新增「先建 release CLI 並 staging」一步（透過 beforeBuildCommand 或 staging 子目錄，細節見 design）。staging 子目錄會加進 `.gitignore`。
- 明確 out of scope（列後續 backlog、本 change 不做）：P3 真機裝/卸/升級驗證、P4 GitHub Releases CI、P5 README/install docs、code signing、auto-update、macOS/Linux、antivirus 誤判處理。
