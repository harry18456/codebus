## Context

Phase 4 把 Workspace 整體 layout 對齊 design v1 拆三個 change：
- Phase 4A · Lobby refresh（已 archive）
- Phase 4B · Sidebar rework（已 archive）
- Phase 4C · 本 change，右側內容區 Goals / Quiz tab content header row

AUDIT 在 `codebus-app/design-handoff/AUDIT.md` 把這塊散在 R2/R3/R4/R5/R6 + GP1/GP2/GP3 + QE4/QL6 共 9 個 site，跨 Goals 跟 Quiz 兩 tab、empty 跟 populated 兩種狀態。Phase 4A 留下的 `<SectionLabel variant="caps">` 跟 amber active bar 需要在這裡跟 content title h1 視覺呼應，layout 結構才完整收尾。

### Pre-apply 校準（apply 第一步 grep 後對應更新）

Apply 動手前 grep 一次校準 AUDIT，已知差異如下（per `project_phase_3a_blind_spots_cleanup_lessons` lesson 1：AUDIT「N 處」count 別信）：

- **R4（部分已 wire）**：i18n key `workspace.goals.examplePlaceholder1..3` 已存在於 `messages.ts`（zh + en 兩套），但 `GoalsTab.tsx` 內 `GOAL_EXAMPLES` 仍是英文字面常數、未走 `t()`。本 change 補完 wiring，不重建 key。`workspace.goals.newGoalButton` / `workspace.goals.emptyHint` 早已 i18n、保留重用。
- **QE1（已部分落地）**：QuizTab 現況已有 h2 顯示 `workspace.quiz.tab.heading`（"Quiz history"）。本 change 改為 `<TabContentHeader>` 渲染，h1 文字改用新 key `workspace.quiz.headerTitle`，原 `workspace.quiz.tab.heading` 保留但本 site 不再消費（其他位置仍可能引用、留著不刪）。
- **QL6 / QE4**：QuizTab 現況已有 `+ New quiz` button（`workspace.quiz.tab.newButton`），位置在 topbar row。本 change 搬進 `<TabContentHeader>` CTA slot、不改 key。
- **GP3**：`<SectionLabel variant="caps">` 已在 design-system spec 存在（`codebus-app/src/components/ui/SectionLabel.tsx`），可直接 consume。
- **Goals header row 現況**：`GoalsTab.tsx` 已有 `flex justify-end border-b border-border p-3 pr-[160px]` topbar row，只放右側 `+ New goal` button、無 h1/subtitle/chip。本 change 把它換成 `<TabContentHeader>`。

實機 apply 階段第一個 task 是再跑一次 grep 把上述四項 final-verify（site 還在嗎 / 已被別 change 順手修了嗎），任何新差異補進本段。

**Apply task 1.1 grep 校準結果（2026-05-27）**：

| 項 | 假設 | grep 命令 | 命中 | 結論 |
| --- | --- | --- | --- | --- |
| R4 examples wiring | i18n key 已建、source 還未走 `t()` | `GOAL_EXAMPLES\|examplePlaceholder` in `codebus-app/src` | `messages.ts:159-161` (en) + `586-588` (zh) 已備、`GoalsTab.tsx:18-22` `GOAL_EXAMPLES` 仍英文常數、`100` 直接 render `{ex}` 未呼叫 `t()` | 假設 ✅；task 4.2 / 4.3 wiring 補完 |
| QE1 / QL6 site | QuizTab 已有 h2 + heading key + `+ New quiz` button | `workspace\.quiz\.tab\.heading` / `data-testid="new-quiz"` in `codebus-app/src` | `QuizTab.tsx:522` consume heading、`QuizTab.tsx:527` `data-testid="new-quiz"` button、`messages.ts:363` (en) / `786` (zh) 兩 locale | 假設 ✅；task 5.2 改用新 `headerTitle` 並棄消費 `tab.heading`（key 不刪） |
| emptyHint consumers | 只在 GoalsTab 一處 | `workspace\.goals\.emptyHint` in `codebus-app/src` | `messages.ts:157,584`（兩 locale）+ `workspace.test.ts:16` + `GoalsTab.tsx:97` 一處消費 | 假設 ✅；emptyHint key 保留不動、新增 emptyHeroTitle / emptyHeroSubtitle 取代消費點（避免 hint vs hero 語意混疊） |
| SectionLabel caps | component 跟 caps variant 都存在 | `SectionLabel\|section-label` in `codebus-app/src` | `components/ui/SectionLabel.tsx` 提供 `variant: "default" \| "caps"`、`globals.css` 跟 `EmptyState.tsx`/`Lobby.tsx` 已 consume | 假設 ✅；task 4.2 直接 `<SectionLabel variant="caps">RECENT</SectionLabel>` |

