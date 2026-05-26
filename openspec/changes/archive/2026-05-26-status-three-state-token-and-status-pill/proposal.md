## Why

Phase 3A i18n sweep 已 land。AUDIT GP5 / X4 / QL3 + design v1 reply C5 確定 codebus 要 canonical 三態（done / interrupted / failed），並以 amber + motion 處理 running（don't try a fourth hue）。現況 status 視覺散落各處 hard-code（amber/green/red inline class、`RunDetailCancelled` 自己畫 banner、`GoalsTab` row 用 ✓/⚠ icon、`QuizReview` fail 用 mono 灰色字），跨 surface 不一致也沒 token 撐單一改點。Phase 3B 把 token 跟 `StatusPill` 元件**綁同一個 change** land，避免 Phase 2 已踩過的「`@theme` 加 token 但 utility 沒 consumer → Tailwind v4 不 emit、:root 連 CSS var 都沒 inject」gotcha。

## What Changes

- 在 `codebus-app/src/styles/tokens.css` `@theme` block 加 4 個 semantic alias：`--color-status-done` / `--color-status-interrupted` / `--color-status-failed` / `--color-status-running`，分別 alias 現有 `--color-success` / `--color-warn` / `--color-error` / `--color-warn`（running 與 interrupted 同 hue，差異走 motion + caret，不另找第 4 hue）
- 新增 `codebus-app/src/components/ui/StatusPill.tsx` 元件，支援 4 status × 2 variant（`dot` 7px 圓 / `pill` dot + label）。`running` variant 自動帶 pulse ring（box-shadow + `@keyframes pulse` 1.4s loop）+ optional caret slot；`dot` variant 不接受 `running` status
- 新增 pulse 動畫並 gate 於 `@media (prefers-reduced-motion: reduce)`：reduce 模式下 fallback 靜態（無 box-shadow ring 動畫）
- 在 `codebus-app/src/i18n/messages.ts` 加 4 條 status label key（`workspace.status.done` / `interrupted` / `failed` / `running`），zh + en bundle
- 套 `<StatusPill>` 到既有 consumer：
  - `GoalsTab.tsx` row dot（替 `✓`/`⚠` icon、使用 `dot` variant）
  - `RunDetailRunning.tsx` header pill（`running` + caret slot 給 stream-tail mono narration）
  - `RunDetailDone.tsx` header pill（`done`）
  - `RunDetailCancelled.tsx` header pill（`interrupted`，跟 W3/I4/X3 header right action 邏輯共構但**不負責搬位置或改 banner**——那是 02c full layout 另一 change 的工作）
  - `QuizReview.tsx` / `QuizAnswering.tsx` fail tag（替 mono 灰色「fail」、使用 `pill` variant）
- 不動 QF1 quiz completion hero（hero icon 自成 spec、`StatusPill` 不 apply hero）

## Non-Goals (optional)

- 不做 02c Interrupted 的 banner 三變體、`interrupt_reason` backend 欄位、`Retry` 行為——交 `interrupted-state-formalize` change
- 不重新命名 `RunDetailCancelled.tsx` → `RunDetailInterrupted.tsx`——同上交 02c change
- 不改 02a/02b/02c header layout（W3 / I4 / X3 header right action 統一位置）——交 02 view full change
- 不改 GP8 「running row 自動 expand 加 stream tail caret」 行為——本 change 只提供 caret slot，行為交 GP8 後續 change
- 不改 QF1 quiz completion hero icon（hero 自成視覺、status token 只連動色值不換 component）
- 不做 `--success` / `--warn` / `--error` 重命名或刪除——保留現有 alias、僅新增 status semantic layer
- 不做 i18n Cat A/B/C 殘留 wiring（QF2 / QR4 等）——交 `i18n-sweep-phase-3a-followup`

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `design-system`: 新增 4 個 status semantic token、新增 `StatusPill` 元件 API、新增 pulse 動畫 reduce-motion 規範、token+consumer 同 change land 規範

## Impact

- Affected specs:
  - Modified: `openspec/specs/design-system/spec.md`（加 status token requirement + StatusPill 元件 requirement + pulse 動畫 reduce-motion requirement）
- Affected code:
  - New:
    - codebus-app/src/components/ui/StatusPill.tsx
    - codebus-app/src/components/ui/StatusPill.test.tsx
  - Modified:
    - codebus-app/src/styles/tokens.css（加 4 status alias）
    - codebus-app/src/styles/globals.css（加 `@keyframes pulse` + reduce-motion gate）
    - codebus-app/src/i18n/messages.ts（加 4 status label key zh + en）
    - codebus-app/src/components/workspace/GoalsTab.tsx
    - codebus-app/src/components/workspace/RunDetailRunning.tsx
    - codebus-app/src/components/workspace/RunDetailDone.tsx
    - codebus-app/src/components/workspace/RunDetailCancelled.tsx
    - codebus-app/src/components/workspace/QuizReview.tsx
    - codebus-app/src/components/workspace/QuizAnswering.tsx
  - Removed: (none)
