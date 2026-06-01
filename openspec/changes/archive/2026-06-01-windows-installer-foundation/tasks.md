## 1. P1 — 開啟 bundler 並納入兩 binary

- [x] 1.1 依「決策 5：build ordering — beforeBuildCommand 先建 release CLI 並 staging」，把 `codebus-app/src-tauri/tauri.conf.json` 的 `build.beforeBuildCommand` 改為先 `cargo build -p codebus-cli --release`、複製產出的 `codebus.exe` 到 src-tauri 下的 `bin-staging/codebus.exe`、再 `npm run build`；並把 `bin-staging/` 加進 `.gitignore`。**行為**：`tauri build` 前 staging 出 release CLI exe 供 resources 取用。**驗證**：跑一次 build，確認 `codebus-app/src-tauri/bin-staging/codebus.exe` 存在且 `git status` 不顯示該檔（已被 ignore）。
- [x] 1.2 [P] 依「決策 1：installer 機制選 NSIS 而非 WiX/MSI」與「決策 2：CLI 用 bundle.resources 落 bin/ 子目錄、不用 externalBin」，在 `tauri.conf.json` 設 `bundle.active = true`、`bundle.targets = "nsis"`、`bundle.icon` 指向既有圖示集（含 `icons/icon.ico`）、`bundle.resources` 以 map 把 `bin-staging/codebus.exe` 對映到 `bin/codebus.exe`。**行為**：bundler 啟用、產物為 NSIS、CLI 以 resource 落 `bin/` 子目錄。**驗證**：`tauri build` 在 Windows 成功並於 `target/release/bundle/nsis/` 產出 `-setup.exe`。
- [x] 1.3 驗證「Windows installer bundles GUI and CLI binaries」需求：列出 Tauri 產生的 `installer.nsi` 的 File 指令（payload 權威來源），確認 install root 有 GUI `codebus-app.exe`（Tauri MAINBINARYNAME=app crate bin 名 `codebus-app`，非 productName）、`bin\codebus.exe` 有 CLI、product version=`3.0.0`、identifier=`com.codebus.app`，且 `mock-claude` **不**在 payload。**驗證**：列出 File 指令清單作為證據比對上述四點。

## 2. P2 — 安裝時加 PATH、卸載時還原

- [x] 2.1 依「決策 4：PATH 寫法用原生 HKCU registry、不引入 EnVar 第三方 plugin」與「決策 3：安裝範圍 per-user（installMode currentUser、HKCU PATH）」，新增 `codebus-app/src-tauri/windows/installer-hooks.nsh`，在 `NSIS_HOOK_POSTINSTALL` 以原生 registry 讀 HKCU `Path`、若未含 `$INSTDIR\bin` 才 `WriteRegExpandStr` append（idempotent）、廣播 `WM_SETTINGCHANGE`。**行為**：滿足「Installer adds CLI to per-user PATH and reverses on uninstall」需求的 install 半邊（per-user、idempotent）。**驗證**：審查 `.nsh` 內含 substring 檢查與 HKCU 寫入、無 HKLM 寫入、無第三方 plugin `!addplugindir`。
- [x] 2.2 在同一 `installer-hooks.nsh` 的 `NSIS_HOOK_PREUNINSTALL`（或 POSTUNINSTALL）只移除 `$INSTDIR\bin` 那段 PATH 並再廣播；hook 全程不讀寫 `~/.codebus/` 或任何 `.codebus/` 路徑。**行為**：滿足「Installer adds CLI to per-user PATH and reverses on uninstall」的 reverse 半邊，並滿足「Installer and uninstaller never touch user data」。**驗證**：審查 uninstall 段為精準段移除（非整條清空）、且 grep hook 內無任何 `.codebus` 路徑引用。
- [x] 2.3 在 `tauri.conf.json` 設 `bundle.windows.nsis = { installMode: "currentUser", installerHooks: "windows/installer-hooks.nsh" }`，把 hook 接上 installer。**行為**：build 出的 `-setup.exe` 內嵌該 hook 且為 per-user 模式。**驗證**：`tauri build` 後解開 payload 確認 hook script embedded、installer 為 currentUser 模式（不要求 admin）。

## 3. 驗證分層與誠實回報

- [x] 3.1 整理證據並分層回報：**已驗（本環境）**＝`tauri build` 產出 `-setup.exe` + payload 內容（兩 binary、hook embedded、version/identifier、無 mock-claude）；**待 user 真機驗（P3、本 change 不做）**＝實際安裝後 HKCU PATH 生效、新終端機 `codebus --version` 可跑、卸載 reverse 乾淨、user data 保留。**驗證**：回報明列兩層，不宣稱 installer「能用」。
