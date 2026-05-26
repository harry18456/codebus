## Why

Design AUDIT 結算階段 Phase 2 鎖定兩件視覺基礎工作：字號 scale 全 token bump +1 級、border `#1f1f1f → #2a2a2a` promote、以及把 14 個位置會用到的中文 section label 抽成共用 `<SectionLabel>` component。這層基礎不先落地，Phase 4 的 layout 重排 / Phase 3 的 status token / Phase 6 的 v1.1 mock view 都沒有可信的視覺尺規可對齊；harry 在 1920×1080 + Windows 100% 縮放下實機反饋「整體文字偏小、有點吃力」、設計稿 hairline `#1f1f1f` 邊框在 ClearType 渲染下完全看不到，是當下最痛的視覺缺陷。

Phase 1 已 merged 進 main（commit `6bdaf51`），Phase 2 在 AUDIT 原本拆成 `design-token-typography-and-border` + `section-label-component` 兩個 change，這次合一個 change 一起做——token bump 跟 component 在使用上強耦合（component 的字號就是新的 meta 12），分開做 component 端會先綁舊字號值再回頭調，等於 churn。

## What Changes

**1. Tailwind v4 `@theme` token 擴充（`codebus-app/src/styles/tokens.css`）**

新增 font-size token，全 +1 級對齊 AUDIT「Cross-cutting · 字號 scale」決議：

- `--text-body: 14px`（原 hard-code 13）
- `--text-body-lg: 15px`（原 14）
- `--text-meta: 12px`（原 11）
- `--text-micro: 11px`（原 10）
- `--text-h-row: 20px`（原 18）
- `--text-h-detail: 22px`（原 20）
- `--text-h-quiz: 24px`（原 22）
- `--text-h-empty: 28px`（原 24）

**2. Border token promote（`codebus-app/src/styles/tokens.css`）**

- `--color-border` 從 `#1f1f1f` 改為 `#2a2a2a`（即原 `--color-border-strong` 值）
- 新增 `--color-border-hairline: #1f1f1f` 給「明顯只用在 card 內 row separator」的場合
- `--color-border-strong` 保留現值不動，但 sweep 後若無 consumer 可移除（留到後續 change）

**3. 新增 `<SectionLabel>` component**

- 位置：`codebus-app/src/components/ui/SectionLabel.tsx`（新檔）
- API：`<SectionLabel variant="default" | "caps" count={n?} className={...}>{children}</SectionLabel>`
- `variant="default"`：amber 2px 左 bar + 12px / 500 / `text-fg-secondary`，無 uppercase、無 tracking（對齊 walkthrough §01.1 `.section-label` spec）
- `variant="caps"`：保留 amber bar，但 text 走 uppercase + `letter-spacing: 0.08em` + 11px / `text-fg-tertiary`（對齊 `.section-label--caps`，給 Wiki tree 5-bucket 英文 taxonomy 用）
- `count` prop：optional，渲染為右側 mono 11px `text-fg-tertiary`，`margin-left: auto`
- 不在這次套到 14 個位置（屬 Phase 4 layout 重排範圍），這次只負責「component 寫好、走過 unit test、新元件位置與 API 鎖死」

**4. 全 `src/` sweep `text-[Npx]` hard-code**

- 用 grep 找出 `codebus-app/src/` 下所有 `text-[10px]` / `text-[11px]` / `text-[12px]` / `text-[13px]` / `text-[14px]` / `text-[18px]` / `text-[20px]` / `text-[22px]` / `text-[24px]` hard-code（現況約 152 處）
- 對應新 token：直接換成 Tailwind v4 自動產生的 utility（`text-body` / `text-meta` / 等）或保留 inline 但用新數字
- font-size 為 56/64/72 等大型 emoji glyph 視 case-by-case，**不必動**
- 結束後再 grep 一次，確保 `text-[Npx]` 只剩極少數合理 case（如測試 fixture、emoji glyph）

**5. vitest snapshot 重生**

- `<SectionLabel>` 新增 unit test：default / caps / count / className override 各一支
- 既有 snapshot test 因為字號 / border 變動會 fail，整批重生
- typecheck (`npm run typecheck`) 必須全綠

