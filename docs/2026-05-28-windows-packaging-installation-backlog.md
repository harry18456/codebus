# Backlog: Windows 打包 / 安裝流程（app + CLI）

**Date:** 2026-05-28
**Surfaced during:** 4 bug + design audit 全收完、開始考慮 distribution
**Severity:** release readiness（distribution gap）
**Status:** open（待研究）
**Target OS:** Windows first（user 主開發環境）、macOS / Linux 後續

---

## 一句話

codebus 有兩個 binary（`codebus-app` Tauri GUI + `codebus` CLI）、目前都靠 `cargo build` / `cargo tauri dev` 跑、**沒有 user-facing 安裝流程**——end user 拿到怎麼裝、怎麼放 PATH、怎麼更新都還沒設計。

## 現況

| Component | 怎麼跑（dev）| 怎麼裝（end user）|
|---|---|---|
| `codebus-app`（Tauri）| `cargo tauri dev` | ❌ 沒 installer |
| `codebus` CLI | `cargo build` + 手動 PATH | ❌ 沒 distribution |
| Bundle 兩個 | 沒整合 | ❌ |

## 待研究的關鍵問題

### Q1 · Bundle 兩個 binary 還是分開 distribute

- **Bundle 派**：app 內部呼 CLI、user 裝一份就拿到全功能、UX 統一
- **分開派**：CLI 跑得起來不依賴 GUI、開發者 dev workflow 不用裝 GUI、可單獨升級
- 大機率：Bundle（app installer 內含 CLI、PATH 自動加）+ 提供 CLI-only zip 給 power user

### Q2 · Windows installer 機制

| 選項 | Pros | Cons |
|---|---|---|
| **Tauri 內建 bundler**（`tauri build` → `.msi` 或 `.exe`）| Tauri 官方支援、設定在 `tauri.conf.json`、CI 整合容易 | Tauri 對複雜 install 邏輯支援有限（如自訂 PATH 加法）|
| **NSIS**（Nullsoft Scriptable Install System）| 完全可程式化、業界標準、社群成熟 | 額外工具鏈、學習曲線 |
| **Inno Setup** | 設定簡單、台灣開發者熟、UI 自訂佳 | 跟 Tauri 整合需手動 |
| **WiX Toolset → MSI** | 企業環境最佳、Windows 原生 | 學習曲線重 |

→ 推先試 **Tauri 內建 bundler** 走 MSI、不夠用時再評估 NSIS / Inno。

### Q3 · CLI 怎麼放 PATH

- installer 寫 `HKLM\...\PATH` / `HKCU\...\PATH`（per-user 較安全、不需 admin）
- 或：installer 把 CLI 放 `%LOCALAPPDATA%\codebus\bin\` 後 `setx PATH "%PATH%;%LOCALAPPDATA%\codebus\bin"`
- 卸載時要 reverse（PATH 清乾淨）

### Q4 · Code signing

未 signing 的 .exe / .msi 在 Windows 10/11 會跳 SmartScreen 警告「未識別發行者」、user 看到會怕。

| 選項 | 成本 | 效益 |
|---|---|---|
| 不 sign | $0 | SmartScreen 警告、user 體感差 |
| Self-signed cert | $0 | 仍 SmartScreen、僅技術 user 接受 |
| **EV Code Signing Cert**（DigiCert / Sectigo etc.） | $300-500/年 | SmartScreen 立即通過、企業 user 接受 |
| Sigstore（cosign）| $0 | 開發者社群認、一般 user 不識 |

→ solo dev 早期可不 sign + 文件說明「按執行」、後期上對外發布前再買 EV cert。

### Q5 · 版本同步 + auto-update

- app 跟 CLI 版本是否強制鎖同步（如 `app 3.0.0` 一定配 `cli 3.0.0`）
- auto-update 機制：Tauri updater plugin（HTTP / S3 / 自架 server）
- v1 可不做 auto-update、靠 user 手動下載新 installer

### Q6 · Vault / settings 升級時保留

- installer 不能蓋 user `~/.codebus/` 或 vault 內 `.codebus/` 資料
- uninstaller 預設保留 user data（提供「完全清除」選項）
- 跨版本 settings schema migration（若有 breaking change）

### Q7 · Antivirus 相容

- Tauri WebView2 binary 常被各家 antivirus 誤判
- 解：simulate run 真實 Windows + 主流 AV（Defender / Avast / Kaspersky）測一輪
- 若中招 → 提交 false positive report（Defender 可自助、其他要寫信）

## 開放問題（待 user 決策）

1. **Bundle 還是分開？** 推 bundle、user 可決
2. **EV cert 何時買？** 內部 dogfood 階段不買、對外 release 前買
3. **Auto-update 第一版做嗎？** 推 v1 不做、手動下載
4. **預期 user 怎麼拿到 installer？** GitHub Releases page？官網（沒有）？
5. **CLI-only 安裝路線提供嗎？** 推「is、bundle installer 含 CLI、另提供 CLI-only zip」

## 假設 scope（先 bundle + MSI + 無 signing + 無 auto-update）

| Phase | 內容 | 工程量 |
|---|---|---|
| **P1** | Tauri bundler config 通 `tauri build` 出 `.msi`、含 CLI 副檔 | 半天 |
| **P2** | Installer PATH 加 CLI + 卸載 reverse | 半天 |
| **P3** | 真實 Windows 機（無 dev tool）裝 / 卸 / 升級三輪驗 | 半天 |
| **P4** | GitHub Releases workflow（CI build + 上傳 .msi）| 半天 |
| **P5** | README / docs / install 流程文件 | 半天 |

樂觀總計：**2-3 天**（不含 EV cert / auto-update / cross-OS）。

悲觀（含 antivirus 誤判戰、PATH edge case、MSI rollback 機制）：**3-5 天**。

## Out of scope（後續單獨 backlog）

- macOS DMG / Linux deb/rpm/AppImage（cross-OS 階段）
- EV Code Signing 採購流程
- Auto-update server / S3 distribution
- Homebrew / Chocolatey / winget 等 package manager 整合
- 企業 deployment（MSI silent install / Group Policy）

## 何時動

- **不阻塞**：開發階段 `cargo tauri dev` 已 work
- **觸發點**：要給別人用、要 release 1.0、要 dogfood 給非開發者
- 目前 user solo dev 自己用、可後置

## 相關記憶 / 參考

- Tauri docs: https://tauri.app/v1/guides/distribution/publishing
- Windows installer best practices: https://learn.microsoft.com/en-us/windows/win32/msi/
- Past 經驗：codebus 是 Rust workspace（多 binary）、Tauri bundler 要清楚指明 binary list
