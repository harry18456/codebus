## Context

codebus 是 Rust workspace，產兩個 binary：GUI = `codebus-app`（pkg `codebus-app-tauri`，Tauri **v2**，`tauri.conf.json` 的 `$schema` 為 `https://schema.tauri.app/config/2`），CLI = `codebus`（pkg `codebus-cli`，另有 test-only `mock-claude` 不可打包）。workspace version 鎖 `3.0.0`，app 與 CLI 同版，**版本同步不需處理**。

現況 `bundle = { active: false, icon: ["icons/icon.png"] }`，沒 `targets`、沒 `externalBin`/`resources`。

**Grounding 校正**：
1. backlog doc（`docs/2026-05-28-windows-packaging-installation-backlog.md`）的 Tauri 連結是 **Tauri v1**、stale；本設計一律以 Tauri v2 官方 distribution / config 文件為準。
2. 圖示集**已存在**：`codebus-app/src-tauri/icons/` 內已有 `icon.ico`、`32x32.png`、`64x64.png`、`128x128.png`、`128x128@2x.png` 及完整 Square logo 系列（先前已跑過 `tauri icon`）。本 change **不需**重新產圖，只需把 `bundle.icon` 指向既有檔案；P1「補圖示」其實已備齊。
3. **主程式名與 PATH 隔離**（apply 階段以 build 產物 grounding 校正）：Tauri 的 `MAINBINARYNAME` 取 **app crate 的 bin 名 `codebus-app`**（非 `productName` "codebus"），故 GUI 安裝後是 `$INSTDIR\codebus-app.exe`；CLI bin 名是 `codebus` → `codebus.exe`。兩者檔名本就不同（`codebus-app.exe` vs `codebus.exe`），**無檔名覆蓋衝突**——先前 propose 假設的「同名衝突」前提不成立。但把 CLI 放進 `bin/` 子目錄的決策**仍成立**，理由改為：PATH 只加 `bin/`，讓**只有 CLI** 上 PATH、不把 GUI 目錄（含 `codebus-app.exe`、resources、WebView2 bootstrap 等）整包曝露到 PATH。這驅動決策 2。

## Goals / Non-Goals

**Goals:**

- `tauri build` 在 `bundle.active=true` 下產出 Windows installer（NSIS `-setup.exe`），payload 同時含 GUI（`codebus-app.exe`）與 CLI（`bin/codebus.exe`）。
- installer 安裝時把 CLI 所在的 `bin/` 加進 per-user（HKCU）PATH；卸載時只移除自己加的那段，reverse 乾淨。
- 全程 per-user、免 admin。
- installer/uninstaller 絕不碰 `~/.codebus/` 或 vault 的 `.codebus/`；uninstall 預設保留 user data。

**Non-Goals（本 change 不做、列後續 backlog）:**

- P3 真機裝/卸/升級三輪驗證（需乾淨 Windows 機，由中控 user 手動執行）。
- P4 GitHub Releases CI（build + 上傳）、P5 README/install docs。
- Code signing（SmartScreen）、auto-update（Tauri updater plugin）。
- macOS DMG / Linux deb/rpm/AppImage。
- antivirus 誤判處理、企業 silent install / Group Policy。
- 提供 CLI-only zip 路線（backlog Q1 的 power-user 分支）。

## Decisions

### 決策 1：installer 機制選 NSIS 而非 WiX/MSI

判準＝P2 的「per-user 自動加 PATH + 卸載 reverse 哪個乾淨」。

- **NSIS（選用）**：Tauri v2 提供 `bundle.windows.nsis.installerHooks`，指向一個 `.nsh` 檔，內含四個生命週期 macro（`NSIS_HOOK_PREINSTALL` / `NSIS_HOOK_POSTINSTALL` / `NSIS_HOOK_PREUNINSTALL` / `NSIS_HOOK_POSTUNINSTALL`），可在安裝/卸載各階段插入**任意 NSIS script**——PATH 加法與還原完全可程式化。且 `installMode: "currentUser"` 是 Tauri v2 NSIS 的**預設值**：免 admin、裝到 `%LOCALAPPDATA%`、metadata 寫 HKCU。產物為 `-setup.exe`。
- **WiX/MSI（不選）**：MSI 雖有原生 `Environment` table 能宣告式管 PATH 並由 MSI 交易式還原，但 Tauri v2 對 WiX 的客製較**剛性**——只能透過 `fragmentPaths`（`.wxs` 片段）+ `componentRefs` 注入，且官方文件**未提供 PATH 範例**；加上 MSI per-user 在 Windows 上歷來 admin-finicky（perMachine 才順）。對「per-user + 免 admin + 乾淨 reverse」這組判準，NSIS hook 的可程式化勝出。

