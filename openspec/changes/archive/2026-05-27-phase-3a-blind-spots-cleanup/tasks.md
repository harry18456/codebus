## 1. Pre-apply 盤點 + Decision 1 路線決策

- [x] 1.1 落實 design.md `Decision 1 · Validation message i18n seam (apply 第一個 task 落實)`：Read `src/lib/ipc.ts` 5 處（line 339 / 351 / 360 / 483 / 489 區段）validation throw shape 與 `EndpointSection.tsx` `endpoint-validation-summary` (`<li>{e.message}</li>`) 渲染證據 + Read `src/i18n/errors.ts` LocalizedError 現況；15 分鐘內依 design.md Decision 1 判斷準則決定走路線 1（tStatic）或路線 2（LocalizedError-shaped），決策結果回寫進 design.md Decision 1 結果段並列出每處屬於 user-facing 或內部斷言。驗證：design.md 出現「Decision 1 結果」段、含 5 處逐項分類與所選路線。
- [x] 1.2 [P] 落實 design.md `Decision 2 · i18n key 命名（pre-apply 已確認 convention）`，並交叉確認新 key 符合 `app-shell` spec 的 i18n Bundle Coverage Policy 對 `src/**/*.ts` 的 Pattern 6 涵蓋：Grep `codebus-app/src/i18n/messages.ts` 內 `settings.endpoint.*` 既有 key，比對 Decision 2 預設命名（`settings.endpoint.validation.*` namespace + camelCase 葉節點）與既有 `validationSummaryHeading` / `saveButtonIncompleteTitle` 慣例對齊；若發現不符（例如葉節點實際是 snake_case）就更新 design.md Decision 2 預設清單。驗證：design.md Decision 2 key list 與真實 messages.ts 取樣對齊（每個新 key 都符合既有 layer 命名）。

## 2. i18n bundle 鍵新增（Decision 2 · i18n Bundle Coverage Policy）

- [x] 2.1 在 `codebus-app/src/i18n/messages.ts` 加入 en + zh validation key 共 **7 key**（按 task 1.1 校準結果）：`settings.endpoint.validation.azureProfileRequired` / `baseUrlRequired` / `apiVersionRequired` / `keyringServiceRequired` / `deploymentNameRequired`（含 `{verb}`） / `effortInvalid`（含 `{verb}` + `{allowed}`） / `systemModelRequired`（含 `{verb}`），en 沿用既有英文 wording、zh 對應中譯。驗證：`pnpm tsc` 綠（messages.ts 的 `Record<keyof typeof messages.en, string>` 雙 bundle parity check 通過）。

## 3. ipc.ts wire（落實 Decision 1 結果）

- [x] 3.1 將 `src/lib/ipc.ts` 內 `validateClaudeCodeBlock` / `validateCodexBlock` **12 處** user-facing validation 訊息（C1-C6 + X1-X6，見 design.md Decision 1 結果段表格）全部改走 i18n 路線 2：改 `ClaudeCodeValidationError` shape 為 `{field: string, key: MessageKey, vars?: Record<string, string|number>}`（沿用 `src/i18n/errors.ts` `LocalizedError` shape）並對應 push 7 個 i18n key 之一。驗證：`pnpm tsc` 綠 + 對 12 處逐一 grep 確認沒有殘留 hard-code 英文 message literal（`grep -n 'message: ' src/lib/ipc.ts` 應只剩 type 定義或 zero hit）。
- [x] 3.2 將 `codebus-app/src/components/settings/EndpointSection.tsx` `endpoint-validation-summary` 區塊 `<li>{e.message}</li>` 改為 `<li>{t(e.key, e.vars)}</li>`（含必要的 `useT` import）；form error 區塊顯示行為對 active locale 反應 reactive。驗證：`pnpm tsc` 綠 + 跑 `pnpm test` 中 EndpointSection 既有 test 全綠（assertion 對齊新 shape，驗 e.key）。
- [x] 3.3 [P] 將 `codebus-app/src/lib/codex-validation.test.ts` + `codebus-app/src/lib/ipc.effort.test.ts` 對 `.message` 的 assertion 改為對 `.key` + `.vars` 的等價斷言，保留每個 case 的覆蓋面。驗證：`pnpm test` 兩支檔案綠 + 每個原 `.message` 斷言可逐一對應到新斷言。

