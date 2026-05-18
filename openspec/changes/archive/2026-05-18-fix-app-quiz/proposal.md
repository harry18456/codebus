## Problem

`v3-app-quiz` (archived 2026-05-16) shipped on automated tests only — Rust core/CLI/Tauri unit suites plus 338 vitest. The roadmap Cross-platform policy requires each change's acceptance checklist to be run and pass on Windows, and for a GUI change that acceptance is a human `cargo tauri dev` walkthrough — which was never performed before archive. Four known follow-up gaps were carried out of the change unaddressed, and any additional defects surfaced by the Windows manual end-to-end run are currently uncaptured.

Known gaps carried out of `v3-app-quiz`:

1. **Pass threshold not wired to settings.** `app-workspace` / Quiz Answering and Summary requires the summary pass/fail to be computed client-side using `app.quiz.pass_threshold`. The implementation uses a hardcoded `DEFAULT_PASS_THRESHOLD = 80` constant in the Quiz tab and the settings store exposes no `pass_threshold` field, so changing the setting has no effect on the summary outcome.
2. **View-generation-log does not render the timeline.** `app-workspace` / Quiz History List requires the view-generation-log affordance to render the attempt's generate-spawn `events.jsonl` through the existing agent stream rendering. The implementation only surfaces the `events_log` path string; it does not replay the events through the existing stream-rendering pipeline already used by the run detail view.
3. **`events_log` end-to-end unverified.** Core tests asserted the persisted quiz markdown `events_log` frontmatter against a mock path. Whether the field points at the real generate-spawn `events.jsonl` with that spawn's actual event content has never been verified end-to-end.
4. **Frontend type/lint hygiene not run.** The frontend was validated with vitest only; `tsc --noEmit` and eslint were never run against the quiz changes, so latent type or lint errors are unknown.

## Root Cause

The change was archived after automated suites passed, treating green unit/vitest as equivalent to the required Windows acceptance run. Automated vitest mocks the Tauri IPC boundary and does not exercise real `spawn_quiz_plan`/`spawn_quiz_generate` against a real `claude` spawn, so spec-vs-implementation gaps (1, 2), an unverified persistence path (3), and untyped/unlinted code (4) all passed through the archive gate. The manual acceptance step that would have exposed them was skipped and not prompted for.

## Proposed Solution

Open this change as the remediation container and work it test-and-fix:

- Wire the Quiz summary pass/fail to `app.quiz.pass_threshold` via the settings store, replacing the hardcoded constant, so the summary outcome honors the configured threshold (default remains 80 only when the key is absent).
- Replace the view-generation-log path-only affordance with a render of the attempt's generate-spawn `events.jsonl` through the existing agent stream rendering pipeline (the same `ThoughtItem`/`ActivityStreamItem`/phase replay used by the run detail done view), reusing it rather than reimplementing.
- Verify end-to-end that the persisted quiz markdown `events_log` frontmatter resolves to the real generate-spawn `events.jsonl` containing that spawn's events; fix the wiring if it does not.
- Run `tsc --noEmit` and eslint over the codebus-app frontend and fix any type/lint errors attributable to the quiz changes.
- Run the Windows manual end-to-end acceptance for the quiz flows (CLI quiz, GUI plan-confirm-generate, wiki-preview Page flow, history + view-log, shared config namespace isolation). Defects discovered during that run are folded into this change via `/spectra-ingest` and fixed in the same `/spectra-apply` pass — test-and-fix in one container.

## Non-Goals

- macOS / Linux manual acceptance — remains deferred to `v3-app-polish-ship` per the roadmap Cross-platform policy and the existing deferred-acceptance registry entry; this change is Windows-only.
- New quiz features, spaced repetition, history charts, or any item in the `v3-app-quiz` out-of-scope list — this change only closes gaps against the already-archived quiz contract.
- Changing quiz runtime behavior or adding features — the quiz behavior requirements are correct and the implementation is brought into compliance; the only contract delta is the mechanical IPC registry count 22 → 23 needed to render the view-log timeline.

## Success Criteria

