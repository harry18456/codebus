## Context

Phase 3A 完成 i18n bundle wiring 後，所有 user-facing 字串都走 `useT(key)` → 透過 `useLocale()` 解析 locale。`useLocale` 目前只看 `navigator.language`，且 `v1` 的 `app-shell` spec 明文禁止 language switcher（Forbidden Behaviors 區 + Settings modal scenario）。AUDIT.md ST14 已要求補上 user-facing 切換，本 design 描述 store 接線、precedence、reactive 行為與 i18n bundle 對齊。

Settings store（`useSettingsStore`）目前是 zustand store，已有 `config: GlobalConfig` round-trip 到 `settings.json`。Locale override 需要：(a) UI 在 Settings modal 可改、(b) 寫進 `GlobalConfig`、(c) `useLocale` reactive 訂閱、(d) 重啟仍 sticky。

`LocalizedError` 是把 backend 錯誤 normalize 成 `{ key, vars }` 的 payload，由 Toast 在 render time 透過 `useT` 解析 — 自動跟著 `useLocale` 變動，免額外接線。

## Goals / Non-Goals

**Goals:**

- Settings modal 加 Language dropdown，3 個 fixed 選項（Auto / 中文 / English）
- `useLocale` reactive 從 settings store 讀 override，立即切換不需重啟
- `locale_override` 持久化進 `settings.json`，重啟仍 sticky
- Backend error 訊息透過 LocalizedError → useT → useLocale 自動跟著切
- v1 `app-shell` spec 翻轉：移除「language switcher forbidden」並改寫對應 scenario

**Non-Goals:**

- 不支援 BCP-47 完整 locale（例如 `zh-Hant-TW` / `en-GB`），只支援 `zh` / `en` / `null` 三值
- 不引入第三種語系
- 不重構 `tStatic` path（React 樹外 code path，獨立 detection，followup 處理）
- 不引入 i18n framework（react-intl / i18next 等）
- 不改 SettingsModal 既有 dropdown 樣式 / Phase 4 layout

## Decisions

### Precedence: hook arg > store override > navigator.language

`useLocale(override?: Locale)` 的 `override` arg 已存在，給測試 mock 用，要保留。Production 從 settings store 讀 `locale_override`。順序固定：

1. Hook arg `override`（非 nullish）→ 直接用
2. Store `locale_override`（非 null）→ 用 store 值
3. 系統 `navigator.language` 啟始 `zh` → `"zh"`，其他 → `"en"`

理由：測試需要 deterministic mock；hook arg 比 store 優先讓測試不必先 setup store。`null` store override 等同 「我要 auto」，明確區分於「未設定」。

### Reactive: zustand subscribe vs. one-time read

zustand store 的 hook 形式（`useSettingsStore(state => state.config.app?.locale_override)`）天然 reactive — 任何 component 用 `useLocale()` 都會在 store 變動時 re-render。不需要 React context、不需要 forceUpdate。

替代方案：在 `App.tsx` 用 context provider 包一層 — 否決，因為 zustand 已有全域 subscription，多包一層 context 是多餘抽象。

### Schema 位置: `app.locale_override` vs. 新 top-level

放在 `GlobalConfig.app.locale_override: "zh" | "en" | null`。`app.*` 已是 UI-only 設定 namespace（`app.quiz.*` 等），語系是 UI 偏好，歸 `app.*` 一致。Zod schema 對應加 `.nullable()`，`null` 預設值表示「auto detect」。

替代方案：top-level `locale_override` — 否決，會污染 root 命名空間，且 `app.*` 已是既有慣例。

### LocalizedError 路徑 — 不需改

`LocalizedError` 由 `useT` 解析，`useT` 內部呼叫 `useLocale()`。Store override 動 → `useLocale` 回新值 → `useT` re-resolve → Toast re-render 用新 locale 顯示錯誤訊息。整條 path 都靠 hook chain，無 imperative locale 讀取。本 change 不必改 `errors.ts`。

**例外**：`tStatic`（若存在）是 React 樹外的同步 helper，會自己讀 `navigator.language`。本 change scope 不處理（Non-Goals 明列）。

### 順手吃 SettingsModal:258 install hint

`Install {provider.displayName} first; then reopen Settings.` 是 phase-3a-blind-spots-cleanup 的 Scope B。en-locale smoke 跑到時必看到，順手 wire 進 `settings.providerCli.installHint` key（含 `{name}` placeholder），en/zh bundle 都填。Archive 階段 update AUDIT 把該 trailer 的 Scope B 移除。

替代方案：留給 phase-3a-blind-spots-cleanup 之後處理 — 不採用，因為 smoke 跑英文時看得到，順手解掉成本 < 之後另外開 change 的 overhead。

## Implementation Contract

**Behavior:**

