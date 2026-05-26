## Why

Phase 3A + 3A followup 完成後，54 處 hard-code 已 wire 進 i18n bundle，infrastructure 就緒；但 v1 規範明文禁止 language switcher，user 只能依系統 locale 自動偵測。實機使用上「想在英文系統看中文」或「想在中文系統看英文」做不到，且 `codebus-app/src/hooks/useLocale.ts` 第 8 行註解明寫「v1 has no language switcher」，`useLocale(override?: Locale)` 預留接口但沒接 settings store。AUDIT.md `## 06 · Settings Modal` 的 ST14 已寫好 scope，本 change 補上 user-facing 切換並翻轉 v1 forbidden 規則。

## What Changes

- **BREAKING** 翻轉 `app-shell` v1 規範：移除「Language switcher UI」forbidden bullet 與「Settings modal has no theme or language controls」scenario；改寫為「Settings modal exposes language override」requirement
- `SettingsModal` 在 Provider 區下方 / PII 區上方加入 Language dropdown（3 個 fixed 選項：Auto / 中文 / English）
- `settings` store 新增 `locale_override: Locale | null` 欄位，Zod schema 加 `.nullable()`，settings.json 預設 `null`
- `ipc.ts` settings load/save round-trip 帶 `locale_override`
- `useLocale` 從 settings store 讀取 override，優先順序：override arg > store `locale_override` > `navigator.language` auto detect
- `i18n/errors.ts` LocalizedError path 確認尊重 store override（後端錯誤訊息切英文後也跟著英文）
- `i18n/messages.ts` 新增 4 條 keys：`settings.language.label`、`settings.language.auto`、`settings.language.zh`、`settings.language.en`
- 順手吃 `SettingsModal.tsx:258` 的中英混雜 hint：wire 進新 i18n key `settings.providerCli.installHint`（含 `{name}` placeholder）

## Non-Goals

- 不動 `tStatic` 那條獨立 locale detection path（給 React 樹外 code path 用，followup 再處理）
- 不翻譯「中文」/「English」字面（identifier 性質，Cat D 原則）
- 不做複雜 locale detection algorithm（規則固定：override null → `navigator.language` → starts with `zh` 就中文，其他英文）
- 不動 Phase 4 layout、不動 SettingsModal 既有 dropdown 樣式
- 不引入第三種語系或 BCP-47 完整 locale 支援（只支援 `zh` / `en` 兩值 + `null`）

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-shell`: 翻轉 v1 forbidden 規則（移除 language switcher 禁制），新增 Language override requirement，更新 i18n / Settings modal 相關 scenarios

## Impact

- Affected specs: `app-shell`
- Affected code:
  - Modified:
    - codebus-app/src/components/settings/SettingsModal.tsx
    - codebus-app/src/store/settings.ts
    - codebus-app/src/lib/ipc.ts
    - codebus-app/src/hooks/useLocale.ts
    - codebus-app/src/i18n/errors.ts
    - codebus-app/src/i18n/messages.ts
    - codebus-app/src-tauri/src/ipc/config.rs
  - New:
    - codebus-app/src/components/settings/LanguageSection.tsx
    - codebus-app/src/components/settings/LanguageSection.test.tsx
    - codebus-app/scripts/.lang-switcher-smoke/ (CDP smoke screenshots, gitignored)
  - Removed: (none)
