## Context

`windows-installer-foundation`（spec `windows-distribution`）已出貨：NSIS uninstaller 的 `NSIS_HOOK_PREUNINSTALL` 目前只 surgical 移除它加進 HKCU PATH 的那一段，刻意不碰任何 user data。這留下兩樣使用者通常想一起清、又很難手動找到的殘留：

- **keyring 憑證**：Azure API key 存在 Windows Credential Manager，service 名為 `codebus-claude-azure`（claude，code default，`codebus-cli/src/commands/config.rs`）與 `codebus-codex-azure`（codex，code default，`codebus-core/src/config/codex.rs`）；只有 azure profile 會有 entry，system profile 無。
- **Tauri app data**：identifier `com.codebus.app` → `%LOCALAPPDATA%\com.codebus.app`（含 WebView2 cache）。

Grounding 揭露的關鍵約束：現行 `codebus config delete-key` 的 `Profile` enum **只有 `Azure` 一個值**（`config_subcommand.rs` 的 `unknown_profile_value_rejected_by_clap` 測試證實 `bedrock` 被 clap 拒），且 `resolve_keyring_service` 只走 claude 路徑（解析 `agent.providers.claude.azure.keyring_service`）。因此 `codebus config delete-key azure` 只能清 **claude** 憑證；**現行 CLI 沒有任何途徑刪掉 codex 的 `codebus-codex-azure`**。`config_subcommand.rs` 也證實 delete-key 對新 `agent.providers` schema config 正常運作、且對不存在的 entry idempotent exit 0。

## Goals / Non-Goals

**Goals:**

- uninstall 時提供一個 **opt-in** 提示；只有使用者明確選 Yes 才執行完全清除。
- 完全清除涵蓋：兩個 provider 的 azure keyring 憑證、`%LOCALAPPDATA%\com.codebus.app`、`%USERPROFILE%\.codebus`。
- 每個清除步驟都 best-effort：失敗/卡住絕不擋住 uninstall。
- 預設（選 No）行為與 `windows-installer-foundation` 完全相同。

**Non-Goals:**

- **不** hunt 或刪除任何 repo 的 vault `.codebus/`（使用者 wiki，可能 git-tracked，自動刪 = 越權）—— 這是硬性 SHALL NOT。
- **不**改變 install 流程、PATH 處理、或 No 分支的任何既有行為。
- **不**在 P3 真機驗證前宣稱 purge「能用」。
- **不**為 codex 引入完整的 provider-dimension keyring CLI（如 `delete-key codex`）；只加滿足 uninstaller 需求的最小命令。

## Decisions

### Decision: 新增 `codebus config purge-keys` 而非呼叫 delete-key 兩次

原 backlog 傾向「reuse 現成 `delete-key` 呼 claude + codex 兩次」以避免新 CLI surface。Grounding 證實此路**不可行**：`Profile` enum 只有 `azure`，沒有 codex 維度，無法 `delete-key codex`。可選方案：

- **(A，採用)** 新增單一 action `codebus config purge-keys`（無 profile 參數）：從 config 解析兩個 provider 的 azure keyring service（無設定時退回 well-known 預設名 `codebus-claude-azure` / `codebus-codex-azure`），對每個 service 呼叫既有的 `delete_azure_key` helper，best-effort、idempotent、永遠 exit 0。Uninstaller 呼叫一次即覆蓋全部。
- (B，否決) 把 `codex` 加進 `Profile` enum，讓 `delete-key codex` 解析 codex service，uninstaller 呼兩次。否決理由：`azure` 是「profile 類型」、`codex` 是「provider」，兩者並列為 sibling enum value 語意不一致，且會擴大既有 `delete-key` requirement 的 surface 與測試面。
- (C，否決) uninstaller 只用 `delete-key azure` 清 claude、放著 codex 憑證不管。否決理由：與「完全清除」目標矛盾，留下使用者以為已清掉的憑證 = 安全誤導。

(A) 是最小但正確：一個自足的命令，恰好對應 uninstaller「移除所有 saved credentials」的需求，未來新增 provider 也能集中在此擴充。

### Decision: keyring service 解析從 config 讀、缺則退回預設名

`purge-keys` 對每個 provider 嘗試解析 `agent.providers.<provider>.azure.keyring_service`；若 config 不存在、解析失敗、或該 azure 區塊缺 `keyring_service`，則退回該 provider 的 code default 預設名。理由：使用者可能曾覆寫 `keyring_service`（需刪對的 entry），也可能 active=system 而 azure 區塊是 cold storage 甚至缺漏（仍可能殘留舊 entry，需用預設名嘗試刪）。purge-keys 對「entry 不存在」與「config 缺漏」皆 best-effort 吞掉、不報錯。

### Decision: opt-in MessageBox 加在 PREUNINSTALL、purge 序列 safe-failing

在現有 PATH 移除邏輯之後，加 `MB_YESNO`（預設 button = No）。文案需明示清除範圍且強調 repo 內 wiki 永不被碰，例如：「Also remove your codebus settings and saved credentials? Your wikis inside repositories are never touched.」選 No → 跳過整段。選 Yes → 依序：

1. 先呼 `"$INSTDIR\bin\codebus.exe" config purge-keys`（**在刪程式檔前**，PREUNINSTALL timing 保證 `codebus.exe` 還在）清 keyring。
2. `RMDir /r "$LOCALAPPDATA\com.codebus.app"` 清 app data。
3. `RMDir /r "$PROFILE\.codebus"` 清全域 config + log。

