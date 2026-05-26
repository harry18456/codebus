## Context

`i18n-sweep-phase-3a-followup`（2026-05-26 archived）完成 phase 3A 6-pattern sweep 後，留下兩類 blind spot 必須拆出來收：

1. **`src/lib/ipc.ts` 5 處 user-facing validation 訊息硬寫**。`validateClaudeCodeBlock` / `validateCodexBlock` 回傳 `{field, message}[]`，其中 `message` 是英文字串（如 "base_url is required when active=azure"）。`EndpointSection.tsx` 直接 `<li>{e.message}</li>` 渲染給 user 看（line ~385，`endpoint-validation-summary` `role="alert"`）。違反 `app-shell` **i18n Bundle Coverage Policy** 對 `src/**/*.ts` outside `components/` 的 Pattern 6 涵蓋（spec line 1077 / 1090）。
2. **Sweep procedure 漏一種結構**。Pattern 1a（單行 JSX text 連續 Latin）抓 `<span>Install Codex first</span>` OK、但抓不到 `<span>Install {provider.displayName} first; then reopen</span>`，因為 Latin 被 `{}` interpolation 切成兩個短片段、單獨都不過 Latin-chunk 門檻。該 blind spot 在 `settings-language-switcher` apply 時被人眼抓到（`SettingsModal.tsx:258`），雖然該行已 ship、但 sweep procedure 本身的洞沒補進 spec、未來新增同類結構仍會漏。

**Spec 已建立的 seam**：`app-shell` spec line 1144 已明確聲明 "Backend errors surfaced through `LocalizedError` SHALL render in the active locale because the toast layer resolves them through `useT` / `useLocale` at display time"。LocalizedError 是 reactive 翻譯的既定 pattern。

**Spec 對 `tStatic` 的現況**：spec line 1146 — "A standalone synchronous helper `tStatic` that resolves locale outside the React tree is out of scope for this requirement and MAY continue to read `navigator.language` directly until a follow-up change wires it to the store." 也就是說 `tStatic` 目前讀 `navigator.language`，**不認** settings store 的 `app.locale_override`。如果走 `tStatic` 路線，ipc.ts 的訊息切 Language dropdown 不會立即反映、得等下次 `tStatic` 被呼叫（多半要重啟）。

## Goals / Non-Goals

**Goals:**

- ipc.ts 5 處 user-facing validation 訊息 wire 進 i18n bundle，使 `endpoint-validation-summary` 在切換 Language dropdown 後即時切換語言（或退而求其次：跟著 active locale，重啟後正確）。
- 新 i18n key 命名沿用既有 `settings.endpoint.*` namespace + camelCase 葉節點、不引入 LLM 慣性 camelCase 違規。
- 在 `app-shell` spec 補 Pattern 1c grep procedure + 升級 sweep scenario 為 7-pattern，使未來同類 interpolation-split JSX text 不再從 sweep 溜過。
- 真實 en-locale CDP smoke 驗 5 處訊息：故意觸發後在 form error 區塊顯示英文、切回中文後重啟仍正確。

**Non-Goals:**

- **不重做** `SettingsModal.tsx:258`（已 ship via `settings-language-switcher`）。
- **不重寫 `tStatic`** wire 到 settings store。spec line 1146 已說「另一支 change」，本 change 不擴。
- **不擴 sweep 範圍** Pattern 2-7（Pattern 1c 例外、是補洞）；Pattern 1a / 1b / 2-6 語意不動。
- **不動 component 層 i18n** 或其它非本次涉及檔案。
- **不把純內部斷言誤拉進 scope**：若 apply 第一個 task 的盤點發現 5 處中任何一處實際是 dev-only 斷言（user 看不到、純 console），保留 raw 英文 + 加 comment 說明，不塞進 bundle。

## Decisions

### Decision 1 · Validation message i18n seam (apply 第一個 task 落實)

#### Decision 1 結果（apply task 1.1 校準 2026-05-27）

**Scope 校準**：真實 grep `src/lib/ipc.ts` 找到 **12 處** hard-coded validation message 位置（非 propose AUDIT trailer 寫的 5 處）：

