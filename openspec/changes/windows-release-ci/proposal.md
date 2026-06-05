## Why

codebus 的打包地基已完成：本機 `tauri build` 已能產出含 GUI + CLI 的 Windows NSIS installer（見 `windows-distribution` spec）。但目前**沒有任何自動化發佈管線**——`.github/` 不存在，每次要釋出版本都得在本機手動建置、手動把 `-setup.exe` 上傳到 GitHub Release。本 change 補上 release readiness backlog 的 P4：一個 tag 觸發的 GitHub Actions workflow，自動在 `windows-latest` 重現 `tauri build` 並把 installer 附到 GitHub Release。

## What Changes

- 新增單一 workflow `.github/workflows/release-windows.yml`：
  - **觸發**：push tag `v*`（主要）+ `workflow_dispatch`（手動補跑）。
  - **runner**：`windows-latest`。
  - **toolchain**：checkout → Rust stable toolchain → Rust build cache → Node.js + npm cache → 在 `codebus-app/` 跑 `npm ci`。
  - **建置與發佈**：用官方 `tauri-apps/tauri-action`，設 `projectPath: codebus-app`，讓它跑既有的 `beforeBuildCommand`（`stage-cli.mjs` 會建出 release CLI 並 staging、再 `npm run build`），產出含 GUI + CLI 的 NSIS installer，並建立一個 **draft** GitHub Release、把 `-setup.exe` 附上去。
  - **權限**：job 設 `contents: write`，用內建 `GITHUB_TOKEN` 建立 Release。
- 不引入任何應用程式碼變更；純 CI 設定 + 一份釋出前版號同步的檢查約定（見 design）。

## Capabilities

### New Capabilities

- `release-automation`: codebus 的自動化發佈契約 —— tag `v*` push 或手動觸發時，GitHub Actions 在 Windows runner 重現 `tauri build`、產出含 GUI + CLI 的 NSIS installer，並以 draft GitHub Release 形式把 installer 附上；產物為 unsigned，且 CI 絕不另跑與 installer 內容無關的步驟。

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `release-automation`
- Affected code:
  - New:
    - `.github/workflows/release-windows.yml`
  - Modified: (none)
  - Removed: (none)
- Affected build：新增 GitHub Actions 發佈管線；本機 `tauri build` 流程與既有設定不變。
- 依賴：沿用既有 `codebus-app/src-tauri/tauri.conf.json`（bundle 設定）與 `codebus-app/scripts/stage-cli.mjs`（CLI staging）；不修改它們。
