## Context

codebus 的 Windows 打包地基（`windows-distribution` capability）已完成：`tauri.conf.json` 設 `bundle.active=true`、`targets=nsis`、`installMode=currentUser`，`beforeBuildCommand` 為先跑 `stage-cli.mjs`（`cargo build -p codebus-cli --release` + 複製到 `bin-staging/`）再 `npm run build`。因此本機 `tauri build` 已能一鍵產出含 GUI + CLI 的 NSIS installer。

目前缺的是「把這條本機路徑搬到 CI 並對外發佈」。repo 根目前**沒有 `.github/` 目錄**，這是 greenfield。套件管理為 npm（`codebus-app/package-lock.json` 存在）。workspace 版本為 `3.0.1`，`repository = github.com/harry18456/codebus`。本 change 對應 release readiness backlog 的 P4（`2026-05-14-github-repo-setup`），但範圍刻意縮到「只做 Windows release build」。

## Goals / Non-Goals

**Goals:**

- 新增單一 workflow，使 push tag `v*` 或手動觸發時，在 `windows-latest` 重現 `tauri build` 並產出含 GUI + CLI 的 NSIS installer。
- installer 以 draft GitHub Release 形式發佈，附上 `-setup.exe`，保留人工把關後再 publish。
- CI 沿用既有 `beforeBuildCommand`，不重複 CLI staging 邏輯、不修改既有打包設定。

**Non-Goals:**

- **三平台 CI matrix / 跨平台 release**（macOS/Linux）——`tauri.conf.json` 只設 nsis，本 change 僅 Windows。
- **CI test workflow**（`cargo test` / `tsc` / vitest 的 PR gate）——屬 backlog 的 CI 部分，與 release build 概念不同，本 change 不做。
- **Code signing**——產物為 unsigned，SmartScreen 會對下載者跳警告；憑證與 secret 管理是另一層 scope。
- **版號自動注入**——本 change 以「釋出前手動同步版號」的檢查約定兜底，不做 tag→`tauri.conf.json` 自動 bump。
- **issue/PR template、branch protection、auto-update、Dependabot**——皆不在範圍。

## Decisions

### 採用 tauri-apps/tauri-action 而非手刻 tauri build 加手動上傳

用官方 `tauri-apps/tauri-action`：它接受 `projectPath: codebus-app`、自動執行專案的 `beforeBuildCommand`、定位 bundle 產物、建立 GitHub Release 並把 installer 附上。

- **替代方案（已否決）**：手動 `npm ci` → `npm run tauri build` → 用 `softprops/action-gh-release` 撈 `codebus-app/src-tauri/target/release/bundle/nsis/*-setup.exe` 上傳。控制更細，但要自己維護產物路徑 glob 與 Release 建立邏輯，維護成本對 solo dev 不划算。
- **理由**：tauri-action 是 Tauri 官方維護、與 bundler 產物路徑同步演進，降低路徑漂移風險。

### 觸發條件為 tag `v*` push 加 workflow_dispatch

主要觸發是 push 符合 `v*` 的 tag（對應版號釋出）；同時提供 `workflow_dispatch` 手動鈕，供補跑或測試。

- **替代方案（已否決）**：push main 出 nightly——會頻繁建置耗 runner 時間、且 solo dev 不需要每次 push 都有產物。
- **理由**：tag-driven 是 release 慣例，手動鈕補上失敗重跑的彈性。

### Release 以 draft 形式建立

tauri-action 設 `releaseDraft: true`，產出的 Release 為 draft，需人工檢視後才 publish。

- **理由**：保留把關時機——確認版號一致、補上 SmartScreen 警告說明與安裝指引，再公開。unsigned 產物尤其需要這層人工說明。

### runner 使用 windows-latest 並加 Rust 與 npm 快取

`windows-latest` 自帶 WebView2 與 MSVC；NSIS 由 Tauri bundler 自動下載。toolchain 步驟：`actions/checkout` → `dtolnay/rust-toolchain@stable` → `Swatinem/rust-cache`（快取整個 workspace，含體積大的依賴）→ `actions/setup-node`（啟用 npm cache、指向 `codebus-app/package-lock.json`）→ 在 `codebus-app/` 跑 `npm ci`。

- **理由**：edition 2024 需 Rust ≥1.85，runner 的 stable 遠超過；rust-cache 避免每次冷編譯造成數十分鐘建置。