證據：Tauri v2 Windows Installer 文件（installerHooks 四 macro、installMode currentUser 為預設）、Tauri v2 config schema（`bundle.windows.nsis` / `bundle.windows.wix` 的 sub-keys）。

### 決策 2：CLI 用 bundle.resources 落 bin/ 子目錄、不用 externalBin

- **不用 `externalBin`**：`externalBin`（sidecar）在 build 時要求檔名帶 target-triple（如 `codebus-x86_64-pc-windows-msvc.exe`），語意是「給 app 內部呼叫的副程式」，安裝後會還原成 `codebus.exe` 落在**主程式同層**——正好撞上 GUI 自己的 `codebus.exe`（見 Context 同名衝突）。不適合「裝到 PATH 給人在終端機用」。
- **用 `bundle.resources`（選用）**：以 map 形式把 staging 出來的 CLI exe 對映到安裝目錄的 `bin/codebus.exe`。`resources` 接受 `string[]` 或 source→target map，支援子目錄目標。CLI 落在 `$INSTDIR\bin\codebus.exe`，與 GUI 的 `$INSTDIR\codebus.exe` 分層、無衝突；且 PATH 只加 `bin/` 子目錄 → GUI exe **不**進 PATH。

### 決策 3：安裝範圍 per-user（installMode currentUser、HKCU PATH）

- `bundle.windows.nsis.installMode = "currentUser"`（Tauri v2 確切 enum 值，非 `perUser`；另兩值為 `perMachine` / `both`，後兩者要 admin）。
- PATH 寫 HKCU，免 admin，對 solo distribution 安全。

### 決策 4：PATH 寫法用原生 HKCU registry、不引入 EnVar 第三方 plugin

NSIS 改 PATH 兩條路：

- **EnVar plugin**（`EnVar::AddValue` / `EnVar::DeleteValue`，可 `EnVar::SetHKCU`）：robust、處理去重與長度上限，但 **Tauri NSIS 不預設 bundle 此 plugin**，需把 `EnVar.dll`（含 x86/x64/unicode 變體）commit 進 repo 並 `!addplugindir` 引用——對 security-conscious 的本專案是引入**第三方 prebuilt binary blob** 的供應鏈疑慮。
- **原生 HKCU registry（選用）**：純 NSIS script，無 binary 依賴、完全可審：在 `NSIS_HOOK_POSTINSTALL` 讀 `HKCU\Environment` 的 `Path`、若未含目標段才 append（idempotent）、`WriteRegExpandStr`（REG_EXPAND_SZ）寫回，再以 `System::Call` 廣播 `WM_SETTINGCHANGE`（`SendMessageTimeout HWND_BROADCAST`）讓新開的 shell 立即生效；卸載時讀回、**只移除本 installer 加的那一段**、寫回並再廣播。

選原生 registry：可審、無 binary blob、與本專案不過度宣稱/可驗證的取向一致。已知限制見 Risks（PATH 過長截斷），用 Tauri NSIS 的 large-string build 緩解。

### 決策 5：build ordering — beforeBuildCommand 先建 release CLI 並 staging

`tauri build` 只建 app crate，**不**建 CLI。需在 bundling 前讓 CLI exe 就位給 `resources` 取用。

- 現況 `beforeBuildCommand = "npm run build"`。改為先 `cargo build -p codebus-cli --release`、把產出的 `codebus.exe` 複製到 src-tauri 下一個 staging 子目錄（如 `bin-staging/codebus.exe`），再 `npm run build`。
- `resources` 以 map 引用 staging 出的本地相對路徑（避免 `..` 跨出 src-tauri 的 resource 解析疑慮）。
- staging 子目錄加進 `.gitignore`（產物、非 source）。

