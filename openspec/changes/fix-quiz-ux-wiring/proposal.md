## Problem

Five pre-existing `v3-app-quiz` / `fix-app-quiz` gaps surfaced during manual e2e of the (now archived) `quiz-attempt-progress` change:

1. Finishing a quiz (the summary screen) — and the answering view in general — has no control to return to the quiz history; the user is stranded with no way out.
2. Clicking the already-active Quiz tab does nothing; there is no way back to the quiz home from inside a quiz flow.
3. The quiz summary and history badges use pass threshold 80 even though the user configured 75 on disk.
4. The generate spawn always produces 5 questions even when the shared quiz length config is set to 10.
5. The plan spawn intermittently fails with "no `[CODEBUS_QUIZ_SCOPE]` marker" even though the marker is present in the output.

## Root Cause

1 & 2: The QuizTab `ready` phase renders the answering view with no surrounding back affordance; `+ New quiz` is intentionally hidden inside a quiz (fix-app-quiz defect #7) and is not a back control anyway. The Workspace tab button only sets the active tab — re-selecting the already-active Quiz tab does not reset QuizTab's internal phase.

3: The settings store load is wired only to the Settings modal's open effect; there is no app/Workspace startup load. QuizTab reads the store's pass-threshold getter, which returns `config.app.quiz.pass_threshold` or the 80 default; with an unloaded store the config is empty so it returns 80 even though disk holds 75.

4: QuizTab passes a hardcoded question-count constant (5) to the generate spawn and never reads the configured quiz length. The Shared Quiz Config Namespace requirement defines the shared top-level `quiz.default_length` (consumed by CLI and app); the saved config also carries a legacy `app.quiz.default_length`.

5: The plan-marker parser only accepts the marker at the start of a trimmed line. It tolerates preamble on prior lines but not a preamble glued onto the same line as the marker (the same defect class as fix-app-quiz defect #4 for the generate body, which was fixed there but never mirrored for the plan marker). The fix is already implemented and TDD-tested in codebus-core (uncommitted); this change records its `quiz` capability spec delta and commits the code with it.

## Proposed Solution

1: The QuizTab `ready` phase wraps the answering view with a back-to-history control, visible during answering AND on the summary, that returns to the quiz history list without spawning. Non-destructive — answering progress is persisted (the archived quiz-attempt-progress cursor work), so reopening the attempt resumes exactly.

2: Workspace detects re-selection of the already-active Quiz tab and resets QuizTab to its history phase (option B1). QuizTab exposes a reset signal; Workspace triggers it on Quiz-tab re-select.

3: Load the global config into the settings store once at Workspace mount, so the pass-threshold getter reflects the persisted value without requiring the Settings modal to be opened first.

4: Add a settings-store default-length getter that reads the shared top-level `quiz.default_length`, falling back to the legacy `app.quiz.default_length`, then 5 — mirroring the existing SettingsModal resolution and the Shared Quiz Config Namespace requirement, clamped to the valid 3..10 range. QuizTab passes this value as the generate question count instead of the hardcoded constant.

5: The plan-marker parser accepts the marker anywhere in a line (substring find), taking the text after the first marker occurrence — mirroring the generate-body preamble tolerance. Code is already implemented and RED→GREEN tested in codebus-core; this change adds the `quiz` capability spec delta describing the inline-marker tolerance and commits the code.

## Non-Goals

- Not changing the quiz progress sidecar / resume cursor / Review / explanation-wikilink behavior (owned by the archived `quiz-attempt-progress`).
- Not redesigning the plan-confirm flow state machine, the generate spawn, or `+ New quiz` semantics.
- Not adding a language switcher or new settings UI; only wiring already-persisted config into the runtime store at startup.
- Not changing the codebus-quiz SKILL agent contract (marker still mandated on line 1; tolerance is caller-side robustness only).
- macOS / Linux manual acceptance remains deferred to `v3-app-polish-ship` per the roadmap registry.

## Success Criteria

- From the answering view and from the summary, a back-to-quiz-history control returns to the quiz history list and triggers no `spawn_quiz_*`; reopening the attempt resumes via the persisted cursor.
- Re-selecting the Quiz tab while inside any quiz phase returns to the quiz history view.
- With the persisted pass threshold = 75 and the Settings modal never opened in the session, a finished quiz's summary and the history badges show threshold 75 (not 80).
- With the configured quiz length = 10, generating a quiz invokes the generate spawn with question count 10 (clamped to 3..10); a length of 2 clamps to 3, 99 clamps to 10.
- The plan-marker parser returns Scope / NoMatch when the marker is glued onto the same line after a preamble sentence.
- `cargo test -p codebus-core -p codebus-cli`, `cargo test` (tauri), `npx vitest run`, `npm run typecheck` all green (aggregate 0 failed).

## Impact

- Affected specs: `app-workspace`, `quiz`
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/QuizTab.tsx
    - codebus-app/src/components/workspace/Workspace.tsx
    - codebus-app/src/store/settings.ts
    - codebus-core/src/verb/quiz.rs
  - New: (none)
  - Removed: (none)
  - Plus the colocated frontend test files for the modified components/store (QuizTab, Workspace, settings store) and the existing codebus-core quiz parser tests.
