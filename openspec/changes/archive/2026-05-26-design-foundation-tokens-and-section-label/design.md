## Context

Phase 2 是 codebus-app design-handoff AUDIT 結算階段的「視覺基礎建設」批次。AUDIT 文件已把所有 spec-level 決議 lock 死（字號 +1 級對照表、border promote 數值、`<SectionLabel>` token spec 與 14 個位置歸屬），這份 design 不重複那些 decision，只 cover「實作怎麼落地」的工程選擇。

**現況**：
- `codebus-app/src/styles/tokens.css` 走 Tailwind v4 CSS-first（`@theme` block），**無 `tailwind.config.ts`**
- 全 `src/` 用 inline `text-[Npx]` hard-code font-size 共約 152 處（grep `text-\[`）
- 無 `<SectionLabel>` 元件、無 `cb-section-label` legacy class
- `--color-border` 與 `--color-border-strong` 並存，但前者 `#1f1f1f` 在 ClearType 渲染下實機看不到

**Source of truth**：
- `codebus-app/design-handoff/AUDIT.md`「Cross-cutting · 字號 scale」「Cross-cutting · 04 Lobby G4 / G5」「結算階段 · Next Steps · Phase 2 區」
- `codebus-app/design-handoff/walkthrough-decisions.html` §01.1 token spec、§01.2 14 位置歸屬、§01.4 兩個 edge case
- `codebus-app/design-handoff/v1.1-mocks.html` §04 Wiki page reader、§05 ChatWidget 字號 / token 使用範例

## Goals / Non-Goals

**Goals:**

- 把 design AUDIT 鎖定的字號 +1 級 bump 與 border promote 數值落到 `tokens.css`
- 引入 `<SectionLabel>` 共用元件，API 涵蓋 default / caps / count 三種變體，14 位置 sweep 時可直接套
- 全 `src/` sweep `text-[Npx]` hard-code，能換 token 就換 token；不能換的明確標註理由
- vitest snapshot 重生、typecheck 全綠

**Non-Goals:**

- 不套 `<SectionLabel>` 到 14 個位置（屬 Phase 4，每個位置有 layout 重排同步進行）
- 不做 status 三態 token (`<StatusPill>`) — Phase 3 並行 change
- 不做 i18n sweep — Phase 3 並行 change
- 不引入 `tailwind.config.ts`，新 token 全走 `@theme`
- 不改 design-handoff/ 任何內容
- 不順手做 layout 重排（CTA 進 header、Sidebar 重整、Vault 詞拿掉）

## Decisions

### Token 命名沿用 Tailwind v4 自動 utility 規則

Tailwind v4 會把 `@theme { --text-body: 14px }` 自動產出 `text-body` utility class。沿用此規則，token 命名直接對應 Tailwind utility，不另起 abstraction layer：

- `--text-body` 對應 `text-body`
- `--text-meta` 對應 `text-meta`
- `--text-h-row` 對應 `text-h-row`
- ...

**為什麼不分 `--font-size-*` 跟 utility 分離**：Tailwind v4 設計就是要 token = utility，多包一層 mapping 違背 framework idiom、且讓 sweep 時要記兩套名字。

### SectionLabel 用 CSS pseudo-element 渲染 amber bar，不用 div

採用：

```tsx
<span className="section-label">
  {children}
</span>
```

不採用：

```tsx
<span className="...">
  <div className="bar" />
  {children}
</span>
```

**為什麼**：
- pseudo-element 不進 DOM、不被 screen reader 念出（amber bar 是純視覺裝飾）
- React tree 乾淨、children 不被 wrapper 干擾
- 跟 walkthrough §01.1 spec 寫的 `.section-label::before` 一致

**取捨**：CSS class 改起來要動 `globals.css`，跟 component 不在同一檔；但 codebus-app 已有 `codebus-bus-roll` keyframe + `.font-mono` override 走同模式，pattern 一致。

### SectionLabel API 用 variant prop 而非 sub-component

採用：

```tsx
<SectionLabel variant="caps">Modules</SectionLabel>
```

不採用：

```tsx
<SectionLabel.Caps>Modules</SectionLabel.Caps>
```

**為什麼**：variant 是運行時 prop，更接近其他 codebus-app component pattern（`Button.tsx` 等）；sub-component 在 React 19 後對 TypeScript 比較囉嗦。`caps` 變體未來只用在 Wiki tree 5-bucket，總共 5 處，sub-component 抽象沒回報。

