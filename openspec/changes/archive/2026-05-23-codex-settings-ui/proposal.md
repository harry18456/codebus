## Why

archived change `codex-backend` 讓 codex 可透過 `~/.codebus/config.yaml` 的 `agent.providers.codex` 使用,但 codebus-app 的 Settings 介面**沒有任何方式選擇 codex 或編輯它的端點**——目前只接好 claude:`EndpointSection` 用 claude 專屬的 `SystemModel` 閉枚舉、`store/settings.ts` 的 `updateClaudeCode` 硬編 `active_provider: "claude"`、`check_cli_installed` 只接受 `claude_code`。對只用 GUI 的使用者,codex 等於不可用(只能手改 YAML)。

此外目前 settings 程式碼**寫死 claude**;若用「複製一份 claude UI 給 codex」的方式接,每加一個 provider 就重複一次。本 change 在 settings 層引入 **provider registry 抽象**:claude 與 codex 成為 registry 的兩個條目,跨 provider 的共用程式碼改成 registry-driven,未來新增 provider 主要是「加一筆 registry + 它自己的編輯器元件」。codex 是真實的第二個 provider,足以驅動此抽象(非為 0-consumer 預作)。

## What Changes

- **provider registry**(`codebus-app/src/lib/providers.ts`):每個 provider 宣告 `{ id, displayName, cliBinaryId, profiles(該 provider 自己有哪些端點 profile), validate(block), EditorComponent }`。跨 provider 的膠水(provider 選擇器、依 id 讀寫 config、CLI 狀態檢查、驗證分派)改成讀 registry。
- **AI Provider 欄位變成真正的選擇器**,設定 `agent.active_provider`(claude | codex);`store/settings.ts` 停止硬編 claude,改 `getProviderBlock(id)` / `updateProviderBlock(id, block)`。
- **Codex 端點編輯器元件**(`CodexEndpointSection.tsx`):model 為**自由字串**輸入(非 claude 的閉枚舉 dropdown),azure 變體含 `base_url` + `api_version` + `keyring_service`。**profiles 由各 provider 宣告,不假設 azure 通用**。
- **`check_cli_installed`** 接受目前 provider 的 `cliBinaryId`(codex → 其 binary id),取代寫死的 `claude_code`。
- **後端 `save_global_config` 驗證目前 active provider 的 block**:codex 走 `parse_codex_yaml`、claude 走 `parse_claude_code_yaml`(避免 GUI 存得下 core 會拒的設定)。
- **claude settings 重構成 registry 的第一個條目**,行為不變(既有測試不退步)。

## Non-Goals (optional)

- 通用「schema → 表單」引擎:各 provider 的端點編輯器是**具體元件**,不做資料驅動表單渲染器。
- **不假設 azure 通用**:profiles 由各 provider 宣告,某 provider 可只有 `system` 或別種第二端點。
- 動態 provider plugin 載入 / 為假想 provider 預埋 hook。
- gemini 等其他 provider(現在只有 claude + codex)。
- codex 後端行為(已在 `codex-backend` 完成)。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-shell`: Settings UI 改成 provider-registry-driven —— provider 選擇器、per-provider 端點編輯器(含 codex 自由字串 model + api_version)、codex CLI 狀態、codex config 驗證、`check_cli_installed` 接受 codex。

## Impact

- Affected specs: `app-shell`(modified)
- Affected code:
  - New:
    - codebus-app/src/lib/providers.ts
    - codebus-app/src/components/settings/CodexEndpointSection.tsx
  - Modified:
    - codebus-app/src/components/settings/SettingsModal.tsx
    - codebus-app/src/components/settings/EndpointSection.tsx
    - codebus-app/src/store/settings.ts
    - codebus-app/src/lib/ipc.ts
    - codebus-app/src-tauri/src/ipc/config.rs
  - Removed: (none)
