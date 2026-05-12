## Context

`codebus-app/src/components/settings/SettingsModal.tsx` 目前讀寫 `claude_code.{goal,query,fix}.model` 的 flat 形狀並提供未版本化 `opus`/`haiku`/`sonnet` 的 dropdown — 兩個都是 `claude-code-endpoint-profiles` change ship 後就過時的設計。配合 `fail-loud-on-config-parse-error` change 已落地的「config parse error 直接 abort」邏輯，**現在的 Settings UI 一旦碰到合法 profile schema 的 yaml 就會 round-trip parse 不出 model 欄位**（變成 `undefined`），等於現 Settings UI 在 main repo 已是壞的（雖然平常 user 是直接編 yaml + 用 CLI，沒走 GUI 所以沒被回報）。

Stage B 需要把 Settings UI 內部與 `claude_code.*` 相關的讀寫路徑遷移到 profile schema，同時加上 endpoint section + keyring 管理 UI。

`codebus-app/src-tauri/src/ipc/` 已有 `config.rs`（generic yaml⟷json round-trip）+ `vault_list.rs`（vault metadata CRUD），三個檔案的 pattern 統一：每個 IPC command 是 `#[tauri::command] async fn` + 透過 `IpcResult<T> = Result<T, AppError>` 統一 error；`mod.rs` 用 `REGISTERED_COMMANDS` 常數 + `generate_ipc_handler!` macro + unit tests 鎖定數量與名稱。本次新增三個 keyring command 沿用該 pattern。

`AppError` 是 discriminated union (`serde(tag = "kind", rename_all = "snake_case")`)，前端 discriminate `kind` 欄位渲染對應 UI。新增 keyring 相關 error 要對齊既有 variant 設計（`io` / `config_parse` / `invalid` / `internal` 等）。

Frontend store `codebus-app/src/store/settings.ts`（Zustand）目前用 `config: serde_json::Value` 直接存從 `load_global_config` 拿到的 JSON tree，update / save 透過 mutate object 後呼叫 `save_global_config`。Stage B 把 model 欄位讀寫路徑從 `config.claude_code.goal.model` 改成 `config.claude_code.{active}.{verb}.model`，但 store 內部仍維持「整份 JSON tree」的 storage shape — round-trip 邏輯不變。

## Goals / Non-Goals

**Goals:**

- Settings UI 與 profile schema + versioned `SystemModel` enum 對齊；切 active 不丟另一邊輸入。
- Keyring 管理（set / get-status / delete）完整在 GUI 內可操作，不需要開 terminal。
- API key 不經 yaml round-trip，secret 只在 set-key modal → keyring IPC payload 短暫滯留。
- 對既有 IPC pattern（`REGISTERED_COMMANDS` 常數 + macro + unit tests）最小擾動，新增三條 command 沿用同 pattern。
- Frontend 元件責任清晰：`EndpointSection` 負責 form + active 切換，`SetKeyDialog` 負責 key 輸入 + keyring IPC 呼叫；兩者透過 props 連動。

**Non-Goals:**

- 不做 provider selector（Codex 真要進來時另開 change）。
- 不在 IPC 層做 endpoint schema 驗證 / 翻譯（schema 驗證由 codebus-core 既有的 fail-loud load 路徑負責；frontend 用 TypeScript type 預檢）。
- 不做 endpoint reachability health-check / Test Connection 按鈕。
- 不動 Settings UI 既有的 PII scanner / quiz threshold / log dir 欄位。
- 不改 CLI 端任何行為。
- 不引入新的 frontend state-management 函式庫（既有 Zustand 已足夠）。
- 不做 i18n locale 完整覆蓋（只加新 endpoint section 翻譯 keys，其他現有 keys 不動）。

## Decisions

### IPC 數量與命名：三條獨立 keyring command 而非單一 `manage_endpoint_key(action, ...)` aggregate

`set_endpoint_key(profile, key)` / `get_endpoint_key(profile) -> "set" | "unset"` / `delete_endpoint_key(profile)` 三個 command 分開列出，理由：

