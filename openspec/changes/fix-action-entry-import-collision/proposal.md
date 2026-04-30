## Why

Phase 7 e2e A8 finding（`docs/notes-2026-04-29-phase7-e2e-findings.md` §一.5）：`cargo tauri dev` 起來時 Nuxt 印 warning：

```
WARN  Duplicated imports "ActionEntry", the one from
"web/app/composables/useExplorerStream.ts" has been ignored and
"web/app/composables/useQaSession.ts" is used
```

兩支 composable 在不同 archive 落地（`agent-console-p0` 2026-04-29 / `qa-overlay-p0` 2026-04-29）期間各自 `export interface ActionEntry`。實際 grep `web/app/composables/useExplorerStream.ts:16` 與 `web/app/composables/useQaSession.ts:26` 兩處型別**結構完全相同**（`tool: string` / `observation: string` / `tokens_used: number` / `isError: boolean`）— Phase 7 notes 原寫「兩 type 的欄位不同」是觀察期誤判，今天因兩 type shape 同所以無 runtime 錯。

但：

1. **若任一支未來獨立演進 schema，silent collapse 直接成 production bug**（runtime 拿到非預期 shape，TS compile 通過但行為錯）— 純 type 不會在 vitest / typecheck 抓到
2. **`openspec/specs/qa-overlay/spec.md` §requirement「reactSteps[].actions[]」第 55 行已明文記載「shaped like agent-console-p0's `ActionEntry`」**— 兩處本來就該同 source；DRY 化會把這層結構意圖編碼進 code，不只活在 spec prose
3. Phase 7 e2e Stage 3.6 Q&A drawer 觀察會被這個 warning 干擾，是 noise

實作 Phase 6 step 29（三介入點）/ D-033 Change B 之前先解，避免上一層疊代蓋住這個 silent footgun。

## What Changes

- 新增 `web/app/types/agent-action.ts`：唯一 `ActionEntry` interface 定義（沿用既有四欄位 shape）+ JSDoc cite Explorer console capability spec 與 Q&A overlay capability spec 兩處 `ActionEntry` 引用點
- `web/app/composables/useExplorerStream.ts` 移除 line 16 的 `export interface ActionEntry`，改 `import type { ActionEntry } from '~/types/agent-action'`
- `web/app/composables/useQaSession.ts` 移除 line 26 的 `export interface ActionEntry`，同改 import
- 任何下游 component / vitest 引用 `ActionEntry`（從上述兩支 composable）若是顯式 `import { ActionEntry } from '...composables/use*'` 改指 `~/types/agent-action`；若走 Nuxt auto-import 則無需改
- `openspec/specs/qa-overlay/spec.md` 第 33 行 `actions: ActionEntry[]` 與第 55 行 cross-reference 文字校正：把第 55 行原引用 Explorer 的 `ActionEntry` 結構改為「imported from `web/app/types/agent-action.ts`（single source — 與 Explorer console 的 `useExplorerStream` 共用）」

## Non-Goals

- **不變更 `ActionEntry` schema**（保留 `tool` / `observation` / `tokens_used` / `isError` 四欄位，加減任何欄位另開 change）
- **不抽其他共用 type**（`StepEntry` / `ThoughtEntry` / `TurnState` 各自只在一支 composable 用，無 collision，不在 scope）
- **不重構 SSE event payload shape**（純 frontend type 整理，不動 sidecar API / `agent_action_result` event schema）
- **不解 A9（Tauri stderr）/ A11（demo-synthetic fixture）**（`docs/notes-2026-04-29-phase7-e2e-findings.md` 其他 finding 各自開 change）
- **不解 Phase 6 step 29 / D-033 Change B**（critical-path，本 change 之外）

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `qa-overlay`: `ActionEntry` 引用 source 從 inline export 改指 `web/app/types/agent-action.ts` single source；既有 reactSteps[].actions[] 結構 invariant 不變。第 55 行 Explorer 的 cross-reference 改為直接 cite `web/app/types/agent-action.ts` single source。

> **Note**：Explorer console capability 的 spec 第 45 行只描述 inline shape `{ tool, observation, tokens_used, isError }`，未明確 cite 型名 `ActionEntry`，故 implementation-side import source 改動屬實作細節、無需該 capability 的 spec delta。`useExplorerStream.ts` 的 import 改寫只走 commit 不上 spec。

## Impact

- Affected specs:
  - openspec/specs/qa-overlay/spec.md（Modified：第 33 / 55 行 `ActionEntry` 引用 source 改寫，cite `web/app/types/agent-action.ts` single source）
- Affected code:
  - New:
    - web/app/types/agent-action.ts
  - Modified:
    - web/app/composables/useExplorerStream.ts（remove inline export，加 import）
    - web/app/composables/useQaSession.ts（remove inline export，加 import）
    - web/app/components/qa/QAOverlay.vue（若有顯式 import，重指；走 auto-import 則 no-op）
    - web/app/components/qa/QaTurnCard.vue（同上）
    - web/app/components/qa/QaCitations.vue（同上）
    - web/app/components/console/ConsoleTimeline.vue（同上）
    - web/app/components/console/StepCard.vue（同上）
    - web/app/components/console/ProgressStrip.vue（同上）
    - web/app/components/console/CoverageBanner.vue（同上）
    - web/tests/composables 與 web/tests/components 任何顯式 import `ActionEntry` 的測檔（若有）
  - Removed: 無（舊 `export interface ActionEntry` 從 2 支 composable 移除即可，不刪檔）
