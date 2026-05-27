## Why

Phase 4A Lobby refresh 跟 4B sidebar rework 把 Workspace 左側全收完之後，右側內容區 Goals tab 跟 Quiz tab 仍未對齊 design v1 的「content header row」pattern（`h1 + subtitle + CTA + shortcut chip`）。AUDIT 已盤點 R2/R3/R4/R5/R6 + GP1/GP2/GP3 + QE4/QL6 共 9 個 site，散在 empty / populated 兩種狀態，全部需要統一視覺。Phase 4C 是 Phase 4 layout structure 收尾的最後一塊，做完 4A 的 amber bar 才能跟 4C 的 h1 視覺呼應（active nav indicator ↔ content title）。

## What Changes

- 新增共用 component `<TabContentHeader>`（`codebus-app/src/components/ui/TabContentHeader.tsx`），統一承載 h1 + subtitle + 右側 CTA + 可選 shortcut chip，所有 tab content header 都 consume 同一 component。
- **Goals tab empty state**（R2/R3/R4/R5/R6）：加 content header row（h1「Goals」+ subtitle + `+ New goal` CTA + `N` shortcut chip）；empty visual 改三段式 layout（header row → 中央 hero 區 → 底部 examples）；examples 文案接到既有 i18n key（`workspace.goals.examplePlaceholder1..3`，messages.ts 已備但 `GoalsTab.tsx` 未 wire）。
- **Goals tab populated state**（GP1/GP2/GP3）：套同一 `<TabContentHeader>`；goal list 上方加 `RECENT` `<SectionLabel variant="caps">`（Cat D 識別符不翻）；不動 goal row 內容。
- **Quiz tab**（QE4/QL6）：empty + populated 都套 `<TabContentHeader>`（h1「Quiz」+ subtitle + `+ New quiz` CTA，無 shortcut chip）；不動 quiz wizard / history row 內容。
- **i18n 新 key**（value-only、key 命名沿 `workspace.<scope>.*` snake_case 慣例）：
  - `workspace.goals.headerTitle`、`workspace.goals.headerSubtitle`
  - `workspace.quiz.headerTitle`、`workspace.quiz.headerSubtitle`
  - 既有 key（`workspace.goals.newGoalButton` / `workspace.quiz.tab.newButton` / 三個 examplePlaceholder）一律重用、不改名（per Phase 4A G-copy-2 教訓）。
- **shortcut chip 文字**（`N` / 無）走 component 層 inline 識別符，不進 i18n value。
- **Pre-apply 校準** 寫進 design.md「Pre-apply 校準」段：R4 i18n key 已建但 `GoalsTab.tsx` 未 wire、QuizTab 現況已有 h2 + `+ New quiz` 但缺 subtitle / chip / shared component、SectionLabel caps variant 已存在可直接用。

## Non-Goals

- 不動 Lobby（Phase 4A 範圍、已收）。
- 不動 Sidebar / nav structure（Phase 4B 範圍、已收）。
- 不動 goal list row 內容（GP4/GP5/GP6/GP7/GP8 屬 Phase 5）。
- 不動 quiz wizard 任何步驟（QNEW/QC 系列屬 Phase 5）。
- 不動 quiz history row 內容（QL1-5 屬 Phase 5）。
- 不改既有 i18n key 名稱（只新增 / value-only 更新）。
- 不做 v1.1 spec 才落地的內容（quiz wizard fullscreen view / Wiki page reader 等屬 Phase 5/6）。
- 不 inline 同一 header pattern 四次——一律抽 `<TabContentHeader>` 共用 component。
- shortcut chip 文字（`N`）不進 i18n value（識別符性質）。
- 不為 `<TabContentHeader>` 抽尚未需要的 props（例如 trailing slot / multi-CTA）——只支援目前 4 個 site 實際需要的 shape。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-workspace`: Goals tab 跟 Quiz tab 的「empty 預設視覺」+「populated header 結構」對齊 design v1 content-header-row pattern；新增 RECENT section label requirement；現有 `+ New goal` / `+ New quiz` CTA 位置從獨立 topbar row 移入 content header row。
- `design-system`: 新增 `<TabContentHeader>` 共用 component requirement（屬 Layout / Composition 類），記 h1 + subtitle + CTA + optional shortcut chip 的 props shape 與排版規格。

## Impact

- Affected specs:
  - openspec/specs/app-workspace/spec.md（modified — Goals tab + Quiz tab header 結構與 RECENT section label）
  - openspec/specs/design-system/spec.md（modified — 新增 TabContentHeader 共用 component requirement）
- Affected code:
  - New:
    - codebus-app/src/components/ui/TabContentHeader.tsx
    - codebus-app/src/components/ui/TabContentHeader.test.tsx
    - codebus-app/src/hooks/useNewGoalShortcut.ts（apply 階段加：CDP smoke 抓到 N chip 對應的 keyboard binding 不存在）
    - codebus-app/src/hooks/useNewGoalShortcut.test.tsx
  - Modified:
    - codebus-app/src/components/workspace/GoalsTab.tsx
    - codebus-app/src/components/workspace/GoalsTab.test.tsx
    - codebus-app/src/components/workspace/QuizTab.tsx
    - codebus-app/src/components/workspace/QuizTab.test.tsx
    - codebus-app/src/i18n/messages.ts
    - codebus-app/src/i18n/workspace.test.ts
    - codebus-app/src/i18n/quiz.test.ts
    - codebus-app/design-handoff/AUDIT.md（archive 階段標 archived 2026-05-27 in R2/R3/R4/R5/R6 + GP1/GP2/GP3 + QE4/QL6）
  - Removed: (none)
