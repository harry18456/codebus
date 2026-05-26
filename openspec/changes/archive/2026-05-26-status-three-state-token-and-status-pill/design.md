## Context

design audit AUDIT.md GP5 / X4 / QL3 / QF1 + design v1 reply C5 確定 codebus 採 canonical 三態 status（done / interrupted / failed），running 用同 amber + motion 區分（don't try a fourth hue）。設計 tokens（`--color-success` / `--color-warn` / `--color-error`）在 Phase 2 `design-foundation-tokens-and-section-label` change 已 register 進 `codebus-app/src/styles/tokens.css` `@theme` block。

現況 status 視覺散落各處 hard-code：

- `GoalsTab` row 用 `✓` 白 check 跟 `⚠` amber，沒 failed 樣式落地
- `RunDetailRunning` / `RunDetailDone` / `RunDetailCancelled` 各自畫 header status，pill 樣式不一致
- `QuizReview` / `QuizAnswering` fail 用 mono 灰色「fail」字、沒色彩語意
- v1.1-mocks.html §02 已 lock `.status-pill` CSS（`font-size: 12 / padding 3px 9px / border-radius 3px / 1px border + tint bg`）跟 7px dot 樣式

Phase 2 同 change 留下教訓（記憶 `project_tailwind_v4_theme_token_emission.md`）：Tailwind v4 `@theme` block 加 `--color-X` 但無 source 用對應 utility 時，Tailwind **不 emit utility、也不 inject `:root` CSS var**——所以 token 跟 consumer 必須同 change land。

## Goals / Non-Goals

**Goals:**

- 把 status 三態（done / interrupted / failed）+ running 收進單一 token 系統，跨 surface 一致
- 提供 `<StatusPill>` 元件作為 status 視覺的單一 entry point，sweep 既有 hard-code consumer
- token + 元件 + sweep 同 change land，避免 Tailwind v4 emit gotcha
- pulse 動畫遵守 `prefers-reduced-motion: reduce` 規範

**Non-Goals:**

- 不解 02c Interrupted banner / `interrupt_reason` / Retry 行為（交 `interrupted-state-formalize`）
- 不解 02 header layout 統一（W3 / X3，交 02 view full change）
- 不解 GP8 running row auto-expand stream tail（本 change 只提供 caret slot）
- 不解 QF1 quiz completion hero（hero 自成視覺、StatusPill 不 apply）
- 不解 i18n Cat A/B/C 殘留（交 `i18n-sweep-phase-3a-followup`）

## Decisions

### Token naming uses color-status prefix to ride Tailwind v4 utility generation

**選**：`--color-status-done` / `--color-status-interrupted` / `--color-status-failed` / `--color-status-running` 放 `@theme` block，分別 alias `--color-success` / `--color-warn` / `--color-error` / `--color-warn`。

**理由**：Tailwind v4 看到 `--color-NAME` 會自動 generate `text-NAME`、`bg-NAME`、`border-NAME` utility。`<StatusPill>` 用 utility class 當 consumer——只要該 component 在 sweep 後被 import 到頁面上，Tailwind 就會看到 `text-status-done` 等出現於 source、emit utility、`:root` 也會 inject CSS var。

**Alternatives**：

- `--status-done`（無 `color-` prefix）：Tailwind 不會 generate utility、CSS var 也可能不 inject；StatusPill 要直接 `var(--status-done)` 寫進 inline style 或自寫 CSS class、繞過 Tailwind utility 系統，違反 design-system spec「token 是唯一改點」精神
- 不加 alias、StatusPill 直接讀 `--color-success` 等：semantic 層消失，未來改 status 配色得改 component 而非 token

### Running 不找第 4 hue · 走 same amber + pulse + caret

**選**：`--color-status-running` 直接 alias `--color-warn`（同 interrupted），StatusPill `running` variant 透過 motion（`@keyframes` box-shadow ring 4px→6px 1.4s loop）+ optional caret slot 區分。

**理由**：design v1 reply C5 直接明說「Don't try to find a fourth hue」。motion 跟 affordance 是 axis、不是 color；新增第 4 hue 會稀釋三態語意、increase visual noise。

**Alternatives**：