## 4. Test 落實（Decision 4 · Test 落點 · i18n Bundle Coverage Policy）

- [x] 4.1 落實 design.md `Decision 4 · Test 落點`：新增 `codebus-app/src/lib/ipc.validation-i18n.test.ts`，覆蓋：每個新 i18n key（7 key）在 en 與 zh bundle 都存在（messages 表查找）+ 12 處 validation 觸發後回傳結構可被 i18n 層消費（驗 `e.key` 為合法 `MessageKey`、`e.vars` 含預期 placeholder 對應每個 site）。驗證：`pnpm test` 對該 test 檔綠。
- [x] 4.2 跑 `pnpm tsc` + `pnpm test` 全 repo 雙綠，Implementation Contract Acceptance 1 + 2 通過。驗證：兩個 command 在乾淨 working tree 下 exit code = 0。

## 5. Spec Pattern 1c sweep procedure 真實驗證（Decision 3 · Spec Pattern 1c 命名（不複用 1a / 1b）+ LocalizedError NOTE 段 · i18n Bundle Coverage Policy）

- [x] 5.1 在 `codebus-app/` 目錄真實跑 spec 內 Pattern 1c grep command（`grep -rPn '>[^<>{}]*\{[^}]+\}[^<>{}]*[A-Za-z]+[^<>]*<' src/components/ --include='*.tsx' | grep -v '.test.' | grep -v 't("'`），將結果記到 `codebus-app/scripts/.blind-spots-smoke/pattern-1c-results.txt`，並逐行 reconcile：每筆要嘛是已修 site（如 `SettingsModal.tsx:258` 已 ship via `settings-language-switcher`、跑出來時應已是 `t(...)` 形式）、要嘛是 Cat D / runtime keyword exception、要嘛是 known non-sweep site；不可有未 accounted line。驗證：reconcile 報告中每行有對應 disposition、無 unaccounted line。
- [x] 5.2 確認 spec delta `openspec/changes/phase-3a-blind-spots-cleanup/specs/app-shell/spec.md` 末尾的 LocalizedError architectural-guard NOTE 段已涵蓋「.ts plain-string user-facing error 不靠 grep、靠 TS 型別」原則；交叉檢查本 change 12 處實作（task 3.1）的 `ClaudeCodeValidationError` 型別 + `EndpointSection.tsx` consumer surface 與 NOTE 段內 contract 完全一致（型別字面 `{key: MessageKey, vars?: Record<string, string | number>}` 與實作對齊）。驗證：spec NOTE 段內 TypeScript shape 與 `codebus-app/src/lib/ipc.ts` 內 `ClaudeCodeValidationError` 定義 textually 對應、不漂移。

## 6. 真實 en-locale CDP smoke（Implementation Contract Acceptance 3 · i18n Bundle Coverage Policy）

- [x] 6.1 啟動 app（`cargo tauri dev --remote-debugging-port=9222`）+ 透過 CDP `codebus-app/scripts/cdp.mjs` 連線 + 開 Settings + Language dropdown 切 English + 透過 form input 故意觸發 7 個 i18n key 對應 12 處 user-facing site 中的代表 site（每個 key 至少 1 個 site）；form error `endpoint-validation-summary` 顯示英文訊息（wording 同 messages.ts en value）。每處截圖存 `codebus-app/scripts/.blind-spots-smoke/<key>-en.png`。驗證：7 張截圖內 `<li>` 文字逐一比對 messages.ts en value 通過。
- [x] 6.2 切回 zh + 完整重啟 app（關閉 dev server + 重啟） + 重新觸發同樣 7 個 key 代表 site；form error 顯示中文訊息（wording 同 messages.ts zh value）。每處截圖存 `codebus-app/scripts/.blind-spots-smoke/<key>-zh.png`。驗證：7 張截圖內 `<li>` 文字逐一比對 messages.ts zh value 通過。
- [x] 6.3 在 6.1 場景延伸：保持 app 不重啟、回 Settings 切 Language dropdown 從 English → 中文，預期 `endpoint-validation-summary` 內訊息立即切換成中文（不重啟、不 remount）；再切回英文同樣立即生效。截圖存 `codebus-app/scripts/.blind-spots-smoke/dropdown-reactive-zh.png` + `dropdown-reactive-en.png`。驗證：兩張截圖各自比對對應 locale 的 messages.ts wording 通過（路線 2 reactive contract 達成）。