- 沿用既有 `vault_list` pattern（`list_vaults` / `add_vault` / `remove_vault` 三個 command 而不是一個 `manage_vaults(action, ...)`）。
- 前端 type 比較乾淨：每個 IPC 的 args / return type 各自精確，不用 TypeScript `discriminated union` 多包一層。
- IPC count 從 5 → 8 不是大問題；`app-shell` spec 的「exactly five」本來就是預期會擴張的數字（spec 內部 `exactly_five_commands_are_registered` 測試名稱明確帶數字，每次 IPC 擴張本來就要動 spec）。

替代：把三個動作塞進單一 command 用 `action: "set" | "get" | "delete"` 區分 — 被否決，IPC 介面語意混淆。

### `get_endpoint_key` 永遠不回傳 key 值

`get_endpoint_key(profile) -> "set" | "unset"` 只回 status 字串，**不**有 `--show` 等價的 IPC 變體可以拿到明文 key。理由：

- IPC payload 經 Tauri serialise 後雖然 process boundary 安全，但會出現在 `tauri::AppHandle` event log、Tauri dev tools、可能的 crash dump 內 — 沒必要傳輸明文。
- UI 不需要顯示明文 key（user 自己鍵入後就沒理由再看；忘了就 delete + reset）。
- CLI 端的 `--show` flag 是給 debug user 用，IPC / GUI 不採同樣 surface。

替代：加 `show_endpoint_key` 第 9 條 IPC — 被否決，違反 secret 最少暴露原則。

### `set_endpoint_key` 接受 String args（不接受 stream）

`set_endpoint_key(profile: String, key: String) -> Result<(), AppError>` — key 直接 String 參數傳入。雖然 IPC payload memory 滯留時間短，但對齊既有 Tauri command 模式（無 stream 機制）。

UI 端：`SetKeyDialog` 是 modal 帶 `<input type="password">`，user 輸入後按 confirm → 直接呼叫 IPC，IPC 回 ok 後立刻清空 input state（Zustand store **不**儲存 key 值）。

### Active profile radio + accordion 結構

兩個 profile 區塊都常駐在 DOM，但採 **accordion 結構**：active 預設展開、inactive 預設折疊成單行 header（仍顯示 `(inactive)` label），user 可點 header 手動展開 inactive 編輯 cold storage：

```
┌─ Endpoint section ─────────────────────────────┐
│ Active: ( ⦿ System / ○ Azure )                  │
│                                                  │
│ ▾ System Profile                                 │
│ ┌─────────────────────────────────────────────┐│
│ │ goal:  [opus-4-6 ▼]   [effort: high]        ││
│ │ query: [haiku-4-5 ▼]  [effort: low]         ││
│ │ fix:   [sonnet-4-6 ▼] [effort: medium]      ││
│ └─────────────────────────────────────────────┘│
│                                                  │
│ ▸ Azure Profile (inactive)              [click] │
│                                                  │
└──────────────────────────────────────────────────┘

(click 後)

│ ▾ Azure Profile (inactive)                      │
│ ┌─────────────────────────────────────────────┐│
│ │ base_url:        [____________________]     ││
│ │ keyring_service: [codebus-azure________]     ││
│ │ API key: ● Set    [Set new...] [Delete]      ││
│ │ goal:  [dep:_______________]  [effort: high] ││
│ │ ...                                          ││
│ └─────────────────────────────────────────────┘│
```

**Folding policy**：
- 切換 active radio 時，新 active 自動展開、舊 active 自動折疊（兩個 expanded state 跟 active 跑）。
- User 手動點折疊 header 可單獨展開 inactive — 不影響 active 那邊。
- 折疊狀態下 form input **仍在 DOM**（用 CSS `hidden` 隱藏，不 unmount），切換時 input value 保留（spec 的 `Active radio switch preserves non-active profile inputs` 契約不變）。

**為什麼從原本「兩 sub-section 同時渲染」改 accordion**：實機驗證後發現兩 block 同時 expanded 佔太多 vertical space — Settings modal 已經有 7 個 field，再加兩 block 各 5+ 行 form 元素，scroll 體驗差。Accordion 讓 inactive 縮成 1 行 header，主要視覺焦點對到 active，user 仍能 1 click 進 cold storage 編輯。