| 編號 | 行 | Function | active profile | 觸發條件 | Message wording |
|---|---|---|---|---|---|
| C1 | 350 | validateClaudeCodeBlock | azure | `block.azure == null` | `"Azure profile is required when active=azure"` |
| C2 | 357 | validateClaudeCodeBlock | azure | `base_url` 空 | `"base_url is required when active=azure"` |
| C3 | 363 | validateClaudeCodeBlock | azure | `keyring_service` 空 | `"keyring_service is required when active=azure"` |
| C4 | 370 | validateClaudeCodeBlock | azure | `az[verb].model` 空 | `` `${verb} deployment name is required when active=azure` `` |
| C5 | 382 | validateClaudeCodeBlock | system | effort 非 enum | `` `${verb} effort must be one of ${SYSTEM_EFFORTS.join(" / ")}` `` |
| C6 | 391 | validateClaudeCodeBlock | azure | effort 非 enum | 同 C5 |
| X1 | 499 | validateCodexBlock | azure | `block.azure == null` | 同 C1 |
| X2 | 504 | validateCodexBlock | azure | `base_url` 空 | 同 C2 |
| X3 | 507 | validateCodexBlock | azure | `api_version` 空 | `"api_version is required when active=azure"` |
| X4 | 510 | validateCodexBlock | azure | `keyring_service` 空 | 同 C3 |
| X5 | 514 | validateCodexBlock | azure | `az[verb].model` 空 | 同 C4 |
| X6 | 520 | validateCodexBlock | system | `block.system[verb].model` 空 | `` `${verb} model is required when active=system` `` |

AUDIT trailer 給的行號（339/351/360/483/489）不指向訊息行，疑似舊版檔案筆誤；archive 階段順手更新 trailer 標 12 處 + 正確行號。

**12 處 user-facing 屬性確認**：12 處全部流入 `EndpointSection.tsx` `endpoint-validation-summary` `<li>{e.message}</li>`（`role="alert"`、給 user 看的 form error 區塊）。**無內部斷言、無 dev-only**。

**選定路線 = 路線 2（LocalizedError seam）**，理由：
- 12 處皆 user-facing 切 Language dropdown 應 reactive 立即生效；路線 1 的 `tStatic` 不認 dropdown override（spec line 1146）會留 UX 異味。
- LocalizedError type seam 是「ipc.ts → React 邊界 user-facing error 的型別護欄」：未來新增 user-facing error 想偷懶 plain string、TypeScript 直接擋。比 grep procedure 可靠（grep 在 .ts 檔抓 plain-string user-facing 訊息會夾大量 false positive：internal log / typedef / non-user-facing error / Error subclass message arg）。
- 跟 spec line 1144 既有 `LocalizedError` pattern 對齊（`errors.ts:8-11` 已定 `{key: MessageKey, vars?}` shape，validation site 直接複用）。

**型別具體形狀**（reuse `LocalizedError` from `errors.ts`）：
```ts
export interface ClaudeCodeValidationError {
  field: string
  // LocalizedError-shaped: render via t(e.key, e.vars) in JSX
  key: MessageKey
  vars?: Record<string, string | number>
}
```
（不再有 `message: string` 欄位。`EndpointSection.tsx` 內 `<li>{e.message}</li>` 改為 `<li>{t(e.key, e.vars)}</li>`。）

**Collapsing 12 處 → 7 unique i18n keys**（共用 placeholder 後）：

| Key | Sites | Placeholders |
|---|---|---|
| `settings.endpoint.validation.azureProfileRequired` | C1, X1 | — |
| `settings.endpoint.validation.baseUrlRequired` | C2, X2 | — |
| `settings.endpoint.validation.apiVersionRequired` | X3 | — |
| `settings.endpoint.validation.keyringServiceRequired` | C3, X4 | — |
| `settings.endpoint.validation.deploymentNameRequired` | C4, X5 | `{verb}` |
| `settings.endpoint.validation.effortInvalid` | C5, C6 | `{verb}`, `{allowed}` |
| `settings.endpoint.validation.systemModelRequired` | X6 | `{verb}` |

#### Decision 1 原始路線比較（pre-apply 紀錄、保留以利後追溯）

- **路線 1 · 直接 tStatic() 走 i18n bundle**：ipc.ts import `tStatic`、`validateXBlock` 內 `message: tStatic("settings.endpoint.validation.xxx", { verb })`。
  - 優：實作簡單、scope 小（只 wire + 1 個 import）。
  - 劣：`tStatic` 讀 `navigator.language` 不認 Language dropdown（spec line 1146），切 dropdown 不立即生效；user 看到的訊息只有重啟後才會切回中文。
  - 適用：訊息實際只是工程師除錯用（user 觸發機率低、reload 一次能接受）。**本 change 12 處皆 user-facing、不適用。**