- `info` `--color-info: #60a5fa` 已存在；可挪作 running：被排除，blue 不在 status 語意層、目前留作 future neutral info banner

### Token plus consumer same-change land · 元件 sweep 算 consumer

**選**：本 change 把 token 新增、`<StatusPill>` 新元件、6 個 sweep site（GoalsTab / RunDetailRunning / RunDetailDone / RunDetailCancelled / QuizReview / QuizAnswering）綁同一 PR。

**理由**：Phase 2 教訓——只加 token 不加 consumer 會 silent fail（CDP probe `getPropertyValue` 回空字串，token 等同沒加）。把 sweep 算進 consumer 行列確保 utility class 被 Tailwind 看到。

**驗證**：實作完跑 CDP smoke：

1. `document.documentElement.style.getPropertyValue('--color-status-done')` 不為空字串
2. Goals list 三態 row dot 顏色對
3. Goal Detail header pill 三 state 顏色對
4. Quiz history fail tag 紅色對

### Dot variant 不接受 running status

**選**：`StatusPill` props 型別內 `dot` variant 排除 `running`（透過 discriminated union 或 runtime invariant）。

**理由**：Goals list row 是「過去結果」展示（done / interrupted / failed），沒有 running 行（running goal 直接導向 Goal Detail view、不留在 list 視覺層級）。Goal Detail header 才是 running 唯一展示位置，用 `pill` variant + pulse ring。

**Alternatives**：

- 允許 `dot` + `running`：得在 7px 圓上 layer pulse ring（4px ring 直接超出 row 高度、視覺破版）
- 不限制、靠約定：reviewer / 未來 contributor 容易誤用

### Pulse 動畫 gate 於 prefers-reduced-motion reduce

**選**：`@keyframes status-pulse` 定義跟 `.status-pill__dot--running-animated { animation: ... }` 全部包進 `@media not (prefers-reduced-motion: reduce) { ... }`；reduce 模式 fallback 靜態 box-shadow ring（不動）。

**理由**：a11y 必要，且 codebus running ambient 訊號靠「動」傳遞、reduce 模式下退化成靜態 ring 仍可區分 running vs interrupted（visual cue 退化但不消失）。

### i18n key 走 workspace.status namespace

**選**：`workspace.status.done` / `interrupted` / `failed` / `running` 進 `i18n/messages.ts`，zh + en bundle。

**理由**：跟現有 `workspace.*` namespace 對齊；status 是跨 view（Goals / Goal Detail / Quiz）語意、不歸特定 sub-view。

## Implementation Contract

**Behavior**

- 開 Goals 頁，row dot 三態色（done = green `--color-success` / interrupted = amber `--color-warn` / failed = red `--color-error`）；舊 `✓` white check 跟 `⚠` emoji 不再出現
- 開 Goal Detail running state，header right pill 顯示 amber dot + pulse ring + 「執行中」label + caret slot（caret 內容由 caller `RunDetailRunning` 帶 mono narration、本 change 不負責 stream-tail 內容）
- 開 Goal Detail done state，header right pill 顯示 green dot + 「完成」label
- 開 Goal Detail interrupted state，header right pill 顯示 amber dot + 「已中斷」label（**banner / Retry 行為不在本 change**）
- Quiz review / answering 結果 fail 顯示 red dot + 「未通過」pill；pass 顯示 green dot + 「通過」pill
- `prefers-reduced-motion: reduce` 啟用時，running pulse ring 靜態（無動畫）但顏色保留

**Interface**

```tsx
// codebus-app/src/components/ui/StatusPill.tsx
export type StatusPillStatus = "done" | "interrupted" | "failed" | "running";
export type StatusPillVariant = "dot" | "pill";

export interface StatusPillProps {
  status: StatusPillStatus;
  variant: StatusPillVariant;
  caret?: React.ReactNode; // 只在 variant === "pill" && status === "running" 時 render
  className?: string;
}

export function StatusPill(props: StatusPillProps): JSX.Element;
```

- `dot` variant：7px 圓、`bg-status-{status}`、不渲染 label / caret
- `pill` variant：dot + label（從 `workspace.status.{status}` i18n key 讀）+ 1px border tint bg（border 跟 bg 用 status color + alpha）
- `dot` + `running` 組合**禁止**：runtime invariant 觸發 dev-mode warn（不 throw，避免 prod break；但 dev console 提示），且 type-level 透過 conditional render 自動忽略 caret