替代：tabbed UI（一次只看一個 profile，切到另一個就看不到本邊）— 被否決，違反「user 切 active 時看到另一邊資料還在」的直覺；accordion 折疊狀態仍有 visible header，跟 tabbed 全藏不同。

### Frontend form validation 對齊 spec 而不重新發明驗證

`SystemModel` 選項在 dropdown 寫死 4 個 hard-coded values；`base_url` / deployment name / `keyring_service` 文字輸入只做「非空」驗證，**不**做 URL 格式 / 服務名 charset 驗證。理由：

- Schema 驗證由 codebus-core fail-loud load 路徑負責（Stage A + fail-loud change 已落地）。
- Frontend 多做驗證會跟 backend 規則分歧，雙寫負擔且容易 drift。
- User 輸入無效值（如 base_url 漏 https）會在下次 Save 後 reload yaml 時被 codebus-core parse error 擋下 — 屬於合理 UX。

### 兩層必填驗證：frontend disable Save + backend save 拒寫

實機驗證後發現一個 UX trap：若 user 切 `active=azure` 但 azure 欄位（base_url / deployment names）沒填完，原本 Save 會把不完整的 yaml 寫進 disk，下次 CLI 跑就 fail-loud abort — GUI 寫的設定 CLI 載不了。修法分兩層：

1. **Frontend 主動驗證**：`lib/ipc.ts::validateClaudeCodeBlock(block)` 是 single source of truth，回傳 `ClaudeCodeValidationError[]`。`SettingsModal` 用它 disable Save 按鈕；`EndpointSection` 用它在 input 上掛 `aria-invalid` + 渲染 inline validation summary。
2. **Backend 寫入前驗證**：`save_global_config_at` 收到 payload 後呼叫 `endpoint::parse_claude_code_yaml` 對 `claude_code` 區塊跑一次完整 schema validation；invalid 回 `AppError::Invalid { field: "claude_code", message }`，**yaml 完全不落 disk**。

Frontend 驗證提供即時 feedback；backend 是 backstop — 即使 GUI bug 或 user 從別處走 IPC，無效 config 仍寫不下去。**兩層用同一個 codebus-core schema 規則**（codebus-core 是 ground truth），frontend 規則跟 backend 規則對齊不重複實作。

### `Authentication` field 改為 `Claude Code CLI` installation probe

原本 Settings 第 2 個 field 叫 `Authentication`，渲染靜態 `oauthStatus` label（沒 live auth flow，純 placeholder）。改成 `Claude Code CLI` row：open Settings 時呼叫新 IPC `check_cli_installed("claude_code")` 跑 `claude --version`，依結果渲染 `Installed · <version>` / `Not installed`（+ 安裝提示）/ `Checking…`（probe in-flight）。

未來新 vendor（Codex / Gemini）進來時：在 settings 加同樣 row + 對應 `provider` 值；不採 provider selector dropdown（同 Codex 預留決策 — single-impl 不抽象）。

`check_cli_installed` IPC 行為：spawn `<binary> --version` 在 `tauri::async_runtime::spawn_blocking` 內，任何失敗（binary missing / non-zero exit / empty stdout）都 collapse 成 `NotInstalled` — user 視角這些都是「你還沒 setup」，沒必要區分。

### Keyring service 預填 `codebus-azure`

UI load 時若 `claude_code.azure.keyring_service` 是空字串或缺欄位 → input 預填 `codebus-azure`（spec 的 default keyring service name）。User 即使還沒按 Save，按「Set new...」也會用該預填值寫 keyring（對齊 CLI 端 `read_azure_keyring_service_from_config` 的 fresh-setup fallback）。

### 既有 `SettingsModal` model dropdown 同步遷移

`SettingsModal` 上方既有的 goal/query/fix 三個 dropdown **保留**，但讀寫路徑與選項改為 profile-aware：

- 讀：根據 `claude_code.active`，goal/query/fix 三個 dropdown 顯示**當下 active profile 的 model 值**。System mode 顯示 4 versioned enum；Azure mode 顯示 deployment name 字串（dropdown 退化為 text input — 或考慮統一改成「直接編 `<EndpointSection>` 內部」，移除頂部 dropdown）。
- 寫：mutate `claude_code.{active}.{verb}.model`。