無新 site、無 scope 改變。i18n test 既有 expected-keys 陣列（`workspace.test.ts:16-19`、`quiz.test.ts:36`）apply 時要把新 key 加進去（task 3.2 對應 verification）。

**Apply task 6.2 CDP smoke 抓到的 finding（2026-05-27）**：

AUDIT R6 「保留現有 `N` keyboard shortcut」實際 grep 全 repo（`useNewGoalShortcut|new.goal.*shortcut|key.*===.*['\"][nN]['\"]`）**無任何現有綁定**——`useNewVaultShortcut.ts` 是 Lobby 限定且要 Cmd/Ctrl。沒有 binding 下、`<kbd>N</kbd>` chip 是 visual lie（CDP smoke 抓到的整合層缺口、unit test 抓不到）。

**處理（per user 決議 2026-05-27）**：scope 加 N binding 進本 change，避免 chip 與行為脫鉤。
- 新增 hook `codebus-app/src/hooks/useNewGoalShortcut.ts`：bare N（不帶 modifier、不在 input/textarea/contenteditable）→ `onFire`；mount 在 `GoalsTab` 內、靠 Workspace tab re-mount 契約自動 scope 到「Goals tab active」。
- GoalsTab 把 `useNewGoalShortcut(() => modalOpen ? undefined : openModalWith())` 串進去；modal 開時不重複觸發。
- 新增 unit test `useNewGoalShortcut.test.tsx`（8 case：bare/uppercase/modifier/input/textarea/unrelated/unmount）+ GoalsTab 加 2 個 keydown shortcut scenario。
- spec scenario「Goals tab content header row」新增明確 N binding 行為 scenario（取代原本含糊的「preserve existing N shortcut」說法）。

## Goals / Non-Goals

**Goals:**

- 抽 `<TabContentHeader>` 共用 component、4 個 site（Goals empty / Goals populated / Quiz empty / Quiz populated）一律 consume 同一 component。
- Goals tab empty state 重排成三段式（header row → 中央 hero 區 → examples），R5 大留白由三段撐起自然緩解。
- Goals populated 在 goal list 上方加 `RECENT` SectionLabel caps。
- Quiz tab empty + populated 一律套 content header row、保留現有 `+ New quiz` CTA 行為。
- Goals examples 接到既有 i18n key（messages.ts 已備、wiring 未完成）。
- 真實 CDP smoke 驗 zh + en locale（避踩 `project_cdp_smoke_webview2_pitfalls` 5 雷）。

**Non-Goals:**

- 不動 Lobby（Phase 4A 範圍）。
- 不動 Sidebar / nav structure（Phase 4B 範圍）。
- 不動 goal list row 任何 row 內容（GP4-GP8 屬 Phase 5）。
- 不動 quiz wizard 任何 step view（QNEW/QC 系列屬 Phase 5）。
- 不動 quiz history row 內容（QL1-5 屬 Phase 5）。
- 不改 i18n key 名稱（只新增 / value-only 更新）。
- 不 inline 4 份 header pattern（一律走 `<TabContentHeader>`）。
- 不為 `<TabContentHeader>` 抽尚未需要的 props（trailing slot / multi-CTA 等都 YAGNI）。
- shortcut chip 文字（`N`）不進 i18n value。

## Decisions

### 抽共用 component TabContentHeader

**選擇**：抽 `codebus-app/src/components/ui/TabContentHeader.tsx`、4 個 site 都 consume。

