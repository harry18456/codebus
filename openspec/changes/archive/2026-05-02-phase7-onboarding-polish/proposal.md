## Why

承接 D-033 Change B（`provider-settings-and-onboarding` archive 2026-05-01），進入 Phase 7（README.md §九 第五階段「跨平台測試 + 打磨 demo」）。第一輪實機 cargo tauri dev 掃過 onboarding wizard 後立刻暴露兩條問題：

1. **welcome 頁文案綁定 OpenAI ToS** — 與 D-033 整體 multi-provider 架構（TrackedProvider allowlist 已預留 multi-impl 槽位）相矛盾，未來加 Anthropic / Azure / local provider 會變成尷尬的歷史包袱。
2. **onboarding wizard 與 settings page 全英文** — 與專案 zh-TW 約定（CLAUDE.md §溝通語言）不一致，D-033 B 著陸前沒覆蓋到 i18n 一輪。

本 change 走 fluid 模式：propose 階段只鎖當前已知 scope（i18n + ToS 解綁）；apply 階段邊跑 D-033 B archive 留下的 task 12.4（manual e2e 三條路徑：冷啟動 onboarding / chat hot-swap / embed hot-swap），新發現的 UX / 功能面 / 文案問題透過 `/spectra-ingest` 滾入 tasks，不另開 change 切碎。

## What Changes

**Initial scope（propose 鎖定）：**

- **Onboarding wizard 文案 zh-TW i18n**：welcome / providers / done 三頁所有可見文字（標題、段落、按鈕、placeholder、error message）翻譯為 zh-TW。
- **Settings page 文案 zh-TW i18n**：settings 頁 header + 三大 section（ProviderPoolList、RoleBindingTable、PiiModeToggle）+ 兩 modal（ProviderEditModal、EmbeddingChangeConfirmModal）所有可見文字翻譯為 zh-TW。
- **welcome 頁 ToS 解綁 OpenAI**：spec narrative 拿掉「link out to the OpenAI Terms of Service」描述；ToS 連結職責下移到 providers 頁的 chat / embed form contextual 顯示（依 provider type 切換 ToS URL）。
- 對應 spec 同步更新：provider-onboarding capability MODIFY welcome / providers 兩段 narrative 與 SHALL clause（ToS contextual 規則）。

**Open scope（apply 階段透過 `/spectra-ingest` 滾入）：**

- task 12.4 三條路徑跑出來的 UX 不順、bug、功能缺漏（不限定型態）。
- frontend-shell（TopBar）/ 其他 page 的 leftover 英文，若 e2e 走訪時使用者指出再加 task。

## Non-Goals

- **不重設計 D-033 B 架構**：Provider pool / RoleBinding / Hot-swap / RegistryHolder / keyring 三 IPC 全不動。本 change 只動 prose 文案與 narrative spec 描述。
- **不擴展 provider type**：不在此 change 加 Anthropic / Azure / local provider 實作；僅把 ToS 結構抽象化以容納未來擴充。
- **不動 task 12.5 範圍**：cross-platform PoC（macOS / Linux 實機 keyring）保持 archive 後 manual TODO 狀態。
- **不主動 sweep onboarding / settings 之外的英文**：station page / explorer console / audit drawer 的 leftover 英文若沒被使用者於 e2e 期間指出，本 change 不主動處理。
- **不開 P1+ feature**：Q&A cross-session memory / KB ops UI / sanitizer-audit-unlock 等仍 defer 至 Phase 2 / 後續 change。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `provider-onboarding`: welcome narrative 拿掉 OpenAI ToS 描述；ToS 移到 providers 頁 per-provider-type contextual 顯示（SHALL clause 行為變更）。

> 註：settings 相關前端檔案雖在下方 Impact 程式碼清單內，但純 i18n prose 文案調整，不觸及 spec 的 SHALL/MUST normative language（spec 一律英文，與終端使用者面向的 zh-TW 文案脫鉤），故不列為 modified capability、無需 delta spec。

## Impact

- Affected specs:
  - `openspec/specs/provider-onboarding/spec.md`（MODIFY welcome narrative + ADDED Scenarios 描述 contextual ToS）
- Affected code:
  - Modified:
    - `web/app/pages/onboarding/welcome.vue`
    - `web/app/pages/onboarding/providers.vue`
    - `web/app/pages/onboarding/done.vue`
    - `web/app/pages/settings.vue`
    - `web/app/components/settings/ProviderPoolList.vue`
    - `web/app/components/settings/RoleBindingTable.vue`
    - `web/app/components/settings/PiiModeToggle.vue`
    - `web/app/components/settings/ProviderEditModal.vue`
    - `web/app/components/settings/EmbeddingChangeConfirmModal.vue`
  - New: (none)
  - Removed: (none)