**Decision**：移除 `SettingsModal` 頂部的舊 goal/query/fix dropdown，全部下放到 `<EndpointSection>` 內的兩 tab 顯示，避免「同一 user 概念顯示兩次」造成 mental model 衝突。SettingsModal 頂部保留 PII scanner / quiz / log dir 等其他 fields。

## Implementation Contract

### IPC commands (新)

```rust
#[tauri::command]
pub async fn set_endpoint_key(profile: String, key: String) -> IpcResult<()>;

#[tauri::command]
pub async fn get_endpoint_key(profile: String) -> IpcResult<KeyStatus>;

#[tauri::command]
pub async fn delete_endpoint_key(profile: String) -> IpcResult<()>;

/// Discriminated union serialised as `serde(tag = "kind", rename_all = "snake_case")`.
pub enum KeyStatus { Set, Unset }
```

行為契約：

- `profile` 只接受字串 `"azure"`，其他值（包括未來的 `bedrock` / `vertex`）SHALL 回 `AppError::Invalid { field: "profile", message: ... }`。
- 三個 command 內部呼叫 `codebus_core::config::keyring::{store_azure_key, probe_keyring_only, delete_azure_key}` 對應的 helper，傳入的 `service` 從 config.yaml 的 `claude_code.azure.keyring_service` 解析（透過 `read_azure_keyring_service_from_config` 的 IPC 版本邏輯，遇 parse error fail-loud 回 `AppError::ConfigParse`）。
- `set_endpoint_key` 完成 ok 後 IPC payload 立刻清空（Rust side 無 caching；frontend store **不**儲存 key 值，僅儲存 status）。
- `delete_endpoint_key` idempotent — 即使 entry 不存在亦回 `Ok(())`。
- Keyring backend 完全不可用 → 三個 command 回 `AppError::Internal { message: "keyring backend unavailable: ..." }`。

### Frontend element: `<EndpointSection>`

```tsx
interface EndpointSectionProps {
  // Subset of config.claude_code from global config payload
  claudeCode: {
    active: "system" | "azure"
    system: SystemProfile
    azure: AzureProfile | null  // null when block absent
  }
  // Update callback — mutates parent store's config tree
  onChange: (updated: ClaudeCodeBlock) => void
}

interface SystemProfile {
  goal:  { model: SystemModel; effort: string }
  query: { model: SystemModel; effort: string }
  fix:   { model: SystemModel; effort: string }
}

type SystemModel = "opus-4-7" | "opus-4-6" | "haiku-4-5" | "sonnet-4-6"

interface AzureProfile {
  base_url: string
  keyring_service: string
  goal:  { model: string; effort: string }
  query: { model: string; effort: string }
  fix:   { model: string; effort: string }
}
```

行為契約：

- Active radio 切換 → 立即 mutate parent state `claude_code.active`，**不**清除另一 profile 的欄位內容。
- System tab 三個 verb 的 model dropdown 列固定 4 options。
- Azure tab keyring_service input 預填 `codebus-azure` 當該欄位是空字串或 undefined。
- API key 區塊：mount 時呼叫 `get_endpoint_key("azure")` 拿 status；「Set new...」開 `<SetKeyDialog>` modal；「Delete」直接呼叫 `delete_endpoint_key("azure")` IPC，成功後 status 更新為 `Unset`。

### Frontend element: `<SetKeyDialog>`

```tsx
interface SetKeyDialogProps {
  open: boolean
  onClose: () => void
  onSuccess: () => void  // 通知 parent 更新 status 為 Set
}
```

行為契約：

- 開啟時 input 為空字串。
- input type=password（不 echo）。
- 「Confirm」按鈕呼叫 `set_endpoint_key("azure", value)` IPC；成功則 close + `onSuccess()`；失敗則顯示 inline error，input 不清空，user 可改後重試。
- 「Cancel」按鈕 close，input value 不持久化（modal 內部 state，unmount 時清空）。

**Acceptance criteria**：