- Changing `app.quiz.pass_threshold` in settings changes the Quiz summary pass/fail boundary; with the setting at 80, a 5-question quiz with 4 correct shows a passing outcome, and with the setting at 90 the same 4/5 shows a failing outcome — verified by a test driving the settings store value (not a prop).
- Activating view-generation-log on a history attempt row renders that attempt's generate-spawn `events.jsonl` through the existing agent stream rendering (Thought/ToolUse/result items appear), not a bare path string — verified by a frontend test asserting stream-rendered items.
- A persisted quiz markdown's `events_log` frontmatter resolves to a real file whose contents are the generate spawn's events for that attempt — verified by a CLI-level end-to-end assertion against a real (mock-claude) generate spawn.
- `npx tsc --noEmit` and eslint pass for codebus-app with no errors attributable to quiz code.
- The Windows manual end-to-end acceptance checklist for all five quiz areas is executed and every item either passes or has its defect ingested into this change and subsequently fixed; the deferred-acceptance registry note for Windows is accurate.

## Impact

- Affected specs: `app-workspace` (MODIFIED: Quiz Answering and Summary, Quiz History List, Tauri IPC Commands for Quiz Plan and Generate Lifecycle). Quiz Answering/History are tightened with explicit "SHALL NOT" clauses (no hardcoded threshold constant; path-string display does not satisfy view-log) plus pinning scenarios — no behavior contract change there. The IPC requirement IS a contract change: rendering the view-generation-log timeline requires a new `read_quiz_events` command (no existing IPC reads an events.jsonl by path; quiz history is filesystem-scanned, not RunLog-correlated), so the quiz command count goes 5 → 6 and the registry total 22 → 23. `quiz` (MODIFIED: Quiz Verb Two-Shot Flow) is tightened for manual-e2e defect #3: the caller's plan-marker parser tolerantly recovers the marker (strip leading code fence, accept first marker line despite agent preamble — mirroring the D4 fence tolerance already applied to the generate body) and the no-marker error carries a truncated diagnostic head of the spawn output. The SKILL agent contract (marker on the first line) is unchanged. `quiz` (MODIFIED: Quiz Markdown Schema and Caller Frontmatter Injection) is tightened for manual-e2e defect #4: the caller also tolerantly discards any agent preamble before the first `## Q1.` heading (including a preamble glued onto the same line) so the persisted/parsed body begins exactly at the first question — same tolerant-cleanup shape as the fence handling; SKILL still prohibits preamble. `app-workspace` (MODIFIED: Quiz Tab Plan-Confirm-Generate Flow) is tightened for manual-e2e defect #5: a pinning scenario + clause make explicit that the Quiz tab SHALL subscribe to `quiz-stream` and render plan/generate agent activity live through the existing stream rendering (a static label alone does not satisfy it) — a frontend compliance fix to the already-required behavior, no contract change. `app-workspace` (MODIFIED: Quiz History List, re-authored for UX feedback #6): the view-generation-log affordance moves off the history row and into the opened attempt detail view, presented as a centered modal (reusing the existing `Dialog` primitive) instead of an inline-expand panel; row no longer inline-expands. Supersedes the D2 row-level entry point (the `read_quiz_events` IPC and `QuizGenerationLog` component are unchanged). `app-workspace` (MODIFIED: Quiz Tab Plan-Confirm-Generate Flow, additional scenario for manual-e2e defect #7): the `+ New quiz` control is rendered only in the history list and the topic-input compose screen, and SHALL NOT appear while inside a quiz flow or an opened attempt — a frontend-only conditional-render fix that also removes the window-controls overlap in the answering view.
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/QuizTab.tsx
    - codebus-app/src/components/workspace/QuizAnswering.tsx
    - codebus-app/src/store/settings.ts
    - codebus-app/src/components/settings/SettingsModal.tsx
    - codebus-app/src/lib/ipc.ts
    - codebus-app/src-tauri/src/ipc/quiz.rs
    - codebus-app/src-tauri/src/ipc/mod.rs
    - codebus-app/src-tauri/tests/keyring_ipc.rs
    - codebus-core/src/verb/quiz.rs
    - codebus-cli/src/commands/quiz.rs
    - docs/v3-app-roadmap.md
  - New:
    - codebus-app/src/components/workspace/QuizGenerationLog.tsx
  - Removed: none