## Implementation Contract

**Behavior（ship 後可觀察）：**

- 在乾淨 Windows 上 `cargo tauri build`（或 `npm run tauri build`）成功，於 `target/release/bundle/nsis/` 產出 `codebus_3.0.0_x64-setup.exe`（檔名格式依 Tauri NSIS 慣例）。
- 該 `-setup.exe` 的 payload 解開後含：主程式 `codebus-app.exe`（GUI）、`bin\codebus.exe`（CLI）、embedded 的 installer hook script、正確的 product version `3.0.0` 與 identifier `com.codebus.app`。
- （待真機驗，見 Acceptance）安裝後 `bin\` 進 HKCU PATH、新開終端機 `codebus --version` 可跑；卸載後該 PATH 段移除、user data 保留。

**Interface / 設定形狀（`tauri.conf.json` 的 `bundle`）：**

- `active: true`
- `targets: "nsis"`
- `icon`: 引用既有 `icons/` 圖示集（含 `icons/icon.ico`）
- `resources`: map，把 staging CLI exe → `bin/codebus.exe`
- `windows.nsis`: `{ installMode: "currentUser", installerHooks: "windows/installer-hooks.nsh" }`
- `build.beforeBuildCommand`: 先建 release CLI + staging，再 `npm run build`

**NSIS hook（`codebus-app/src-tauri/windows/installer-hooks.nsh`）契約：**

- `NSIS_HOOK_POSTINSTALL`：idempotent 把 `$INSTDIR\bin` 加進 HKCU `Path`、廣播 `WM_SETTINGCHANGE`。
- `NSIS_HOOK_PREUNINSTALL`（或 POSTUNINSTALL）：只移除 `$INSTDIR\bin` 那一段、廣播 `WM_SETTINGCHANGE`。
- 絕不讀寫 `~/.codebus/` 或任何 `.codebus/` vault 路徑。

**Failure modes：**

- CLI 未先建 → `resources` 找不到 staging 檔，`tauri build` 失敗（fail-loud，非靜默）。
- PATH 已含目標段 → 不重複 append（idempotent，避免汙染）。

**Acceptance criteria：**

- 本環境可驗（**已驗** tier）：`cargo tauri build` 在 Windows 成功產出 `-setup.exe`；解開/列出 payload 確認 `codebus.exe` + `bin\codebus.exe` 兩 binary 在、hook script embedded、version/identifier 正確；`mock-claude` **不**在 payload。
- 需乾淨 Windows 機真機驗（**待 user** tier、本 change 不做、屬 P3）：實際安裝後 PATH 生效、`codebus` 終端機可呼叫、卸載 reverse 乾淨、user data 保留。

**Scope boundaries：**

- In scope：`tauri.conf.json` bundle 設定、新增 NSIS hook、beforeBuildCommand 調整、`.gitignore` 補 staging 目錄。
- Out of scope：決策外的 CI、docs、signing、auto-update、跨 OS、真機行為驗證（見 Non-Goals）。

## Risks / Trade-offs

- [原生 registry 寫 PATH，若既有 PATH 超過 NSIS 字串上限會截斷/汙染] → Tauri NSIS 使用 large-string build（`NSIS_MAX_STRLEN` 放大）緩解；append 前先做 substring 檢查確保 idempotent；卸載只做精準段移除。若日後遇真實截斷，再評估改用 EnVar plugin（決策 4 的備案）。
- [`resources` 以 `..` 跨出 src-tauri 解析可能不穩] → 用 beforeBuildCommand 先 copy 到 src-tauri 內的 staging 子目錄，`resources` 只引用本地相對路徑，迴避此風險（決策 5）。
- [本環境無法驗真實安裝/PATH/卸載行為] → 嚴格區分「已驗：build 產出 + payload 內容」vs「待 user 真機驗：安裝行為」，回報不宣稱 installer「能用」。
- [WebView2 runtime] → Tauri NSIS 預設處理 WebView2 bootstrap；本 change 不另客製，沿用預設（`preInstalledWebview2` 等保持預設）。