**理由**：
- 4 個 site 全部 share 相同骨架（h1 + subtitle + 右側 CTA），inline 4 份 = 同步改 4 次的負債。
- 對齊 Phase 4A `<SectionLabel>` 既有共用 pattern 精神。
- 通過 memory `feedback_dont_speculative_abstract` 的 4-use 門檻（4 個明確 consumer，不是 single-use）。

**Props shape**：

```ts
interface TabContentHeaderProps {
  title: string
  subtitle?: string
  cta?: React.ReactNode
  shortcutChipText?: string
  testId?: string
}
```

**Layout 骨架**：

- 外層 row：`flex items-center justify-between border-b border-border p-3 pr-[160px]`、`data-tauri-drag-region`、`data-testid={testId}`。
- 左側 stack：`<h1 className="text-h-row font-medium text-fg-primary">{title}</h1>` + 可選 `<p className="text-meta text-fg-secondary">{subtitle}</p>`。
- 右側 group：CTA node + 可選 shortcut chip `<span className="shortcut-chip" aria-hidden="true">{shortcutChipText}</span>`。
- `text-h-row` token 已存在於 design-system（20px）對應 h1 size。
- `pr-[160px]` 沿用既有 WindowControls（3 × 46px）留白 convention，跟 GoalsTab/QuizTab 現況同。
- `data-tauri-drag-region` 保留拖拽行為。
- shortcut chip 樣式對齊 Phase 4B sidebar 已落地的 chip pattern（apply 時 grep 既有 class / token 對齊）。

**Alternatives**：
- Inline 4 份 → 拒絕（同步負債、違 Phase 4A pattern）。
- 抽更多 slot（trailing 區、leading icon）→ 拒絕（YAGNI、目前 4 site 都不需要）。
- 把 shortcut chip 文字進 i18n → 拒絕（識別符性質、key 是按鍵字面、translation 無意義）。

### Goals empty state 三段式 layout

**選擇**：分三區、垂直 flex column、撐起整個 right pane 高度。

**結構**：

1. `<TabContentHeader>`（R2/R6）。
2. 中央 hero `<div>`：🎯 emoji + h-empty headline + 一句副標（R3）。
3. 底部 examples 區：3 個 amber-tinted mono pill button、點任一即 prefill NewGoalModal（保留現有互動）。

- Hero hero icon 用 emoji（🎯，跟 design v1 對齊；emoji 是視覺元素、不走 i18n）。
- Hero headline 文案候選：「還沒有任務」/ en「No goals yet」。Key 新增 `workspace.goals.emptyHeroTitle`。
- Hero 副標候選：「列出你想搞懂的事，公車一站一站讀給你看。」/ en 對應。Key 新增 `workspace.goals.emptyHeroSubtitle`。
- Examples 文案改用既有 `workspace.goals.examplePlaceholder1..3` key（messages.ts 已備、本 change 補 wiring）。
- 既有 `workspace.goals.emptyHint`（"Click + New goal to ask…"）可保留為 hero 副標來源，或新增 emptyHeroSubtitle key 取代——apply 階段定一個（per Phase 4A G-copy-2 教訓，能重用既有 key 優先）。

**R3/R4/R5 一起包**：R2 加 header row 後 R3 視覺更突兀（原本中央三行被擠扁）、R5 大留白被三段撐起自然消，必須一起做。

### Goals populated RECENT SectionLabel

**選擇**：goal list `<ul>` 之前插入 `<SectionLabel variant="caps">RECENT</SectionLabel>`，無 count。

**理由**：caps variant 已存在；`RECENT` 是識別符（Cat D）不翻；count 屬 Phase 4B/5 範圍、不擴大本 change scope。

### Quiz tab 套 TabContentHeader

**選擇**：empty 跟 populated（含 `phase === "history"`）兩態都套；wizard / planning / generating / ready / review / attempt / error 等中間 phase 不套 content header（保留現有 wizard topbar 行為、屬 Phase 5 範圍）。