- `codebus-app/src-tauri/tests/keyring_ipc.rs`（new）驗證三個 IPC command 的契約：(a) 合法 profile=azure round-trip set→get→delete；(b) profile=bedrock 拒絕回 `AppError::Invalid`；(c) delete 不存在的 entry 仍 ok；(d) `REGISTERED_COMMANDS` 含三個新名稱且共 8 個 commands。
- `codebus-app/src/components/settings/EndpointSection.test.tsx`（new）使用 vitest + React Testing Library 驗證：(a) active radio 切換不清除另一 tab 內容；(b) system dropdown 列 4 versioned options；(c) azure keyring_service 預填行為；(d) 「Set new...」開啟 modal、「Delete」呼叫 IPC（mock IPC，斷言 payload）。
- 既有 `codebus-app/src/components/settings/SettingsModal.test.tsx` 從 legacy schema fixture 改成 profile schema fixture 後維持綠。
- `codebus-app/src-tauri/src/ipc/mod.rs` 內 `exactly_five_commands_are_registered` test 改名 `exactly_eight_commands_are_registered` + 對應的 set 比對更新；name_matches / no_duplicates 兩 test 自動跟著修。
- 手動驗證：跑 `pnpm tauri dev`（或對應 build flow），開 Settings，切換 active、編輯兩 tab、按 Set new... 輸入 key → 確認 yaml 寫對 + keyring 寫對；按 Delete → 確認 keyring entry 移除 + status 顯示 Unset。

**Scope boundaries**：

- **不**改 CLI 端任何檔案。
- **不**改 codebus-core（keyring helper 已在 Stage A 落地，直接 reuse）。
- **不**動 `claude_code` config schema（已由 Stage A 拍板）。
- **不**動 PII / quiz / log 等既有 Settings 欄位行為。
- **不**改 `load_global_config` / `save_global_config` IPC 簽名（保留 yaml⟷json 透傳契約）。
- **不**做 secret rotation policy / 多 key 管理（每個 profile 一條 keyring entry，不版本化）。

## Risks / Trade-offs

- **App 既有 store 對 legacy schema 的依賴沒測試覆蓋**：`SettingsModal` 之前的 model dropdown 沒有完整的 e2e 測試覆蓋 yaml round-trip，遷移後可能踩到 corner case（例如 config 中只有 active=system 但 azure block 完全缺）→ mitigation: 在 `SettingsModal.test.tsx` 補上 corner case fixture（config 沒 azure block + config 有 partial azure block + config 有完整兩 block）。
- **Frontend 4 enum values 跟 backend `SystemModel` 寫死兩處**：未來新增 `opus-4-8` 等 variant 時要兩邊同步改 → mitigation: 在 spec/design 註記為「known coupling, accept」；未來真的常改可考慮從 codebus-core 透過 build script 產 TS const。
- **Keyring backend 不可用時 UI 行為**：Linux headless 或 Docker 內無 secret service → 三個 keyring IPC 都會回 `AppError::Internal` → mitigation: UI 顯示「keyring unavailable; set CODEBUS_AZURE_KEY env var instead」hint（與 CLI 端訊息對齊）；不嘗試 fallback 到 env 因為 IPC 介面不是設定 env 的地方。
- **`SettingsModal` 頂部 model dropdown 移除可能違反 user 直覺**：之前的 UX 是「最上面看到 model dropdown」，現在改成「展開 Endpoint section 才看到」→ mitigation: 「Endpoint section」放在 SettingsModal 上方第二欄位（緊接 AI Provider 標籤之後），預設展開，降低 user 找不到 model 設定的風險。
- **`get_endpoint_key` 不回明文 key 可能讓 user 困惑「我設了什麼 key」**：UI 只顯示 `Set` / `Unset` → mitigation: 在 spec scenario 寫明「verifying key 內容 SHALL 透過跑 `codebus query` 驗證，不透過 UI 回顯」；hint 文字「To verify the key works, run `codebus query \"ping\"` in your terminal」。
- **IPC count 從 5 → 8 是 spec 變更**：明確 BREAKING for `app-shell` spec 的「exactly five」requirement；但這條 requirement 是字面數字，每次 IPC 擴張本來就要動 → 預期之內，不算 risk，明確標 BREAKING 就好。