- **路線 2 · 走 LocalizedError-shaped 結構** (selected)：細節見上方「Decision 1 結果」。

### Decision 2 · i18n key 命名（pre-apply 已確認 convention）

- 沿用既有 `settings.endpoint.<sub>.<leaf>` namespace，新 sub 取 `validation`。
- 葉節點 camelCase（跟 `validationSummaryHeading`、`saveButtonIncompleteTitle` 對齊）。
- 預計新 key（最終由 apply 落實時依 5 處實際訊息對應）：
  - `settings.endpoint.validation.azureProfileRequired`
  - `settings.endpoint.validation.baseUrlRequired`
  - `settings.endpoint.validation.keyringServiceRequired`
  - `settings.endpoint.validation.apiVersionRequired`（codex 專用）
  - `settings.endpoint.validation.deploymentNameRequired`（用 `{verb}` placeholder 共用 claude / codex）
  - `settings.endpoint.validation.systemModelRequired`（用 `{verb}` placeholder，codex system profile）
- `effort` 訊息現有 `${verb} effort must be one of …`，apply 時若該訊息屬於 user-facing scope 一併納入：`settings.endpoint.validation.effortInvalid` with `{verb}` + `{allowed}` placeholders。

### Decision 3 · Spec Pattern 1c 命名（不複用 1a / 1b）+ LocalizedError NOTE 段

`app-shell` spec line 1092 已聲明 Pattern 1 拆成 1a（單行 JSX）+ 1b（多行 indented JSX text）。本次新增 pattern 命名為 **Pattern 1c**（JSX text with Latin split by `{}` interpolation），與 1a / 1b 同屬「JSX text node」家族但結構不同。

- 6-pattern sweep scenario 升級為 7-pattern（標題 + 表格 + 行內參照都要同步更新）。
- 在新增 Pattern 1c 後追加一段解釋文字、說明它補 1a / 1b 對 `{}` interpolation 的失效（沿用 spec 既有「Pattern N 後加註」風格、line 1092 / 1094 / 1096 三段是先例）。

**新增 NOTE 段（apply 1.1 校準後加入）**：spec i18n Bundle Coverage Policy 末尾加一段非 Pattern 的 architectural guard 說明，給未來 reviewer 解釋「為什麼 .ts 層 plain-string user-facing error 不靠 grep 偵測」：

> `.ts` layer plain-string user-facing errors (e.g. validation messages returned to React form components) SHALL NOT be detected by sweep patterns because semantic grep on `message: "<Latin>"` produces high false-positive volume (internal logs, typedef literals, non-user-facing error subclass message args, etc.). Such sites SHALL instead be guarded architecturally: ipc.ts → React user-facing error data SHALL carry a `LocalizedError`-shaped contract (`{key: MessageKey, vars?}` from `src/i18n/errors.ts`), and TypeScript's compile-time type check SHALL enforce that no new user-facing error site can degrade to a plain `string` message. Internal-only error data (dev console, logs) MAY keep plain strings without compromising the policy.

此 NOTE 段不是新 Pattern、不擴 sweep 範圍；它是給 reviewer / 未來 follow-up 解釋「為什麼 ipc.ts 的 12 處 plain-string + template-literal 訊息經本 change 一次性 wire 後，未來新增同類 site 不靠 sweep 也不會 regress」的依據。

### Decision 4 · Test 落點

- 新增 1 支 validation i18n 對應 test 確保新 i18n key 在 en + zh bundle 都有對應、且 5 處 validation site 觸發後可被 i18n 層消費。
- 預設落點 `codebus-app/src/lib/ipc.validation-i18n.test.ts`。若 apply 走路線 2 改 shape、既有 `codex-validation.test.ts` + `ipc.effort.test.ts` 的 `.message` assertion 需同步調整（assert on `messageKey` + `messageVars`），由 apply 的 task 涵蓋。

## Implementation Contract

**Observable behavior（5 處 validation 訊息）：**

- WHEN active locale = `en`（包含切 Language dropdown 後）AND user 觸發 5 處任一 validation（例：codex active=azure 但 base_url 空白）
- THEN `endpoint-validation-summary` 區塊 `<li>` 顯示英文訊息（如 "base_url is required when active=azure"，wording 由 i18n bundle en value 控制）
- AND 切回 zh（或 zh active）後同一觸發顯示中文訊息（wording 由 zh value 控制）
- 路線 2 的 reactive 行為：dropdown 切換不需重啟、訊息立即切。路線 1 的退化：dropdown 切換不立即生效、重啟後正確。

