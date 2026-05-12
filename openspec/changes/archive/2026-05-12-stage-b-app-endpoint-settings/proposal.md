## Why

已 archived 的 `claude-code-endpoint-profiles` change 在 CLI 端落地了 profile schema + 版本化 `SystemModel` enum + OS keyring 整合，但 **Tauri App 的 `SettingsModal.tsx` 還停在 legacy schema**：

- UI 讀 `claude_code.goal.model` flat 形狀（已不存在），不認 `claude_code.system.goal.model` 巢狀結構。
- Model dropdown 選項仍是未版本化的 `opus` / `haiku` / `sonnet`（已被 `claude-code-config / System Profile Model Aliases` 拒絕；新 schema 只接受 `opus-4-7` / `opus-4-6` / `haiku-4-5` / `sonnet-4-6`）。
- 沒有 `active` profile selector、沒有 azure tab、沒有 keyring API key 管理介面 — user 要設 Azure endpoint 只能開 terminal 跑 `codebus config set-key azure` + 手改 yaml。

Stage B 把 App Settings UI 補上 endpoint / keyring 管理，讓 user 完全在 GUI 內完成 system ↔ azure 切換 + key 管理；同時解掉現有 UI 對 legacy schema 的依賴。

## What Changes

- **Settings UI 新增 Endpoint section**：放在現有 quiz / model dropdown 同一個 `SettingsModal`，標題 `Claude Code endpoint settings`（純文字標籤，**不**加 provider selector — Codex 等其他 vendor 真要進來時另開 change 處理 UI restructure）。
- **Active profile radio**：兩選一（`system` / `azure`），切換不丟另一邊輸入。
- **System profile tab**：三個 verb (`goal` / `query` / `fix`) 各有 model dropdown（4 versioned options）+ effort text input。
- **Azure profile tab**：`base_url` 文字輸入、`keyring_service` 文字輸入（預填 `codebus-azure`）、API key 狀態指示（`Set` / `Unset`）+ Set/Show/Delete 三個按鈕、三個 verb 的 deployment name + effort 輸入。
- **既有 model dropdown 從 legacy schema 遷移到 profile schema**：原本散落在 `SettingsModal` 上方的 `goalModel` / `queryModel` / `fixModel` 三個欄位讀寫路徑從 `claude_code.{verb}.model` 改成 `claude_code.system.{verb}.model`（若 active=azure 則改寫 `claude_code.azure.{verb}.model`），dropdown 選項從未版本化 `opus`/`haiku`/`sonnet` 換成 versioned 4 個值。**BREAKING for UI 內部 store**：`useSettingsStore` 的 model 欄位 shape 改變。
- **新增三條 keyring IPC**：`set_endpoint_key` / `get_endpoint_key` / `delete_endpoint_key`，實作於 `codebus-app/src-tauri/src/ipc/keyring.rs`，內部 delegate 到 `codebus_core::config::keyring::{store_azure_key, probe_keyring_only, delete_azure_key}`。**BREAKING for app-shell spec**：IPC 數量從 `exactly five` 改為 `exactly eight`，`REGISTERED_COMMANDS` 常數 + `generate_ipc_handler!` macro + `exactly_five_commands_are_registered` 測試都要更新。
- **API key 不經 yaml round-trip**：Save 按鈕只觸發 `save_global_config`（寫 yaml）；改 key 是獨立 path（modal + keyring IPC）。Secret 不出現在任何 IPC payload 之外的位置。
- **不加 Test Connection 按鈕**：與 CLI 端「不加 doctor」一致，避免引入第 9 條 IPC + timeout / error display UX 設計負擔。user 自測請跑 `codebus query "ping"`。

## Non-Goals

- **不**做 provider selector / `Provider: [Claude Code ▼]` dropdown — single-option 死碼是 anti-pattern；Codex 進來再 restructure。
- **不**動 `claude_code` config schema 本身（已由 Stage A 拍板）。
- **不**做 endpoint reachability health-check（無 Test 按鈕、無 doctor 命令）。
- **不**動其他 Settings 欄位（PII scanner / quiz threshold / log dir），這些既有 fields 保留現狀。
- **不**改 CLI 端任何行為（CLI 已在 Stage A / fail-loud-on-config-parse-error 兩個 change 完成）。
- **不**在 IPC 層做任何 endpoint schema 翻譯（保留「IPC 透傳 yaml json」精神，schema 驗證由 codebus-core 負責；前端用 TypeScript type 預先驗證 form input）。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-shell`: 修改 IPC Command Registry requirement（5 → 8 commands），新增 keyring 三個 sub-action 的 IPC 契約。新增 Settings UI Endpoint Section 行為要求（active radio + 兩 tab + keyring 管理 UX + form 驗證對齊 `claude-code-config` schema）。

## Impact

- 影響 spec：modify `app-shell`（IPC count + Endpoint section UI 行為要求）。
- 影響程式碼：
  - 新增：
    - codebus-app/src-tauri/src/ipc/keyring.rs（三個 Tauri command 實作 + `AppError` 映射）
    - codebus-app/src/components/settings/EndpointSection.tsx（新 React 元件，掛在 SettingsModal 內）
    - codebus-app/src/components/settings/SetKeyDialog.tsx（modal for input api key）
    - codebus-app/src/components/settings/EndpointSection.test.tsx（form 行為單元測試）
    - codebus-app/src-tauri/tests/keyring_ipc.rs（IPC integration test）
  - 修改：
    - codebus-app/src-tauri/src/ipc/mod.rs（register 三個 command + 更新 `REGISTERED_COMMANDS` 常數 + `generate_ipc_handler!` macro + 既有 unit tests 從「exactly_five」改成「exactly_eight」）
    - codebus-app/src-tauri/src/error.rs（新增 `AppError::Keyring` variant；`KeyringError` → `AppError` 映射）
    - codebus-app/src-tauri/src/lib.rs（無實質改動；handler list 透過 macro 自動帶入）
    - codebus-app/src/components/settings/SettingsModal.tsx（讀寫路徑從 legacy `claude_code.goal.model` 遷移到 `claude_code.{active}.{verb}.model`；嵌入 `<EndpointSection />`）
    - codebus-app/src/components/settings/SettingsModal.test.tsx（既有 model dropdown 測試 fixture 從 legacy schema 換成 profile schema）
    - codebus-app/src/store/settings.ts（state shape 從 flat `model` 換成 `{ active, system, azure }`；新增 keyring IPC 呼叫 helper）
    - codebus-app/src/i18n/locales/*.json（新增 endpoint section 翻譯 keys）
  - 刪除：無
- 影響使用者：UI 內部 schema 遷移屬於必要修正（既有 Settings UI 在 fail-loud 改動後遇到 legacy schema 會 parse error，UI 早就不能正常 round-trip）；對 user 觀感是「Settings 新增 Endpoint 區塊」。
- 不影響：CLI 行為（Stage A 完成）、Tauri app 啟動流程、Lobby vault list 行為。
