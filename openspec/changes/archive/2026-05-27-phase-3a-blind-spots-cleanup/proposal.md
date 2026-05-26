## Why

Phase 3A 的 i18n sweep（`i18n-sweep-phase-3a-followup`，2026-05-26 archived）跑完後，留下兩類 blind spot 必須拆出來收乾淨：

1. **`src/lib/ipc.ts` 5 處 validation 錯誤訊息**硬寫英文字串、繞過 i18n bundle，會直接被 `EndpointSection` 渲染成 `<li>{e.message}</li>` 給 user 看（form error 區塊）。違反 `app-shell` 的 **i18n Bundle Coverage Policy**（Pattern 6 涵蓋 `src/**/*.ts` outside `components/`）。
2. **i18n Bundle Coverage Policy 的 sweep procedure 漏一種結構**：Pattern 1a（單行 JSX text）抓得到 `<span>Install Codex first</span>`、但抓不到被 `{}` interpolation 切斷的 `<span>Install {provider.displayName} first; then reopen</span>`。這個 blind spot 在 `settings-language-switcher` apply 時抓到（`SettingsModal.tsx:258`），雖然該行已順手 ship、但 sweep procedure 本身的洞沒補進 spec、未來新增同類結構仍會漏。

不修：language dropdown 已存在但對 ipc.ts validation 錯誤訊息無感（5 處對切換 locale 沒反應）；Pattern 1a 的洞會讓未來新增 interpolation-split JSX 結構繼續溜過 sweep。

## What Changes

- 將 `validateClaudeCodeBlock` / `validateCodexBlock` 5 處 user-facing validation 錯誤訊息（`src/lib/ipc.ts:339/351/360/483/489` 區段附近）改為走 i18n bundle，使 form error 區塊 (`EndpointSection.tsx` line ~385) 顯示的訊息隨 active locale 切換。
- 新增 i18n bundle 鍵（en + zh）覆蓋這 5 處訊息，沿用既有 `settings.endpoint.validation.*` 命名空間 + camelCase 葉節點。
- 在 `openspec/specs/app-shell/spec.md` 的 **i18n Bundle Coverage Policy** 新增 **Pattern 1c**（JSX text with Latin split by `{}` interpolation）grep procedure，並在 `Scenario: 6-pattern sweep finds no policy violations` 升級為 7-pattern sweep。
- 補一支 `ipc.ts` validation i18n unit test，確認 validation 結果可被 i18n 層消費（具體形式由 apply 第一個 task 的架構決策決定）。

## Non-Goals

- **不重做** `SettingsModal.tsx:258` 的 install hint（已被 `settings-language-switcher` ship 進 `settings.providerCli.installHint`）。
- **不擴 sweep 範圍**：不新增 Pattern 2-7（除 Pattern 1c 外）；Pattern 1c 只補洞、不改 Pattern 1a / 1b / 2-6 的語意。
- **不動 component 層 i18n**：本 change 只動 `src/lib/ipc.ts`、`src/i18n/messages.ts`、`openspec/specs/app-shell/spec.md`、加 1 支 test。
- **不把純內部斷言誤拉進 scope**：若 5 處中任何一處實際是 dev-only 斷言（user 看不到、純 console），保留 raw 英文 + 加 comment 說明，不塞進 bundle。判斷準則由 apply 第一個 task 決定（見 design.md）。
- **不在本 change 重寫 `tStatic`**：spec line 1146 已聲明 `tStatic` 未來會 wire 到 settings store，那是另一支 change；本 change 若選 `tStatic` 路線就接受「ipc.ts 訊息不立即 reactive，須等 page reload」的限制（如果選 LocalizedError 路線則自動 reactive）。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-shell`: 在 **i18n Bundle Coverage Policy** 加 Pattern 1c grep procedure（補洞 phase 3A sweep 對 `{}` interpolation-split JSX text 的失效），並把 6-pattern sweep scenario 升為 7-pattern。Scope A 的 ipc.ts validation 訊息修法屬於 policy 既有覆蓋面，不新增 requirement、只是 bring code into compliance。

## Impact

- Affected specs: `app-shell` (i18n Bundle Coverage Policy — 加 Pattern 1c + scenario 升為 7-pattern)
- Affected code:
  - Modified:
    - `codebus-app/src/lib/ipc.ts` (5 處 validation 訊息 wire 進 i18n bundle 或經 LocalizedError-shaped 結構 — 路線在 apply 第一個 task 決定)
    - `codebus-app/src/i18n/messages.ts` (新增 en + zh validation 鍵)
  - New:
    - `codebus-app/src/lib/ipc.validation-i18n.test.ts` (validation i18n unit test — 確保 5 處訊息可被 i18n 層消費；檔名若已被佔用則延用 `ipc.test.ts` 或合進 `codex-validation.test.ts`，由 apply 決定)
    - `codebus-app/scripts/.blind-spots-smoke/` (真實 en-locale CDP smoke 截圖落點)
  - Removed: (none)