### 釋出前版號同步檢查（約定，非自動化）

installer 檔名版本取自 `tauri.conf.json` 的 `version`（目前 `3.0.1`），**不是** git tag。若 tag 推 `v3.1.0` 但忘了同步 `tauri.conf.json` 與 `Cargo.toml` 的 `version`，Release 名與檔名版本會不一致。本 change 以一條釋出前檢查約定兜底（寫進 tasks 的釋出操作說明），不引入自動注入機制。

## Implementation Contract

**Behavior（operator 觀察到的）：**

- 當 push 一個符合 `v*` 的 tag（例如 `v3.0.1`）到 `github.com/harry18456/codebus`，或在 Actions 頁手動 dispatch `release-windows` workflow 時：
  - 一個跑在 `windows-latest` 的 job 啟動，完成 Rust + Node toolchain 準備與 `npm ci`。
  - tauri-action 跑既有 `beforeBuildCommand`（staging release CLI + 前端 build），再 `tauri build` 產出 NSIS `-setup.exe`（payload 含 install root 的 `codebus-app.exe` 與 `bin/codebus.exe`，不含 `mock-claude`，與 `windows-distribution` spec 一致）。
  - 建立一個 **draft** GitHub Release（tag 為觸發的 ref，名稱含該 tag），並把 `-setup.exe` 附為 release asset。
- workflow_dispatch 觸發時行為相同；其 Release 的 tag 取當前 dispatch 的 ref（branch）對應的 tag 設定，由 tauri-action 的 `tagName` 參數決定。

**Interface / 設定形狀：**

- 檔案：`.github/workflows/release-windows.yml`，單一 job（名稱如 `build-windows`），`runs-on: windows-latest`。
- 觸發：`on.push.tags: ["v*"]` + `on.workflow_dispatch`。
- 權限：job 或 workflow 層設 `permissions.contents: write`，使用內建 `secrets.GITHUB_TOKEN`（不需自建 PAT）。
- 步驟順序：checkout → rust-toolchain(stable) → rust-cache → setup-node(npm cache) → `npm ci`（working-directory `codebus-app`）→ tauri-action（`projectPath: codebus-app`、`tagName` 綁觸發 tag、`releaseName` 含版本、`releaseDraft: true`、env 帶 `GITHUB_TOKEN`）。

**Failure modes：**

- 任一步驟（toolchain、`npm ci`、`tauri build`）失敗則 job 失敗，**不**建立或發佈 Release；不得產出半成品 Release。
- tauri-action 找不到 bundle 產物時視為建置失敗，job 失敗。

**Acceptance criteria：**

- workflow YAML 通過 GitHub Actions schema（push 後 Actions 頁無 "invalid workflow file" 錯誤）。
- 在 fork 或實際 repo 以 `workflow_dispatch` 觸發一次，job 成功完成、產生一個 draft Release 且其 asset 含一個 `*-setup.exe`。
- installer 內容由既有 `windows-distribution` spec 保證（本 change 不重複驗 payload，只驗「CI 能產出並附上 installer」）。

**Scope boundaries：**

- 範圍內：唯一新增檔 `.github/workflows/release-windows.yml`，以及一條釋出前版號同步檢查的書面約定。
- 範圍外：不改 `tauri.conf.json`、`stage-cli.mjs`、任何應用程式碼；不加 test/lint CI；不做 signing；不碰 macOS/Linux。

## Risks / Trade-offs

- [unsigned installer 觸發 SmartScreen 警告] → 在 draft Release 說明中標注「unsigned、首次執行需點『更多資訊→仍要執行』」；signing 列後續 backlog。
- [tag 與 `tauri.conf.json` 版號不一致導致 Release 名與檔名版本不符] → 釋出前檢查約定（tasks 中明列）；日後可升級為自動注入。
- [Rust 冷編譯（含大型依賴）使 CI 緩慢] → `Swatinem/rust-cache` 快取 workspace；首次仍會慢，屬可接受一次性成本。
- [tauri-action 版本演進改變參數] → pin 到 major 版本（`@v0`），升級時於 PR 驗證一次再合。
- [`workflow_dispatch` 在非 tag ref 觸發時 Release tag 語意較弱] → 主要釋出路徑仍為 tag push；手動觸發定位為補跑/測試，不保證對外正式發佈。
