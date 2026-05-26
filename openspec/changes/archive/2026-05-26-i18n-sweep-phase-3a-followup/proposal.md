## Why

Phase 3A (`i18n-sweep-cat-a-b-c-d`) 在第 32 處 lock boundary 後完成 archive，CDP en-locale smoke 期間又抓到 8 處 residual hard-code（位於 Phase 3A grep sweep 4 個 pattern 的盲區），且 `ActivityStreamItem.tsx` 的 `bannerLabel` 函式存在 10 case zh-only hard-code（en locale 會中英混雜）。同時 Phase 3A retrospective 已記錄兩個 sweep 盲區（template literal、`.ts` 檔），需要寫進 spec 否則下次 sweep 仍會漏。本 followup 把這三件事一起收乾淨。

## What Changes

- Wire 8 處 residual hard-code 進 i18n bundle（4 處用既有 key、4 處新增 key，含 `.ts` 工具檔）
- 重寫 `ActivityStreamItem.tsx` `bannerLabel` 函式：10 個 case 全部走 i18n key，emoji 留在 i18n value 內（不拆 emoji + text 兩個 key）
- en bundle 補齊 10 條 bannerLabel + 4 條新增 unit / verdict key 的英文翻譯
- 把 Pattern 5（template literal interpolation with Latin context）與 Pattern 6（`.ts` files outside `components/`）的 grep procedure 寫進 `app-shell` spec 的 i18n Bundle Coverage Policy
- 真實 en locale smoke（`LANG=en` + `pnpm tauri dev` + CDP 截圖）覆蓋 Lobby / Workspace Goals / Quiz / Chat / RunDetail running banner，確認無中英混雜、placeholder 全替換、wording 不彆扭
- 把 `codebus-app/design-handoff/AUDIT.md` 第 177-234 行 Followup 段落標 `archived` 並引向本 change

## Non-Goals

- 不調整 Cat D 6 條 identifier 的 jargon 政策（tab labels / verb names / codex effort / PII enum / YAML keys / Claude tool names）
- 不動 `NewVaultFlow.tsx` 第 106 行 `<span>delete</span>`（user re-init 字面 keyword，翻譯會破壞行為）
- 不拆 emoji 與 text 為兩個 i18n key（emoji 是 label 語意一部份，整段一起翻）
- 不擴張到 Pattern 7+ 結構性問題（若 grep 跑出超出本 scope 的新類別，stop 找 user 對齊，不夾帶）
- 不夾帶到 Phase 4 已開的 spectra change（本 change 獨立）

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-shell`: i18n Bundle Coverage Policy 的「Known blind spots in the 4-pattern sweep」段落升級為 Pattern 5 / Pattern 6 grep procedure，並將原本 blind spots 段從「下次 follow-up 時 SHALL 擴充」改為 spec 正式條目（同時保留現有 4-pattern Scenario）

## Impact

- Affected specs: `app-shell`（modify：i18n Bundle Coverage Policy 增 Pattern 5/6 grep procedure 與配套 Scenario）
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/GoalsTab.tsx
    - codebus-app/src/components/workspace/RunListItem.tsx
    - codebus-app/src/components/workspace/RunDetailDone.tsx
    - codebus-app/src/components/workspace/ChatNewChatButton.tsx
    - codebus-app/src/components/workspace/ActivityStreamItem.tsx
    - codebus-app/src/lib/quiz-parse.ts
    - codebus-app/src/i18n/messages.ts
    - codebus-app/design-handoff/AUDIT.md
  - New: (none)
  - Removed: (none)