### Sweep 策略：可換 token 就換、case-by-case 保留要寫註解

Sweep 不是 1-to-1 替換。決策表：

| 原 hard-code | 新做法 |
|---|---|
| `text-[13px]` / `text-[14px]`（body text） | 換 `text-body` |
| `text-[14px]` / `text-[15px]`（body-lg / quiz choices） | 換 `text-body-lg` |
| `text-[11px]` / `text-[12px]`（meta、count、timestamp） | 換 `text-meta` |
| `text-[10px]` / `text-[11px]`（micro、section label uppercase tracked） | 換 `text-micro` |
| `text-[18px]` / `text-[20px]`（screen title） | 換 `text-h-row` |
| `text-[20px]` / `text-[22px]`（goal title） | 換 `text-h-detail` |
| `text-[22px]` / `text-[24px]`（quiz question） | 換 `text-h-quiz` |
| `text-[24px]` / `text-[28px]`（empty hero） | 換 `text-h-empty` |
| `text-[56px]` / `text-[64px]` / `text-[72px]`（大 emoji glyph） | **保留 hard-code**，加註解 `large glyph, intentionally outside type scale` |
| 任何不在表內的 size（如 quiz answer pill 用 `text-[10.5px]`） | 對齊最接近 token；若刻意偏離，加註解說明 |

Sweep 完跑 `grep -rn "text-\[" codebus-app/src --include="*.tsx"`，殘留 case 必須對應「大 glyph」或有註解的特例。

### Border token 三層分工

Sweep 後 `--color-border` 全部走新值 `#2a2a2a`。原本「明顯只用在 card 內 row separator 不希望太搶眼」場合，改 explicit 用 `--color-border-hairline: #1f1f1f`（保留舊值）。

```css
--color-border: #2a2a2a;          /* default — visible hairline */
--color-border-strong: #2a2a2a;   /* 留著但跟 default 同值，sweep 後再評估是否 deprecate */
--color-border-subtle: #161616;   /* 不動，更弱的分隔 */
--color-border-hairline: #1f1f1f; /* 新加 — explicit「希望幾乎看不見」 */
```

**為什麼留 `--color-border-strong` 暫時跟 default 同值**：避免這次 sweep 同時刪 token 跟換值兩件事，blast radius 太大；下個 change 評估有無 explicit consumer 才決定刪不刪。

### 不引入 visual regression snapshot 工具

vitest 既有 snapshot 重生 + manual 1920×1080 100% scaling 三畫面（Lobby empty / Workspace empty / Goals populated）目視即可。引入 Playwright visual regression / Percy 等屬另一個議題，不在這次範圍。

## Implementation Contract

**Observable behavior（apply 完成後）：**

1. 啟動 `codebus-app` dev server (`npm run tauri dev` 或 `npm run dev`)，1920×1080 + Windows 100% scaling 下：
   - Lobby empty / Workspace Goals empty / Goals populated 三畫面文字密度肉眼比目前明顯飽滿（body 從 13px → 14px）
   - 所有 `border-border` 分隔線（topbar 底、footer 頂、card 邊框、Goal table row 等）肉眼可見一條 1px 灰線
2. `<SectionLabel>` 元件存在於 `codebus-app/src/components/ui/SectionLabel.tsx`，可被 import；放一個 Storybook-style smoke render 在測試裡確認三 variant 都渲染正確
3. `grep -rn "text-\[" codebus-app/src --include="*.tsx"` 結果大幅減少（從約 152 處 → 預期 <30 處，剩餘為大 emoji glyph 或註解過的刻意特例）

**Interface / data shape:**

```tsx
// codebus-app/src/components/ui/SectionLabel.tsx
export interface SectionLabelProps {
  variant?: "default" | "caps";  // default: "default"
  count?: number | string;        // optional, 右側 mono 顯示
  className?: string;             // 接 caller 自訂 layout class
  children: React.ReactNode;
}

export function SectionLabel(props: SectionLabelProps): JSX.Element;
```

CSS classes (`codebus-app/src/styles/globals.css`)：

```css
.section-label { /* default treatment per walkthrough §01.1 */ }
.section-label::before { /* amber 2px bar */ }
.section-label--caps { /* uppercase + tracked override */ }
.section-label__count { /* mono right-aligned */ }
```

