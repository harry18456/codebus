# i18n-sweep-phase-3a-followup · en-locale CDP smoke report

Date: 2026-05-26
Method: tauri-debug.ps1 launched by user → CDP override (`force-en.mjs`
injected `navigator.language` getter, reloaded page) → `cdp.mjs` walks.

## Verified live in en locale

| Screen | Element | Rendered text | Wire source |
| ------ | ------- | ------------- | ----------- |
| Lobby | recent vaults header | `RECENT VAULTS` | Phase 3A |
| Lobby | vault card timestamp | `1d ago` | Cat A 2.2 (`common.daysAgo`) |
| Workspace Goals | new-goal button | `+ New goal` | Cat A 2.1 (`workspace.goals.newGoalButton`) |
| Workspace Goals | RunListItem time-ago | `23h ago`, `1d ago` | Cat A 2.2 (`common.hoursAgo`, `common.daysAgo`) |
| Quiz tab | quizBadge verdict | `5/5 · 40% · fail` | Cat A 2.5 (`quiz.badge.fail`) |
| Chat widget | new-chat button | `+ New chat` | Cat A 2.4 (`chat.button.newChat`) |
| Chat widget | token indicator | `0 ↑` | cherry-pick Cat 2 (`chat.tokens.indicator`) |
| RunDetail Done | header summary | `404s · 2258218 tokens` | Cat A 2.3 (`workspace.run.headerSummary`) |
| RunDetail Done | lint summary | `0 errors · 0 warnings` | Cat A 2.3 (`workspace.run.lintSummary`) |
| Settings | provider CLI field | `OpenAI Codex CLI` | cherry-pick Cat 1 (`settings.providerCli.fieldLabel`) |

## Covered by unit tests (not exercised live)

- `ActivityStreamItem.bannerLabel` 10 cases (start / goal / sync_start /
  sync_done / pii_summary / lint_start / lint_done / commit_done / done /
  hint) × 2 locales = 20 expectations. Live triggering would require a
  long-running real goal with shell tool + commit + done; unit test
  fixtures cover each case deterministically.

## Out of scope, captured for next follow-up

Pattern 5 / Pattern 6 sweep found these (NOT in this change's user-locked
Cat A scope; user explicitly cherry-picked Cat 1 + Cat 2 and split Cat 3):

- `src/lib/ipc.ts` validation `message:` strings at lines 339 / 351 / 360
  / 483 / 489 — surfaces in form error banners. Architectural decision
  needed (route through `errors.ts` LocalizedError seam vs. inline `t()`
  in IPC layer). DEFERRED to a separate change.
- `src/components/settings/SettingsModal.tsx:254` — `Install
  {provider.displayName} first; then reopen Settings.` JSX text. Phase 3A
  residual (Pattern 1a blind spot due to `${}` interpolation before
  Latin). Trivially wireable; deferred to next pass to avoid scope creep.

## Screenshots

- `01-lobby.png`, `02-workspace-goals.png`, `03-quiz-tab.png`,
  `04-chat.png`, `05-rundetail-done.png`, `06-settings.png`