每個外部呼叫用 `nsExec::ExecToStack`（或等價）並忽略回傳碼，確保任何失敗/卡住都不擋 uninstall —— 比照既有 PATH hook 的 safe-failing 哲學（hook 註解 decision 3/4：safe-failing、never corrupt）。

### Decision: 硬性不碰 vault .codebus/

purge 序列只觸及三個固定全域位置（keyring、`%LOCALAPPDATA%\com.codebus.app`、`%USERPROFILE%\.codebus`）。**絕不**遍歷檔案系統尋找各 repo 的 vault `.codebus/`。這是 spec 層的 SHALL NOT，不只是實作選擇。

## Implementation Contract

**Behavior（end-user / operator 觀察）:**

- 安裝過 codebus 的使用者執行 uninstall → 看到一個 Yes/No 對話框詢問是否一併移除設定與憑證，預設聚焦 No。
- 選 **No**（或對話框因任何原因失敗）→ 只有 PATH 段與程式檔被移除；`~/.codebus`、所有 vault `.codebus/`、keyring 憑證、app data 全數保留（與現狀相同）。
- 選 **Yes** → 額外移除：Credential Manager 內 `codebus-claude-azure` 與 `codebus-codex-azure` 的 `default` entry、`%LOCALAPPDATA%\com.codebus.app`、`%USERPROFILE%\.codebus`；任一步失敗都不中斷 uninstall。
- 無論選項為何，任何 repo 的 vault `.codebus/` 都不被讀取或刪除。

**Interface / data shape:**

- 新 CLI action：`codebus config purge-keys`（無位置參數、無旗標）。stdout 行為比照其他 config action 的簡潔風格；exit code 永遠 0。
- 解析的 keyring service：claude = `agent.providers.claude.azure.keyring_service`（缺則 `codebus-claude-azure`）；codex = `agent.providers.codex.azure.keyring_service`（缺則 `codebus-codex-azure`）。keyring account 為既有固定字面 `default`。
- 重用既有 `codebus-core` keyring 刪除 helper（`delete_azure_key(service)`），不新增 keyring 抽象。
- NSIS：`NSIS_HOOK_PREUNINSTALL` 內，PATH 移除後新增 `MB_YESNO` 分支與三步 best-effort 序列。

**Failure modes:**

- `purge-keys`：keyring backend 不可用、entry 不存在、config 缺漏/解析失敗 → 一律靜默吞掉、exit 0（best-effort + idempotent）。
- NSIS：`codebus.exe` 不存在、purge-keys 非 0、`RMDir` 失敗（檔案鎖定等）→ 忽略，繼續 uninstall 到完成。
- MessageBox 在 silent uninstall 情境的處置：應視為 No（不 purge），避免 silent 模式誤刪資料；apply 時確認 NSIS silent 旗標下的 default 分支。

**Acceptance criteria:**

- 本環境可驗：(1) `codebus config purge-keys` 存在於 `config --help` 且無 profile 參數；(2) integration test 走真實 keyring 驗證 —— 設兩個 unique service、各 set 一把 key、跑 purge-keys、兩者皆變 unset、且對全空 keyring 再跑一次仍 exit 0（idempotent）；(3) `.nsh` 經 `makensis` 編譯通過；(4) `tauri build` 產出含新 hook 的 installer（沿用 `windows-installer-foundation` 既有 build 驗證路徑）。
- P3 真機才能驗（誠實邊界，不可在本環境宣稱通過）：MessageBox 真跳且預設 No、選 Yes 後 Credential Manager 兩 entry 真消失、`%LOCALAPPDATA%\com.codebus.app` 與 `~/.codebus` 真刪、選 No 時全保留。

**Scope boundaries:**

- In scope：`installer-hooks.nsh` 的 PREUNINSTALL 擴充、`config.rs` 的 `purge-keys` action + codex service resolver、兩個 spec 的 MODIFY、purge-keys 的測試。
- Out of scope：install 流程、PATH 邏輯、No 分支既有行為、vault `.codebus/` 任何處理、為 codex 加 set-key/get-key/delete-key 的完整 provider 維度、macOS/Linux uninstall。

## Risks / Trade-offs

- [purge-keys 在 active=system 時讀不到 cold-storage azure 的覆寫 service 名] → 退回 well-known 預設名嘗試刪除；涵蓋絕大多數（未覆寫）情境。若使用者覆寫了 keyring_service 又把 azure 設為非 active 且 config 不再保留該欄位，極端情況下可能漏刪一把 key —— 視為已知限制，於 spec/design 誠實標註，不擋本 change。
- [PREUNINSTALL 中 `codebus.exe` 可能因防毒/檔案鎖無法執行] → best-effort 忽略，keyring 憑證在此情況保留；不中斷 uninstall。
- [silent / unattended uninstall 下 MessageBox 行為] → 預設視為 No，避免無人值守時誤刪資料；apply 時以 NSIS silent 旗標驗證分支。
- [`RMDir /r` 誤刪風險] → 目標為兩個固定、由 codebus identifier / `.codebus` 決定的全域路徑，非使用者輸入、非遍歷搜尋；不碰 vault。
- [整體 purge 行為僅 P3 真機可驗] → 明確列入 spec/design 誠實邊界；本環境只驗 CLI 命令、編譯、build 產物，不宣稱 purge end-to-end「能用」。