**Interface（路線 2 走的話）：**

- `ClaudeCodeValidationError` shape：`{field: string, messageKey: MessageKey, messageVars?: Record<string, string|number>}`
- `EndpointSection` 渲染：`<li key={e.field}>{t(e.messageKey, e.messageVars)}</li>`
- 既有公開 surface（`validateClaudeCodeBlock` / `validateCodexBlock`）函數簽名只變回傳 shape、不變函數名稱與參數。

**Interface（路線 1 走的話）：**

- `ClaudeCodeValidationError` shape 不變（`{field, message}`）。
- `validateXBlock` 內 `message: tStatic("settings.endpoint.validation.xxx", { ... })`。
- 接受 `tStatic` 不認 dropdown 的限制（spec line 1146 已 carve out）。

**Failure modes：**

- 若 apply 第一個 task 發現 5 處中部分為 dev-only 斷言（user 看不到）→ 該處保留 raw 英文 + 加 comment、不塞進 bundle；tasks.md 對應 task 寫成「保留 raw + 加 comment」。
- 若 apply 過程發現 5 處外還有同類 site（盤點時擴）→ stop 找 user 對齊、不在本 change 擴 scope；額外 site 留給另一支 follow-up change。

**Acceptance criteria：**

1. `pnpm tsc` 綠（路線 2 改 shape 後既有 test assertion 不能爆）
2. `pnpm test` 綠，含新增的 ipc.ts validation i18n test（en / zh 雙 bundle 都有對應 key、5 處訊息可被 i18n 層消費）
3. 真實 en-locale CDP smoke：開 app + Settings 切 English + 故意觸發 5 處 validation + 截圖存 `codebus-app/scripts/.blind-spots-smoke/`；預期 5 處 form error 顯示英文。切回中文 + 重啟後重新觸發、顯示中文（路線 1 必驗；路線 2 也驗，加驗 dropdown 切換立即生效）。
4. Pattern 1c grep 真實跑一遍（command 由 spec scenario 內表格定義）、確認除了 known keep + 已修 site（含 `SettingsModal.tsx:258` 已 ship）外無新發現。

**Scope 邊界（in / out）：**

- 在 scope：`codebus-app/src/lib/ipc.ts`、`codebus-app/src/i18n/messages.ts`、`codebus-app/src/lib/ipc.validation-i18n.test.ts`（or 合進既有 test）、`openspec/specs/app-shell/spec.md`（i18n Bundle Coverage Policy 段）、`codebus-app/scripts/.blind-spots-smoke/`（截圖）。
- 路線 2 的話加 `codebus-app/src/components/settings/EndpointSection.tsx`（`endpoint-validation-summary` 內渲染）、`codebus-app/src/lib/codex-validation.test.ts` + `codebus-app/src/lib/ipc.effort.test.ts`（assertion 對齊新 shape）。
- 出 scope：component 層 i18n、`tStatic` 重寫、新增 Pattern 2-7 除 1c 外、`SettingsModal.tsx:258` 重做。

## Risks / Trade-offs

- **Risk: 路線 2 改 shape 連帶動 EndpointSection.tsx + 兩支既有 test** → Mitigation: apply 第一個 task 估時 + 用 tsc 找所有 `.message` consumer + 一次性更新；若估時超過 60-80 min 上限 stop 找 user。
- **Risk: 路線 1 限制（dropdown 不立即生效）被 user 嫌** → Mitigation: 預設選路線 2；路線 1 只在 5 處皆 dev-only 斷言時才用，而判斷準則已寫死 EndpointSection 渲染證據。
- **Risk: Pattern 1c 命名與既有 1a / 1b 混淆** → Mitigation: spec 新增 pattern 後加註解釋 1c 的補洞角色（沿用 1092 / 1094 / 1096 三段先例）、grep command 表格區分明確。
- **Risk: 60-80 min 工時上限不夠（特別是路線 2 連帶改 test）** → Mitigation: tasks.md 切顆粒度足夠細的 task、apply 跑 task 時計時，超過上限 stop 找 user。
- **Risk: 真實 en-locale smoke 在 Windows WebView2 觸發 5 處 validation 不一定有 form input 入口（如 codex 還未啟用、Azure profile 還沒顯示）** → Mitigation: smoke 前先在 Settings 切到 codex active=azure 確保 5 處表單都可見；若有 site 在 UI 不可達就 mark known-untestable + 改用 unit test 驗 i18n key 存在性。