**理由**：
- QE4 / QL6 範圍只覆蓋 history view 的 header；wizard 是 Phase 5 重排 layout。
- `phase === "idle"` 屬 wizard step 0（topic 輸入），本 change 內保留現況、Phase 5 重做。

### i18n key 處理

**新增 key**（value-only、key 不可改名 per G-copy-2）：

| Key | zh 候選 | en 候選 |
| --- | --- | --- |
| `workspace.goals.headerTitle` | 「Goals」 | "Goals" |
| `workspace.goals.headerSubtitle` | 「列出你想搞懂的事」 | "List what you want to understand" |
| `workspace.goals.emptyHeroTitle` | 「還沒有任務」 | "No goals yet" |
| `workspace.goals.emptyHeroSubtitle` | （候選同 subtitle 或獨立句） | （同） |
| `workspace.quiz.headerTitle` | 「Quiz」 | "Quiz" |
| `workspace.quiz.headerSubtitle` | 「驗證自己有沒有看懂 wiki」 | "Test how well you understood the wiki" |

實際 wording apply 階段定（依 design v1 vocabulary、避用「vault」字面 per G-copy 系列、引用 04b Quickstart copy 文案對齊）。

**保留 key 重用**：`workspace.goals.newGoalButton` / `workspace.goals.emptyHint`（後者若 emptyHeroSubtitle 採用同一字串時可重用）/ `workspace.goals.examplePlaceholder1..3` / `workspace.quiz.tab.newButton` / `workspace.quiz.tab.heading`（本 site 不再消費，但保留 entry、其他位置可能引用）/ `workspace.quiz.tab.emptyHint`。

**Goals headline 命名注意**：headerTitle 重複 t() 後值「Goals」與 tab nav 顯示一致；分開 key 是因 nav label 屬 sidebar 範圍（Phase 4B 已落地），不能跨層共用、避免後續 nav 改名連動。

### 共用 TabContentHeader 進 design-system spec

`design-system` spec 補一條 requirement：`<TabContentHeader>` 共用 component、props shape、layout 結構、`text-h-row` token 使用。

## Implementation Contract

**觀察行為**（apply 完成、實機 GUI 可確認）：

- Goals tab 進入（empty 或 populated）→ 頂部一條 content header row：左側 h1「Goals」+ 副標、右側 `+ New goal` CTA + `N` shortcut chip。原獨立 topbar `flex justify-end` row 不再存在（被 `<TabContentHeader>` 取代）。
- Goals tab empty（沒 goal 的 vault）→ header row 下方為三段式：中央 hero 區（🎯 + headline + subtitle）+ 底部 examples（3 個 amber pills，點任一即 prefill NewGoalModal）。所有英文 hardcode（GOAL_EXAMPLES、hero copy）走 `t()`。
- Goals tab populated → header row 下方先一個 `<SectionLabel variant="caps">RECENT</SectionLabel>` 再 goal list `<ul>`；goal list row 本身內容不變。
- Quiz tab `phase === "history"` 進入 → 頂部 content header row：左側 h1「Quiz」+ 副標、右側 `+ New quiz` CTA（無 shortcut chip）。
- Quiz tab 其他 phase（idle / planning / confirm / generating / ready / review / attempt / no_match / error）→ 本 change 不動現況 layout。
- 按 N 鍵（Goals tab 焦點時）→ 觸發 new goal（既有行為保留、shortcut chip 視覺對應）。
- locale 切換 zh ↔ en → 所有新加 + 既有重用 key 都翻譯生效，無漏字面英文殘留在 Goals/Quiz tab 兩 state。

**Interface / 資料 shape**：

- `<TabContentHeader>` props 如 Decisions 段定義。
- i18n key 新增清單如 Decisions 段表格。
- 既有 IPC / store / event 完全不動。

**Failure modes**：

- 純 UI 改動、無 backend / IPC 變更；不引入新 failure mode。
- i18n key 漏建 → `useT()` fallback 顯示 key 字面（既有行為），unit test 必須覆蓋所有新 key 兩 locale 存在。

**Acceptance criteria**：

