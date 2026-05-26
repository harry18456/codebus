# settings-language-switcher · notes

## 命名統一（inline correction）

Propose/design/spec 原本用 camelCase `localeOverride`；實作發現 codebase YAML 慣例為 snake_case
（`pass_threshold` / `read_image_block` / `active_provider`）。已在 apply 階段 inline `sed` 把全部 artifact +
程式碼統一為 `locale_override`，`spectra validate` 仍綠。

## App.tsx 啟動預載（apply 階段發現的接線缺口）

第一輪 CDP smoke 在 step 3（重啟仍英文）失敗，因 `useSettingsStore.load()` 只在 Workspace 掛載
或 SettingsModal 開啟時觸發。Lobby 階段 `useLocale` 因此 fallback 到 `navigator.language`，
即使 disk 上有 `app.locale_override: en` 也會閃中文。

**修法**：`codebus-app/src/App.tsx` 的 `AppShell` 加 `useEffect(() => void settingsLoad().catch(() => {}), [...])`，
讓 settings 在 app mount 時就 preload。新增 `codebus-app/src/App.test.tsx`（2 條 test）鎖住
此行為：(a) mount 後 `invoke("load_global_config")` 被呼叫 + store 拿到 `locale_override`；
(b) load 失敗時 App 不 throw。

## 測試（task 5.1）

- `pnpm tsc` → exit 0
- `pnpm vitest run src/i18n/settings.test.ts` → 90/90 passed（含本 change 新增 11 條）
- `pnpm vitest run src/lib/ipc.localeOverride.test.ts` → 7/7 passed
- `pnpm vitest run src/hooks/useLocale.test.tsx` → 9/9 passed（涵蓋 precedence Example 表 6 列 + 邊界）
- `pnpm vitest run src/store/settings.localeOverride.test.ts` → 4/4 passed
- `pnpm vitest run src/i18n/errors.test.tsx` → 2/2 passed
- `pnpm vitest run src/components/settings/LanguageSection.test.tsx` → 8/8 passed
- `pnpm vitest run src/components/settings/SettingsModal.test.tsx` → 22/22 passed（含改寫的「Language dropdown is present」反面斷言）
- `pnpm vitest run src/test/forbidden-behaviors.test.tsx` → 5/5 passed
- `pnpm vitest run src/App.test.tsx` → 2/2 passed
- 全 frontend `pnpm vitest run`（隔離跑）→ 770/770 passed；並行跑偶有 2 條既有非本 change 測試 flake（`SettingsModal · calls save_global_config on Save and closes after success` + `QuizReview · 看過程`），單獨跑都綠，視為 pre-existing parallelism timing
- `cd src-tauri && cargo test --lib` → 151/151 passed（含新增 4 條 `app.locale_override` round-trip / null write / legacy load / invalid string rejection）

## CDP smoke（task 5.2）

`scripts/.lang-switcher-smoke/`：

| step | 截圖 | 觀察 |
| --- | --- | --- |
| 1 · default zh | `step-1-default-zh.png` | Lobby 中文（`+ 新增 Vault` / `近期 VAULT` / `設定` / `提示 · 把 repo...`），navigator.language=`zh-TW`、disk 無 `locale_override` |
| 2 · switch English (reactive) | `step-2-switch-en.png` | 切 English 後不重載，Modal `Global Settings`、`AI Provider`、`Language`、`Reads/writes ~/.codebus/config.yaml`、`Save`/`Cancel`、底部 `Settings` 全部即時翻；Language trigger 顯示 `English` |
| 3 · restart English (persist) | `step-3-restart-en.png` | `page.reload()` 後 Lobby 仍英文（`+ New Vault`、`RECENT VAULTS`、`last opened`、`1d ago`、`tip · Drag a repo folder...`、`Settings`） |
| 4 · back to Auto | `step-4-back-to-auto.png` | 切 Auto Save 後 reload，Lobby 回 zh（navigator zh-TW → `"zh"`） |
| 5 · backend error in English | `step-5-backend-error-en.png` | 再切 English Save，透過 IPC 觸發 `vault_not_found` → Toast `Path no longer exists: Z:/no/such/path/codebus-smoke`（LocalizedError chain 無改 code 直接跟著 store locale） |

## Config.yaml round-trip（task 5.3）

```yaml
# baseline (pre-smoke)
app:
  quiz:
    pass_threshold: 69
```

```yaml
# after step 2 Save English
app:
  locale_override: en
  quiz:
    pass_threshold: 69
```

```yaml
# after step 4 Save Auto
app:
  locale_override: null
  quiz:
    pass_threshold: 69
```

```yaml
# after step 5 Save English (final state)
app:
  locale_override: en
  quiz:
    pass_threshold: 69
```

`app.quiz.pass_threshold: 69`（user 既有設定）四次 round-trip 都未被破壞；`agent.*` / `quiz.*` / `goal.*`
namespace 亦未受影響（檔案其餘段落原樣保留）。Apply 結束時 user disk 留在 `locale_override: en`
（最後 smoke 的 saved state）；如要回 Auto 直接 Settings → Auto → Save 即可。
