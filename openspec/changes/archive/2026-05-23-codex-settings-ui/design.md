## Context

archived `codex-backend` 讓 codex 可透過 `~/.codebus/config.yaml` 的 `agent.providers.codex` 使用,但 codebus-app(React/TSX)的 Settings 寫死 claude:`EndpointSection` 用 claude 閉枚舉、`store/settings.ts:updateClaudeCode` 硬編 `active_provider:"claude"`、`check_cli_installed` 只接受 `claude_code`。GUI 使用者無法選/設 codex。本 change 在 settings 層引入 provider registry,讓 claude + codex 成為兩個條目,未來新 provider 主要是「加一筆 registry + 它的編輯器元件」。

前端測試:`npx vitest run` + `npx tsc --noEmit`(`codebus-app/package.json`)。

## Goals / Non-Goals

**Goals:**

- GUI 可選 codex 並編輯其端點(system 自由字串 model + azure 含 api_version),存檔寫到 `agent.providers.codex` 且設 `active_provider`。
- 跨 provider 的共用程式碼改成 registry-driven,新增 provider 不必改膠水。
- 前端驗證與後端 `save_global_config` 對 codex block 一致(都走 `parse_codex_yaml`),不讓 GUI 存得下 core 會拒的設定。

**Non-Goals:**

- 通用 schema→表單引擎(各 provider 編輯器是具體元件)。
- 假設 azure 通用(profiles 由 provider 宣告)。
- 動態 provider plugin / 假想 hook;gemini 等其他 provider;codex 後端行為(已在 codex-backend)。

## Decisions

**D1 provider registry(`codebus-app/src/lib/providers.ts`)**:`{ id, displayName, cliBinaryId, profiles, validate(block), EditorComponent }`,含 `claude` 與 `codex` 兩筆。跨 provider 膠水(選擇器、依 id 讀寫、CLI 檢查、驗證分派、後端驗證分派)讀 registry。

**D2 profiles per-provider 宣告、不假設 azure**:claude/codex 都宣告 `["system","azure"]`;未來 provider 可宣告不同集合,不被硬塞 azure 槽。

**D3 具體編輯器元件,不做表單引擎**:`EndpointSection`(claude,閉枚舉 dropdown,保留現行行為)與新增 `CodexEndpointSection`(自由字串 model + azure 多 `api_version`)。差異留在各元件內。

**D4 store 改 provider-keyed**:`getProviderBlock(id)`/`updateProviderBlock(id, block)` 讀寫 `agent.providers.<id>` 並設 `active_provider`;移除硬編 claude。

**D5 驗證雙層一致**:registry 的 `validate(block)`(前端)規則對齊對應 core parser;後端 `save_global_config` 依 active_provider 走 `parse_claude_code_yaml`/`parse_codex_yaml` 驗證後才寫檔。

**D6 `check_cli_installed` 接受 codex**:後端 match arm 加 `codex`(探 codex binary);非法 provider → `not_installed` 不報錯。

## Implementation Contract

**Behavior**:Settings modal 出現 provider 選擇器(claude/codex);選 codex → 顯示 codex 端點編輯器 + codex CLI 狀態,存檔寫 `agent.providers.codex` + `active_provider: codex`;選 claude → 現行行為不變。

**Interface / data shape**:
- `providers.ts`:`ProviderDescriptor { id: "claude"|"codex"; displayName; cliBinaryId: string; profiles: string[]; validate(block): ValidationError[]; EditorComponent }`;`PROVIDERS` registry。
- `store/settings.ts`:`getProviderBlock(id)`/`updateProviderBlock(id, block)`(取代 `getClaudeCodeBlock`/`updateClaudeCode`,後者可保留為 claude 包裝或移除)。
- `ipc.ts`:`AgenticProvider` 加 `"codex"`;`validateCodexBlock(block)`;`CodexBlock` 型別;`checkCliInstalled(cliBinaryId)`。
- `CodexEndpointSection.tsx`:props `{ block, onChange, errors }`,自由字串 model + azure `api_version`。
- 後端 `codebus-app/src-tauri/src/ipc/config.rs`:`check_cli_installed` 加 codex arm;`save_global_config` 依 active_provider 驗 codex block(`parse_codex_yaml`)。

**Failure modes**:codex active profile 缺 verb → 前端 validation 標紅 + Save 禁用、後端 reject 不寫檔;非法 provider 給 `check_cli_installed` → `not_installed`;codex model 任意字串不被拒。

**Acceptance criteria**:`npx vitest run`(新增 CodexEndpointSection / providers registry / settings store / validateCodexBlock 測試,含 RED 先行)+ `npx tsc --noEmit` 綠;`cargo test --package codebus-app-tauri`(check_cli codex arm + save_global_config codex 驗證);既有 claude settings 測試不退步。

**Scope boundaries**:前端 + app-tauri IPC;不動 codebus-core(codex 後端已完成);不做表單引擎、不假設 azure、不加其他 provider。

## Risks / Trade-offs

- **重構現行 claude settings UI**:把 claude 專屬路徑(EndpointSection/getClaudeCodeBlock/validateClaudeCodeBlock)收進 registry,有讓既有 claude 測試退步的風險 → 以「claude 行為不變」為驗收線、保留並擴充既有 vitest 測試。
- **前後端驗證漂移**:前端 `validate` 與 core parser 可能不一致 → 後端 `save_global_config` 用 core parser 做最終 gate(前端只是 UX),以後端為準。
- **registry 抽象 vs YAGNI**:只抽兩個真實 provider 需要的膠水,不預埋 hook;若第三家 provider 出現需求超出此形狀,屆時再擴(可接受)。