**Token contract**

```css
/* codebus-app/src/styles/tokens.css @theme block 追加 */
--color-status-done: var(--color-success);
--color-status-interrupted: var(--color-warn);
--color-status-failed: var(--color-error);
--color-status-running: var(--color-warn);
```

**Pulse 動畫 contract**

```css
/* codebus-app/src/styles/globals.css 追加 */
.status-pill__dot--running-ring {
  box-shadow: 0 0 0 4px var(--color-accent-tint);
}

@media not (prefers-reduced-motion: reduce) {
  @keyframes status-pulse {
    0%,
    100% {
      box-shadow: 0 0 0 4px var(--color-accent-tint);
    }
    50% {
      box-shadow: 0 0 0 6px var(--color-accent-tint);
    }
  }
  .status-pill__dot--running-animated {
    animation: status-pulse 1.4s ease-in-out infinite;
  }
}
```

reduce 模式：`.status-pill__dot--running-ring` 套靜態 ring（不動畫）；`.status-pill__dot--running-animated` 不存在於 reduce 模式（媒體查詢未命中 → 規則不 emit）。

**i18n contract**

```ts
// codebus-app/src/i18n/messages.ts zh + en namespace workspace.status
{
  done: "完成" | "Done",
  interrupted: "已中斷" | "Interrupted",
  failed: "失敗" | "Failed",
  running: "執行中" | "Running",
}
```

**Acceptance criteria**

- vitest 跑 `StatusPill.test.tsx` 全綠：8 cases（4 status × 2 variant）+ pulse reduce-motion fallback + dev-mode invariant warn 不 throw + caret 只在 running + pill 出現
- vitest 跑 sweep 過的 component 既有 test snapshot regen 通過
- `pnpm typecheck` 全綠
- CDP smoke（依 user memory `project_webview2_cdp_real_frontend.md` 操作）：
  - `getComputedStyle(document.documentElement).getPropertyValue('--color-status-done')` 回非空字串
  - Goals 頁建 3 個 row 分別三態，截圖比對 row dot 色
  - Goal Detail 跑進 running、done、interrupted 三 state，截圖比對 header pill
  - Quiz pass / fail 各跑一次，截圖比對 result tag
- grep `codebus-app/src/components/workspace/` 內 hard-code status color class（`text-amber-` / `text-green-` / `text-red-` / `bg-amber-` / `bg-green-` / `bg-red-` 等 Tailwind palette）數量明顯下降；任何剩餘必須是非 status 用途

**In scope**

- token 新增、StatusPill 元件、i18n key、6 個 sweep site、pulse 動畫 reduce-motion gate

**Out of scope**

- 02c banner / Retry / 重命名（→ `interrupted-state-formalize`）
- 02 header layout 統一（→ 02 view full change）
- GP8 running row auto-expand（→ 後續 change）
- QF1 hero icon（自成視覺、不收進 StatusPill）
- 既有 `--color-success` / `--color-warn` / `--color-error` 重命名或刪除

## Risks / Trade-offs

- **Tailwind v4 emit gotcha 重蹈** → token 跟元件 sweep 同 change land；CDP smoke 在驗收條件強制驗 `getPropertyValue`，落地後立刻檢查
- **`--color-status-interrupted` 跟 `--color-status-running` 同 hue 可能讓 user 誤判** → motion + caret + label 是區分軸；design v1 reply 明確要求；後續若 user 真混淆再評估是否升 ODI 設計議題
- **`dot` + `running` runtime invariant 只 warn 不 throw** → trade off 是 prod 安全 vs dev 學習速度；warn 在 dev console 夠醒目，且 type 系統會傾向阻擋（caret 自動不 render 也是訊號）
- **Sweep 範圍跨 Quiz + Workspace 兩 subsystem** → 仍屬同一 UI primitive（status 視覺），且 sweep 量小（6 site），不違反「跨 3+ 不相關 subsystem」決策門檻
- **Pulse 動畫在 low-end 機器可能丟 frame** → 動畫單純 box-shadow（GPU-friendly）+ 1.4s loop 不密集；若實機踩到再評估改 transform: scale
