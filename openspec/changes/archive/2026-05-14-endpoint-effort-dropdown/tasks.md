<!--
TDD enabled: 每個任務先寫失敗測試（RED），再實作（GREEN）。
parallel_tasks 啟用：標 [P] 的連續任務可並行（不同檔案或同檔案 disjoint 區段，無資料相依）。
-->

## 1. Validation 契約（ipc.ts）

- [x] 1.1 在 `codebus-app/src/lib/ipc.ts` 匯出 `SYSTEM_EFFORTS = ["high", "low", "medium"] as const` 與 `SystemEffort` literal union 型別。完成行為：新常數可從 `@/lib/ipc` import 並包含恰好這三個值且順序固定。驗證：新增 vitest 單元測試 `lib/ipc.effort.test.ts` 斷言 `SYSTEM_EFFORTS` 內容與順序，先寫失敗測試（RED）→ 加常數讓測試綠（GREEN）。
- [x] 1.2 擴充 `validateClaudeCodeBlock`（同檔）使其針對 `system` 與 `azure` 兩個 profile 的 `goal` / `query` / `fix` 三個 verb 的 `effort` 欄位檢查是否屬於 `SYSTEM_EFFORTS`，無論 `active` 為何；不合法值（含空字串與任意 legacy 值）SHALL 產生 `ClaudeCodeValidationError`，`field` 為 `claude_code.<profile>.<verb>.effort`。完成行為：呼叫 `validateClaudeCodeBlock` 對 `system.goal.effort === "super-high"` 與 `azure.fix.effort === "extreme"` 都回傳對應錯誤，而合法值無錯誤。驗證：在 `lib/ipc.effort.test.ts` 新增三個 case（System invalid、Azure invalid when active=system、全合法無錯誤）對應 `Settings UI Endpoint Section` 的「Inactive profile invalid effort still blocks Save」與「Save button enables when active=azure becomes fully populated」scenarios，先 RED 再 GREEN。

## 2. UI dropdown 替換（EndpointSection.tsx）

- [x] 2.1 [P] 將 `codebus-app/src/components/settings/EndpointSection.tsx` 中 System Profile sub-section 內三個 verb row 的 `effort` `<Input>` 替換為 shadcn `<Select>`，options 為 `SYSTEM_EFFORTS`，保留 `data-testid="system-effort-<verb>"`；當當前 state 值不在 `SYSTEM_EFFORTS` 時 trigger 顯示空（無選中文字）且元素帶 `aria-invalid="true"`。完成行為：對應 spec scenarios「System effort dropdown lists exactly three options」與「Legacy invalid effort value renders empty select trigger and flags validation」（後者僅 system 部分）。驗證：在 `EndpointSection.test.tsx` 新增兩個 RTL 測試斷言三個 option value 與順序，以及載入 `super-high` 時 trigger 為空 + `aria-invalid="true"`；先 RED 再 GREEN。
- [x] 2.2 [P] 將同檔 Azure Profile sub-section 內三個 verb row 的 `effort` `<Input>` 替換為 shadcn `<Select>`，options 共用 1.1 的 `SYSTEM_EFFORTS` 常數（不得重複硬編碼清單），保留 `data-testid="azure-effort-<verb>"`，invalid 值處理規則與 2.1 相同。完成行為：對應 spec scenarios「Azure effort dropdown lists exactly three options」與「Inactive profile invalid effort still blocks Save」中 azure 端 UI 行為。驗證：在 `EndpointSection.test.tsx` 新增兩個 RTL 測試斷言 azure 三個 option value 與順序，以及 `azure.fix.effort = "extreme"` 時該 `<select>` 帶 `aria-invalid="true"` 並可在 validation summary 找到 `claude_code.azure.fix.effort`；先 RED 再 GREEN。

## 3. 既存測試遷移與 Selecting-to-clear 行為

- [x] 3.1 更新 `EndpointSection.test.tsx` 與 `SettingsModal.test.tsx` 中所有以 `fireEvent.change` 對舊 effort Input 輸入文字的測試（含 active radio 切換、accordion 折疊保持值等案例），改為透過 RTL 操作新 `<Select>` 元件。完成行為：整個 `pnpm --filter codebus-app test` 套件綠燈，且新增的「Selecting a valid effort clears the invalid flag and enables Save」scenario 測試（從 `super-high` 切到 `medium` 後 Save 啟用且 aria-invalid 移除）通過。驗證：執行 `pnpm --filter codebus-app test EndpointSection SettingsModal lib/ipc.effort` 全綠。