Token additions (`codebus-app/src/styles/tokens.css`)：在既有 `@theme` 內加 `--text-body` / `--text-body-lg` / `--text-meta` / `--text-micro` / `--text-h-row` / `--text-h-detail` / `--text-h-quiz` / `--text-h-empty`；改 `--color-border`；加 `--color-border-hairline`。

**Failure modes:**

- vitest snapshot fail 是預期的（字號 / border 變），重生即可，不算 regression
- typecheck 若 fail：通常是 SectionLabel API 用錯（如忘了 children）— 修使用點即可
- Tailwind utility 認不到新 token：檢查 `@theme` 是否正確在 `@import "tailwindcss"` 之後（globals.css 順序已正確）
- 若 sweep 後某 component 渲染走鐘：通常是字號跳級導致 line-height 牽動其他間距，case-by-case 微調但**不回退 token**

**Acceptance criteria:**

- `npm run typecheck` 全綠
- `npm run test` (vitest) 全綠，snapshot 重生後 git status 顯示 snapshot 檔被更新
- `<SectionLabel>` 自身的 unit test 涵蓋：default render / caps variant / count 顯示 / className 合併 / a11y（amber bar 不被 SR 念出）
- 在 1920×1080 + Windows 100% scaling 跑 dev build，目視確認三畫面（Lobby empty / Workspace empty / Goals populated）：
  - 文字比修前明顯飽滿
  - 所有 hairline border 肉眼可見
- `grep -rn "text-\[" codebus-app/src --include="*.tsx" | wc -l` 從 152 大幅降到 <30；剩餘 case 每個都有理由（大 emoji glyph、註解過的特例）

**In scope:**

- `codebus-app/src/styles/tokens.css` token 擴充與 border 改值
- `codebus-app/src/styles/globals.css` 加 `.section-label*` CSS class
- `codebus-app/src/components/ui/SectionLabel.tsx` 與 test
- 全 `codebus-app/src/` `text-[Npx]` sweep
- vitest snapshot 重生

**Out of scope:**

- 套 `<SectionLabel>` 到 14 個 AUDIT 標記位置（Phase 4 各 change 處理）
- status 三態 token / `<StatusPill>`（Phase 3）
- i18n sweep（Phase 3）
- layout 重排（Phase 4）
- 任何 `codebus-app/design-handoff/` 內容變動
- 任何 `codebus-app/src-tauri/` 或後端變動
- 引入 `tailwind.config.ts`
- 移除 `--color-border-strong`（留下個 change 評估）

## Risks / Trade-offs

- [字號 bump 後某 component 行高被擠破壞 layout] → vitest snapshot diff + manual 1920×1080 三畫面目視；不行就 case-by-case 微調 padding / gap，但不回退 token
- [Sweep 漏網之魚變成「半邊新字號半邊舊」更醜] → 用 grep 跑兩次（sweep 前後），最後再對全 `src/` grep 一次當 final check
- [Tailwind v4 `@theme` token 自動 utility 在 IDE / editor autocomplete 沒被認出] → 不擋實作；Tailwind IntelliSense plugin 升級後自然收斂；過渡期 caller 寫 `text-body` 仍可運作
- [`--color-border-hairline` 沒有 consumer 留空 token] → 這次 sweep 預期沒任何 component 主動切到 hairline；token 留 placeholder 給後續 change（如 Goal table row 內 separator）用，無 dead code 風險
- [`<SectionLabel>` 本 change 不套到 14 位置，可能被誤認 speculative] → 14 位置都在 AUDIT.md / walkthrough-decisions.html §01.2 明確列出、屬 Phase 4 confirmed consumer；component 本身 + 在 design-system spec 鎖 contract 即可，套用 sweep 等 Phase 4 各 layout 重排 change 一起進

## Migration Plan

直接 main、無 rollout gate。Phase 1 已 merged，本 change 跟 Phase 1 互不衝突。

回退策略：若實機驗收發現字號 / border 跑掉，git revert 整個 change（單一 commit 範圍）後重 evaluate。

## Open Questions

- 14 個位置 sweep 完後，是否還有「想用 SectionLabel 但不在 AUDIT 14 位置內」的 case？→ Phase 4 各 change 開工時逐個 review，發現新位置先回頭 ingest 進本 change 的 spec、再去 Phase 4 change 套用
- `--color-border-strong` 是否真該 deprecate？→ 留到 Phase 4 sweep 後再評估；本 change 不動
