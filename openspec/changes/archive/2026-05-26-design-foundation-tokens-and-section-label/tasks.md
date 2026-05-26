<!--
Behavior + verification target embedded in each task description. File paths
appear as locator context only — task body always names the observable
contract and how to verify it.

`[P]` marks tasks that touch independent files and can run in parallel
within a group (parallel_tasks: true in .spectra.yaml).
-->

## 1. Token foundation in `tokens.css`

- [x] 1.1 Add typography scale tokens (`--text-body` 14px / `--text-body-lg` 15px / `--text-meta` 12px / `--text-micro` 11px / `--text-h-row` 20px / `--text-h-detail` 22px / `--text-h-quiz` 24px / `--text-h-empty` 28px) inside the existing `@theme` block in `codebus-app/src/styles/tokens.css`, so the Tailwind v4 auto-generated utilities (`text-body`, `text-meta`, etc.) render at the spec'd pixel values — implements **Typography scale tokens** requirement and the **Token 命名沿用 Tailwind v4 自動 utility 規則** decision. Verify by writing a temporary `<p className="text-body">test</p>` in `App.tsx`, running `npm run dev`, opening DevTools, confirming computed `font-size: 14px`; then revert the temp markup.
- [x] 1.2 Promote `--color-border` from `#1f1f1f` to `#2a2a2a` and add `--color-border-hairline: #1f1f1f`, keeping `--color-border-strong` at `#2a2a2a` for back-compat — implements **Border color tokens** requirement and the **Border token 三層分工** decision. Verify by running `npm run dev` on Windows 1920×1080 @ 100% scaling and visually confirming the Lobby topbar bottom hairline becomes visible (before this task it is invisible).

## 2. SectionLabel component

- [x] 2.1 Add `.section-label` / `.section-label::before` / `.section-label--caps` / `.section-label__count` CSS classes to `codebus-app/src/styles/globals.css` matching the spec values in `walkthrough-decisions.html` §01.1, so any element with these classes renders the amber-bar + text + optional mono count treatment — supports the **SectionLabel component** requirement and the **SectionLabel 用 CSS pseudo-element 渲染 amber bar，不用 div** decision. Verify by inspecting `globals.css` diff and rendering a smoke `<span className="section-label">test</span>` in a test fixture (covered by 2.3).
- [x] 2.2 [P] Implement the `<SectionLabel>` React component at `codebus-app/src/components/ui/SectionLabel.tsx` exposing the `SectionLabelProps` API (`variant?`, `count?`, `className?`, `children`), wiring `default` / `caps` variants to the CSS classes from 2.1 and rendering the count slot via `.section-label__count` when `count` is provided — implements the **SectionLabel component** requirement and the **SectionLabel API 用 variant prop 而非 sub-component** decision. Verify by running `npm run typecheck` and confirming the file compiles with strict types.
- [x] 2.3 [P] Add `SectionLabel.test.tsx` covering: default variant render, caps variant render (uppercase + tracking + 11px), count slot rendering, `className` merge with internal classes, and screen-reader behavior asserting the amber bar is decorative (not in accessibility tree) — covers all four "**Scenario:**" entries under the **SectionLabel component** requirement. Verify with `npm run test -- SectionLabel` showing all cases green.

## 3. Sweep `text-[Npx]` hard-codes (independent files — `[P]` per file)