## Non-Goals

- **不做 layout 重排**：CTA 進 content header、Sidebar 重整、Vault 詞 UI 拿掉等屬 AUDIT Phase 4，這次不動
- **不做 status three-state token (`--success` / `--warn` / `--error` 鎖 + `<StatusPill>`)**：屬 Phase 3 並行範圍，另起 change
- **不做 i18n sweep**：Phase 3 另起
- **不把 14 個位置實際套用 `<SectionLabel>`**：本 change 只負責 component 落地；套用是 Phase 4 各 change 各自處理（lobby empty / workspace sidebar / Goals overview / Settings 子段 ...）
- **不動 design-handoff/ 內容**：那是 source of truth，AUDIT 文末已 lock；本 change 是消費端
- **不引入 tailwind.config.ts**：codebus-app 走 Tailwind v4 CSS-first（`@theme`），新 token 一律走 `tokens.css`
- **不分兩個 PR / branch**：合一個 change、直接 main（solo dev 慣性，spectra apply 跟既有 workflow 對齊）

## Capabilities

### New Capabilities

- `design-system`: codebus-app 的視覺基礎建設層，包括 design token（顏色、字號、spacing、border）、共用 visual primitives（如 SectionLabel 這類非互動性視覺元件），與 sweep 規範（hard-code 何時可接受、何時必須走 token）。後續 status token (`StatusPill`)、其他共用視覺元件也歸入此 capability。

### Modified Capabilities

(none)

## Impact

- Affected specs:
  - New: `openspec/specs/design-system/spec.md`
- Affected code:
  - New:
    - `codebus-app/src/components/ui/SectionLabel.tsx`
    - `codebus-app/src/components/ui/SectionLabel.test.tsx`
  - Modified:
    - `codebus-app/src/styles/tokens.css` — 加 font-size token、改 `--color-border`、加 `--color-border-hairline`
    - `codebus-app/src/components/BottomStrip.tsx`
    - `codebus-app/src/components/DropTargetOverlay.tsx`
    - `codebus-app/src/components/LoadingOverlay.tsx`
    - `codebus-app/src/components/Toast.tsx`
    - `codebus-app/src/components/lobby/EmptyState.tsx`
    - `codebus-app/src/components/lobby/Lobby.tsx`
    - `codebus-app/src/components/lobby/NewVaultFlow.tsx`
    - `codebus-app/src/components/lobby/VaultCard.tsx`
    - `codebus-app/src/components/settings/CodexEndpointSection.tsx`
    - `codebus-app/src/components/settings/EndpointSection.tsx`
    - `codebus-app/src/components/settings/SettingsModal.tsx`
    - `codebus-app/src/components/workspace/ActivityStreamItem.tsx`
    - `codebus-app/src/components/workspace/ChatNewChatButton.tsx`
    - `codebus-app/src/components/workspace/ChatTokenDisplay.tsx`
    - `codebus-app/src/components/workspace/ChatTranscript.tsx`
    - `codebus-app/src/components/workspace/ChatUndoToast.tsx`
    - `codebus-app/src/components/workspace/GoalsTab.tsx`
    - `codebus-app/src/components/workspace/NewGoalModal.tsx`
    - `codebus-app/src/components/workspace/QuizAnswering.tsx`
    - `codebus-app/src/components/workspace/QuizGenerationLog.tsx`
    - `codebus-app/src/components/workspace/QuizReview.tsx`
    - `codebus-app/src/components/workspace/QuizTab.tsx`
    - `codebus-app/src/components/workspace/RunDetailCancelled.tsx`
    - `codebus-app/src/components/workspace/RunDetailDone.tsx`
    - `codebus-app/src/components/workspace/RunDetailRunning.tsx`
    - `codebus-app/src/components/workspace/RunListItem.tsx`
    - `codebus-app/src/components/workspace/WatcherStatusBanner.tsx`
    - `codebus-app/src/components/workspace/WikiPreview.tsx`
    - `codebus-app/src/components/workspace/WikiTab.tsx`
    - `codebus-app/src/components/workspace/WikiTree.tsx`
    - 既有 BottomStrip.test.tsx + 其他 vitest snapshot 重生
  - Removed: (none)