- User 開 Settings modal → 在 Provider 區下方 / PII 區上方看到 Language dropdown，三個選項：「Auto」、「中文」、「English」
- 切英文 → modal 內所有可翻譯文字（含其他 fields 的 label）以及背景的 Workspace / Lobby 文字立即切英文
- 切中文 → 同上立即切中文
- 切 Auto → locale 回到 `navigator.language` 推導值（`zh-*` → `"zh"`，其他 → `"en"`）
- 關閉 app + 重啟 → 上次選擇仍生效
- Backend error toast 跟著當前 locale 顯示

**Interface / data shape:**

- `Locale = "zh" | "en"`
- `GlobalConfig.app.locale_override?: Locale | null`（新欄位）
- `useLocale(override?: Locale): Locale`（簽名不變，內部新增 store 訂閱）
- Settings modal 新 component `LanguageSection`（或 inline 進 SettingsModal，視 SettingsModal 既有結構決定 — apply 階段判斷）
- i18n key 4 條：`settings.language.label` / `settings.language.auto` / `settings.language.zh` / `settings.language.en` + 1 條順手吃：`settings.providerCli.installHint`

**Failure modes:**

- `locale_override` 在 settings.json 是非法值（既不是 `"zh"` 也不是 `"en"` 也不是 `null`）→ Zod parse 失敗 → 整個 settings load 失敗 → 既有 error path 顯示 toast。Settings store 既有錯誤處理覆蓋，本 change 不另外處理
- Settings.json 完全沒有 `app.locale_override` 欄位（舊版升級） → Zod `.nullable().optional()` 接住，視為 `null`（auto）
- 切換時 IPC save 失敗 → 既有 `save()` error path（toast）覆蓋，本 change 不另外處理

**Acceptance criteria:**

- `pnpm tsc` 綠
- `pnpm test` 綠（含新 `LanguageSection.test.tsx`、`useLocale` store 訂閱 test、`settings` store `locale_override` round-trip test）
- 真實 CDP smoke 通過全部 5 條：預設中文 → 切英文 reactive → 重啟仍英文 → 切 Auto → 重啟回中文（依系統）；每步截圖存 `codebus-app/scripts/.lang-switcher-smoke/`
- 切英文後觸發 backend error（例如填 invalid base_url 然後 Save）→ toast 顯示英文錯誤
- `~/.codebus/settings.json` 看得到 `app.locale_override: "en"` 寫入正確
- `app-shell` spec 對應 forbidden 規則 + scenario 已刪除 / 改寫

**Scope boundaries:**

In scope:
- Settings modal Language dropdown UI + i18n keys
- `useLocale` 接 store
- `locale_override` 在 GlobalConfig schema / IPC round-trip
- Backend error 訊息隨 locale 切（自動透過既有 chain，僅驗證）
- 順手吃 `settings.providerCli.installHint`
- `app-shell` spec 翻轉 v1 forbidden

Out of scope:
- `tStatic` path 接 store
- 第三種語系 / BCP-47 完整支援
- Workspace / Lobby / Quiz 任何 UI 重排
- SettingsModal 既有 dropdown 樣式 / Phase 4 layout 改動
- 任何 backend(`codebus-core` / `codebus-cli`) locale 行為改動

## Risks / Trade-offs

- [tStatic path 不一致] → Non-Goals 明列；本 change 留註解標 followup。若 tStatic 在某 React 樹外 code path 被觸發（例如 toast 顯示在 React mount 之前），會 fallback 到 `navigator.language` 而非 store override。實機 smoke 不會碰到（all toast 都在 mount 之後），但若 followup 沒做，未來新 code path 可能踩雷
- [Zod schema 升級] → 舊 settings.json 沒有 `app.locale_override`，要確保 `.nullable().optional()` 不會 break。Test 覆蓋舊 config round-trip
- [Reactive re-render 全域成本] → zustand 只在 selector 回傳值變動時 re-render；`useLocale()` 在 selector 抓 `locale_override` 單值，re-render 成本可控
- [Settings modal 翻譯 race] → user 切語系時 modal 自己也要翻；既有 useT 已 reactive，不需特別處理。實機 smoke 第一步就驗
- [v1 spec 翻轉] → app-shell 的 v1 forbidden 規則是現存 user contract；翻轉後其他依賴該規則的 spec / doc 可能需要 sync。Apply 階段 grep 確認

## Migration Plan

無 backend / data migration。前端純加欄位 + UI。舊 settings.json 自動視為 `locale_override: null`（auto），行為與本 change 之前一致 — backward compatible。

Rollback：revert 整個 change，user 端 settings.json 殘留的 `app.locale_override` 欄位會被舊 schema 忽略（Zod `.passthrough()` 或既有 unknown field 處理），不會壞既有行為。