- [x] 3.1 [P] In `codebus-app/src/components/BottomStrip.tsx`, replace every `text-[Npx]` arbitrary-value with the matching scale utility per the decision table in design.md §**Sweep 策略**; leave large-glyph sizes (≥ 56px) hard-coded with an inline `// large glyph, intentionally outside type scale` comment. Verify with `grep -n "text-\[" codebus-app/src/components/BottomStrip.tsx` showing only comment-justified or large-glyph residues; rerun `npm run test -- BottomStrip` and regenerate any snapshot that fails due to the size change.
- [x] 3.2 [P] Same sweep treatment in `codebus-app/src/components/DropTargetOverlay.tsx` — sizes mapped per decision table, exemptions commented. Verify per-file grep + `npm run test` regenerating snapshots affected by this file.
- [x] 3.3 [P] Same sweep in `codebus-app/src/components/LoadingOverlay.tsx`, treating the 72px 🚌 emoji as a documented large-glyph exemption. Verify per-file grep + vitest pass.
- [x] 3.4 [P] Same sweep in `codebus-app/src/components/Toast.tsx`. Verify per-file grep + vitest pass.
- [x] 3.5 [P] Same sweep in `codebus-app/src/components/lobby/EmptyState.tsx`, treating the 64px emoji as documented large-glyph exemption. Verify per-file grep + vitest pass.
- [x] 3.6 [P] Same sweep in `codebus-app/src/components/lobby/Lobby.tsx`. Verify per-file grep + vitest pass.
- [x] 3.7 [P] Same sweep in `codebus-app/src/components/lobby/NewVaultFlow.tsx`. Verify per-file grep + vitest pass.
- [x] 3.8 [P] Same sweep in `codebus-app/src/components/lobby/VaultCard.tsx`. Verify per-file grep + vitest pass.
- [x] 3.9 [P] Same sweep in `codebus-app/src/components/settings/CodexEndpointSection.tsx`. Verify per-file grep + vitest pass.
- [x] 3.10 [P] Same sweep in `codebus-app/src/components/settings/EndpointSection.tsx`. Verify per-file grep + vitest pass.
- [x] 3.11 [P] Same sweep in `codebus-app/src/components/settings/SettingsModal.tsx`. Verify per-file grep + vitest pass.
- [x] 3.12 [P] Same sweep in `codebus-app/src/components/workspace/ActivityStreamItem.tsx`. Verify per-file grep + vitest pass.
- [x] 3.13 [P] Same sweep in `codebus-app/src/components/workspace/ChatNewChatButton.tsx`. Verify per-file grep + vitest pass.
- [x] 3.14 [P] Same sweep in `codebus-app/src/components/workspace/ChatTokenDisplay.tsx`. Verify per-file grep + vitest pass.
- [x] 3.15 [P] Same sweep in `codebus-app/src/components/workspace/ChatTranscript.tsx`. Verify per-file grep + vitest pass.
- [x] 3.16 [P] Same sweep in `codebus-app/src/components/workspace/ChatUndoToast.tsx`. Verify per-file grep + vitest pass.
- [x] 3.17 [P] Same sweep in `codebus-app/src/components/workspace/GoalsTab.tsx`. Verify per-file grep + vitest pass.
- [x] 3.18 [P] Same sweep in `codebus-app/src/components/workspace/NewGoalModal.tsx`. Verify per-file grep + vitest pass.
- [x] 3.19 [P] Same sweep in `codebus-app/src/components/workspace/QuizAnswering.tsx`. Verify per-file grep + vitest pass.
- [x] 3.20 [P] Same sweep in `codebus-app/src/components/workspace/QuizGenerationLog.tsx`. Verify per-file grep + vitest pass.
- [x] 3.21 [P] Same sweep in `codebus-app/src/components/workspace/QuizReview.tsx`. Verify per-file grep + vitest pass.
- [x] 3.22 [P] Same sweep in `codebus-app/src/components/workspace/QuizTab.tsx`. Verify per-file grep + vitest pass.
- [x] 3.23 [P] Same sweep in `codebus-app/src/components/workspace/RunDetailCancelled.tsx`. Verify per-file grep + vitest pass.
- [x] 3.24 [P] Same sweep in `codebus-app/src/components/workspace/RunDetailDone.tsx`. Verify per-file grep + vitest pass.
- [x] 3.25 [P] Same sweep in `codebus-app/src/components/workspace/RunDetailRunning.tsx`. Verify per-file grep + vitest pass.
- [x] 3.26 [P] Same sweep in `codebus-app/src/components/workspace/RunListItem.tsx`. Verify per-file grep + vitest pass.
- [x] 3.27 [P] Same sweep in `codebus-app/src/components/workspace/WatcherStatusBanner.tsx`. Verify per-file grep + vitest pass.
- [x] 3.28 [P] Same sweep in `codebus-app/src/components/workspace/WikiPreview.tsx`. Verify per-file grep + vitest pass.
- [x] 3.29 [P] Same sweep in `codebus-app/src/components/workspace/WikiTab.tsx`. Verify per-file grep + vitest pass.
- [x] 3.30 [P] Same sweep in `codebus-app/src/components/workspace/WikiTree.tsx`. Verify per-file grep + vitest pass.
- [x] 3.31 [P] Same sweep in `codebus-app/src/components/WindowControls.tsx` if any `text-[Npx]` residue is found by grep. Verify per-file grep + vitest pass; if no `text-[Npx]` is present, mark done with note "no hard-coded sizes in this file".
- [x] 3.32 [P] Same sweep in `codebus-app/src/components/workspace/Workspace.tsx` (3 occurrences at L248/L264/L386 — `text-[12px]`/`text-[11px]` → `text-meta`). This file was missing from the original 3.1-3.31 list but falls within the **Hard-coded font-size sweep convention** requirement's "all `codebus-app/src/`" scope. Verify per-file grep shows zero residue.

## 4. Sweep convention enforcement & global verification

- [x] 4.1 Run a final repo-wide check `grep -rn "text-\[" codebus-app/src --include="*.tsx" | wc -l` and confirm the count is < 30; for each remaining match, confirm it has the `// large glyph, intentionally outside type scale` comment or another documented justification — enforces the **Hard-coded font-size sweep convention** requirement and validates the **Sweep 策略：可換 token 就換、case-by-case 保留要寫註解** decision. Verify by pasting the final grep output and exemption justifications into the apply session log.
- [x] 4.2 Run `npm run typecheck` from `codebus-app/` and confirm zero errors — validates the **Design tokens are the single source for color, typography, and border** requirement holds for the typed call sites. Verify the command exits 0.
- [x] 4.3 Run `npm run test` from `codebus-app/` (vitest), regenerate any snapshot files that drift purely because of the new typography / border values, and confirm the test suite goes green — including the new `SectionLabel.test.tsx` cases from 2.3. Verify by inspecting `git diff` to ensure only snapshot files and intentionally-touched components changed, and by confirming `npm run test` exits 0.

## 5. Manual visual smoke (replaces visual regression tooling per the **不引入 visual regression snapshot 工具** decision)

- [x] 5.1 Start `npm run tauri dev` on a Windows 1920×1080 display at 100% scaling, take screenshots of the three reference views (Lobby empty / Workspace Goals empty / Goals populated), and confirm visually that (a) body text density looks fuller vs. pre-change baseline, (b) every `border-border` hairline (topbar bottom, footer top, card borders, Goal table row separators) is visible — implements the **Implementation Contract** "Observable behavior" acceptance criteria. Verify by attaching the three screenshots to the apply session log and writing one sentence each confirming both density and hairline visibility.