1. `pnpm tsc` 綠（含新 component types）。
2. `pnpm test` 綠：
   - 新 `TabContentHeader.test.tsx`：覆蓋 title-only / + subtitle / + cta / + shortcut chip 四組合渲染、testId 正確。
   - `GoalsTab.test.tsx` 更新：confirm 抓 `tab-content-header-goals` testid、抓 hero region、抓 examples 走 `t()` key（mock locale switch）、抓 RECENT SectionLabel for populated state。
   - `QuizTab.test.tsx` 更新：confirm history view 抓 `tab-content-header-quiz` testid + `+ New quiz` CTA、其他 phase view 不渲染 content header。
   - `workspace.test.ts` / `quiz.test.ts` 加新 key 雙 locale assertion。
3. **真實 CDP smoke**（per `project_cdp_smoke_webview2_pitfalls` 5 雷預掃）：
   - Goals tab empty（沒 goal 的 vault）：header row 存在 + 三段式 visual + zh/en 切換翻譯生效。
   - Goals tab populated（1-3 個 goal）：header row + `RECENT` SectionLabel + goal list row 內容不變。
   - Quiz tab empty / populated history：header row + `+ New quiz` CTA。
   - 按 N → 觸發 new goal 動作（既有行為保留）。
   - 截圖存 `codebus-app/scripts/.content-header-smoke/`。
4. **4B amber bar ↔ 4C h1 視覺呼應確認**：active nav 切換時、左側 amber bar 跟 right side h1 顏色 + 視覺 align。
5. **跨 tab 一致性**：Goals 跟 Quiz 兩 tab content header 同 `<TabContentHeader>` 渲染、視覺完全一致（除 subtitle / chip optional 差異）。

**Scope boundaries**：

- In scope：`<TabContentHeader>` 新增、Goals tab empty + populated 改套 header + 三段式 + RECENT label、Quiz tab history view 改套 header、新 i18n key、既有 Goals examples wiring。
- Out of scope：goal list row 內容、quiz wizard 任何 step、quiz history row 內容、Lobby、Sidebar、nav count、GP4-GP8、QNEW/QC/QL1-5 系列、AUDIT R7 ChatWidget mode、Phase 4B amber bar 細節。

## Risks / Trade-offs

- [`<TabContentHeader>` props shape 跨 tab 分歧] → Mitigation: subtitle / shortcutChipText 都做 optional；4 site 內若 shape 仍不夠 → stop 找 user 對齊（per「工時上限 1 天、超過停下」memory）。
- [R3 三段式 visual 細節需 design 對齊] → Mitigation: design.md 先 lock pseudo-structure（hero emoji + headline + subtitle + 3 examples pill），apply 時若 hero copy 或 pill 樣式需 design 進一步輸入 → 暫保留現況 pill 樣式（已通過 design v1 認可）+ 新加 hero region；wording 不確定處 fallback 用既有 emptyHint 文案。
- [既有 emptyHint key 改用 vs 新增 emptyHeroSubtitle] → Mitigation: apply 階段先 grep `emptyHint` 所有 consumer；若僅 GoalsTab 一處用 → 重用 emptyHint 改 wording（value-only）；若多處用 → 新增 emptyHeroSubtitle 不動 emptyHint。
- [`workspace.quiz.tab.heading`（"Quiz history"）變成 dead key] → Mitigation: 保留 key 不刪（避免破壞 i18n test snapshot 跟其他可能引用）；archive 階段補 doc 註記「本 site 已改用 headerTitle」即可。
- [CDP smoke 在 WebView2 上 React batching / focus timing 五雷] → Mitigation: apply 第一步先把 `project_cdp_smoke_webview2_pitfalls` 五雷列為 checklist；click + query 拆兩段 eval；focus 後 sleep ≥500ms；locale 切換靠 `settings-save` testid。
- [shortcut chip 樣式跟 sidebar 既有 chip 不一致] → Mitigation: apply 時 grep sidebar 既有 chip class / token、`<TabContentHeader>` chip 直接借用 className / 共用 token，不另新增 design-system 條目（chip 樣式屬既有 sidebar 範圍重用）。
