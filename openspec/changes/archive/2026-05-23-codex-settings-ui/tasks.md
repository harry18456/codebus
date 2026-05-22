# Tasks

依賴順序:registry+store(1)→ ipc 型別/驗證(2)→ CodexEndpointSection(3)→ SettingsModal 接線(4,依賴 1/2/3)→ 後端(5,與前端不同檔可並行)→ 回歸(6)。前端 code task 先寫 RED vitest 測試;`.spectra.yaml` tdd:true。驗證:`npx vitest run` + `npx tsc --noEmit` + `cargo test --package codebus-app-tauri`。

## 1. provider registry + store(`Settings Provider Registry`)

- [x] 1.1 [P] [RED] 寫 registry + store 測試:`codebus-app/src/lib/providers.ts` 的 `PROVIDERS` 含 `claude`/`codex` 兩筆(各有 `cliBinaryId`/`profiles`/`validate`/Editor);`store/settings.ts` 的 `updateProviderBlock("codex", block)` 寫到 `agent.providers.codex` 且設 `active_provider:"codex"`、保留其他 providers;`getProviderBlock("codex")` 讀回。初次 FAIL。
- [x] 1.2 實作 `providers.ts`(ProviderDescriptor + PROVIDERS registry,profiles per-provider 宣告、不假設 azure)+ `store/settings.ts` 的 `getProviderBlock`/`updateProviderBlock`(移除硬編 `active_provider:"claude"`)使 1.1 綠。涵蓋 spec requirement: Settings Provider Registry。

## 2. ipc 型別與驗證(`Settings UI Endpoint Section` 驗證部分)

- [x] 2.1 [P] [RED] 寫 `validateCodexBlock` 測試(`codebus-app/src/lib/ipc.ts`):codex azure 缺 `base_url`/`api_version`/`keyring_service`/verb model → 列出失敗欄位;codex system 任意字串 model(如 `gpt-5.5`)不被拒;規則對齊 `parse_codex_yaml`。初次 FAIL。
- [x] 2.2 實作 `ipc.ts`:`AgenticProvider` 加 `"codex"`、`CodexBlock` 型別、`validateCodexBlock`、`checkCliInstalled` 接受任意 `cliBinaryId`,使 2.1 綠。

## 3. CodexEndpointSection 元件(`Settings UI Endpoint Section`)

- [x] 3.1 [RED] 寫 `CodexEndpointSection.tsx` 測試:system 四 verb 為**自由字串 model 輸入**(非 dropdown);azure 子區含 `base_url`/`api_version`/`keyring_service` 輸入 + API key 狀態 + 四 verb deployment-name 輸入;active radio 切換不清空非作用 profile 的值;失敗欄位 `aria-invalid`。初次 FAIL。
- [x] 3.2 實作 `codebus-app/src/components/settings/CodexEndpointSection.tsx`(props `{block,onChange,errors}`)使 3.1 綠。涵蓋 spec requirement: Settings UI Endpoint Section(codex 編輯器)。

## 4. SettingsModal 接線(`Settings UI Endpoint Section` + `Settings UI CLI Status Field`)

- [x] 4.1 [RED] 寫 SettingsModal 測試:provider 選擇器列 claude/codex;選 codex → `active_provider` 變 codex、渲染 `CodexEndpointSection`、CLI 狀態以 codex `cliBinaryId` 探測;選 claude → 仍渲染現行 `EndpointSection`(閉枚舉 dropdown 不變)。初次 FAIL。
- [x] 4.2 實作 `codebus-app/src/components/settings/SettingsModal.tsx`:加 provider 選擇器、依 registry 渲染選中 provider 的 EditorComponent 與 CLI 狀態列、Save 用選中 provider 的 `validate`;claude 路徑改為 registry 第一條目(行為不變)。使 4.1 綠。涵蓋 spec requirement: Settings UI CLI Status Field。

## 5. 後端 IPC(`IPC Command Registry`)

- [x] 5.1 [P] [RED] 寫後端測試(`codebus-app/src-tauri`):`check_cli_installed("codex")` 探 codex binary(missing→`not_installed` 不報錯);非法 provider→`not_installed`;`save_global_config` 收到 `active_provider:codex` + active profile 缺 verb 的 codex block → reject 不寫檔(走 `parse_codex_yaml`)。初次 FAIL。
- [x] 5.2 實作 `codebus-app/src-tauri/src/ipc/config.rs`(及 check_cli 所在處):`check_cli_installed` 加 `codex` arm、`save_global_config` 依 `active_provider` 用 `parse_codex_yaml`/`parse_claude_code_yaml` 驗證,使 5.1 綠。涵蓋 spec requirement: IPC Command Registry。

## 6. 回歸與整合

- [x] 6.1 跑 `npx vitest run` + `npx tsc --noEmit` + `cargo test --package codebus-app-tauri` 全綠;確認既有 claude settings 測試(EndpointSection / SettingsModal)不退步。
- [x] 6.2 手動 e2e(deferred registry 慣例):GUI 開 Settings、選 codex、填 system profile 存檔→確認寫到 `agent.providers.codex`;切 claude→確認原樣;GUI smoke 跑不了則照 docs/v3-roadmap.md §5 歸檔,不卡 archive。
