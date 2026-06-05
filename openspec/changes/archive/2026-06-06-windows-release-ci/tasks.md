## 1. 撰寫 release workflow

- [x] 1.1 建立 `.github/workflows/release-windows.yml`，設定觸發為 `on.push.tags: ["v*"]` 加 `on.workflow_dispatch`，並在 workflow 或 job 層宣告 `permissions.contents: write`。此即 design 決策「觸發條件為 tag `v*` push 加 workflow_dispatch」的落實。完成標準：檔案存在且 GitHub Actions 不報 "invalid workflow file"（push 後 Actions 頁無 schema 錯誤）；兩個觸發條件與權限皆出現在 YAML 中（內容檢視對齊 design 的 Implementation Contract）。
- [x] 1.2 在該 workflow 的 `build-windows` job（`runs-on: windows-latest`）加入 toolchain 與依賴步驟：`actions/checkout` → `dtolnay/rust-toolchain@stable` → `Swatinem/rust-cache`（workspaces 指向 repo 根）→ `actions/setup-node`（啟用 npm cache、`cache-dependency-path` 指向 `codebus-app/package-lock.json`）→ 在 `codebus-app` 目錄跑 `npm ci`。此即 design 決策「runner 使用 windows-latest 並加 Rust 與 npm 快取」的落實。完成標準：六個步驟依序出現、`npm ci` 的 working-directory 為 `codebus-app`（內容檢視）。
- [x] 1.3 加入 `tauri-apps/tauri-action` 步驟：`projectPath: codebus-app`、`tagName` 綁觸發的 tag ref、`releaseName` 含版本、`releaseDraft: true`，env 帶 `GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}`；不另外加 CLI staging 或修改 bundle 設定（沿用既有 `beforeBuildCommand`）。此落實 design 決策「採用 tauri-apps/tauri-action 而非手刻 tauri build 加手動上傳」與「Release 以 draft 形式建立」。完成標準：tauri-action 步驟參數齊全、draft 為 true、無重複實作 staging（內容檢視對齊 spec「Tag-triggered Windows release build」與「Installer published as a draft GitHub Release」）。

## 2. 釋出前版號同步約定

- [x] 2.1 在 `.github/workflows/release-windows.yml` 檔頭以註解寫入釋出前檢查約定：「推 `v*` tag 前須確認 `Cargo.toml` 的 `workspace.package.version` 與 `codebus-app/src-tauri/tauri.conf.json` 的 `version` 已同步成該 tag 版本（installer 檔名取自 `tauri.conf.json`，與 tag 不自動連動）」。此落實 design 決策「釋出前版號同步檢查（約定，非自動化）」。完成標準：workflow 檔頭含此註解、且明確點名兩個版號來源檔（內容檢視對齊 design）。

## 3. 驗證

- [x] 3.1 以 `workflow_dispatch` 手動觸發一次，確認 `build-windows` job 在 `windows-latest` 成功完成、且產生一個 draft GitHub Release，其 asset 含一個 `*-setup.exe`。完成標準：Actions 頁該 run 綠燈、Releases 頁出現對應 draft、draft 內有 `-setup.exe`（手動驗證；installer payload 內容由既有 `windows-distribution` spec 保證，本 change 不重複驗）。
- [x] 3.2 確認失敗路徑不發佈 Release：檢視 workflow 結構，確保唯一建立 Release 的步驟是建置成功後的 tauri-action，無任何獨立於建置成功的 release 發佈步驟。完成標準：靜態內容檢視確認沒有早於或獨立於建置成功的 release-publish 步驟（對齊 spec「Failed build does not publish a release」）。
