## Context

Manual e2e of the archived `quiz-attempt-progress` surfaced five pre-existing `v3-app-quiz` / `fix-app-quiz` gaps (see proposal). This change fixes them without touching the quiz progress sidecar / cursor / Review behavior that `quiz-attempt-progress` already owns. `.spectra.yaml`: tdd, audit, parallel_tasks enabled; locale tw.

## Goals / Non-Goals

### Goals

- A discoverable, non-destructive way out of an in-progress quiz (back-to-history control + Quiz-tab re-select).
- The quiz runtime reflects the persisted `quiz.*` / `app.quiz.*` config (pass threshold, question count) without needing the Settings modal opened.
- The plan-marker parser tolerates a preamble glued onto the marker line (parity with the generate-body fix).

### Non-Goals

- No change to the progress sidecar / resume cursor / Review / explanation wikilinks (archived `quiz-attempt-progress`).
- No plan-confirm state-machine / generate-spawn / `+ New quiz` redesign.
- No new settings UI or language switcher; only startup wiring of existing config.
- The codebus-quiz SKILL agent contract is unchanged (marker still mandated on line 1).

## Decisions

### D1: Back-to-history control in the answering/summary view

The QuizTab `ready` phase wraps the answering view with a `quiz-back-to-history` control (same testid + `setPhase("history")` behavior as the existing idle-phase control; the two never render simultaneously). It is shown during answering AND on the summary screen. Returning is non-destructive: answering progress is persisted by the archived cursor work, so reopening the attempt resumes exactly. It does not spawn. `+ New quiz` stays hidden inside a quiz (unchanged, fix-app-quiz defect #7).

### D2: Re-selecting the active Quiz tab returns to quiz history (B1)

Workspace owns tab state. When the user selects the Quiz tab while it is already the active tab, Workspace increments a `quizHomeSignal` counter passed to QuizTab as a prop. QuizTab runs an effect on that signal that calls `setPhase("history")` (ignoring the initial 0 value). Non-destructive (progress persisted). Selecting Quiz from another tab is unchanged (no reset). Rejected B2 (rely only on the D1 button): the user explicitly expects the tab to act as "home"; the tab path is the discoverable affordance they reached for.

### D3: Load global config into the settings store at Workspace mount

`useSettingsStore.load()` is currently only called by the Settings modal's open effect, so `getPassThreshold()` falls back to the 80 default when the modal was never opened. Workspace calls `useSettingsStore.load()` once on mount (guarded so it does not refight an in-flight/loaded state and does not clobber unsaved edits — load only when the store is still at its empty initial config). QuizTab already reads the threshold through a reactive store selector, so it re-renders with the persisted value once the load resolves.

### D4: Question count comes from the shared quiz length config

Add `useSettingsStore.getDefaultLength()` returning `config.quiz?.default_length ?? config.app?.quiz?.default_length ?? 5`, clamped to the inclusive 3..10 range (the same range the core `quiz` config loader enforces). The shared top-level `quiz.default_length` is authoritative per the Shared Quiz Config Namespace requirement; the legacy `app.quiz.default_length` is tolerated for un-migrated configs (mirrors the existing SettingsModal resolution). QuizTab passes `getDefaultLength()` to the generate spawn instead of the hardcoded constant. Combined with D3 the value reflects the persisted config.

### D5: Plan-marker parser tolerates an inline (same-line) marker

`parse_plan_outcome` accepts the first line that *contains* `[CODEBUS_QUIZ_SCOPE]` / `[CODEBUS_QUIZ_NO_MATCH]` (substring find), taking the payload after the first marker occurrence — mirroring the generate-body `strip_preamble_before_first_question` tolerance. The code is already implemented and RED→GREEN unit-tested in codebus-core (uncommitted); this change records the `quiz` capability spec delta and commits the code with the change. The SKILL still mandates the marker on line 1 (caller-side robustness only).

## Implementation Contract

- **D1 back control**: QuizTab `ready` renders a `quiz-back-to-history` control during answering and summary; activating it sets phase to `history` and invokes no `spawn_quiz_*`. Verified by a QuizTab test (open attempt → answering → back → history list shown; finish quiz → summary → back → history; `invokedCommands` excludes `spawn_quiz_*`).
- **D2 tab reset**: re-selecting the already-active Quiz tab sets QuizTab to the `history` phase; selecting Quiz from another tab does not reset. Verified by a Workspace test (enter quiz flow, click Quiz tab again → quiz history visible) and a QuizTab test (incrementing `quizHomeSignal` prop → history phase).
- **D3 startup load**: Workspace mount calls `useSettingsStore.load()` exactly once when the store is at its empty initial config. Verified by a Workspace test asserting `load_global_config` IPC is invoked on mount; and a QuizTab/summary test where the store config has `app.quiz.pass_threshold: 75` and the summary outcome boundary uses 75 (not 80).
- **D4 question count**: `getDefaultLength()` returns shared `quiz.default_length`, else legacy `app.quiz.default_length`, else 5, clamped to 3..10. QuizTab passes it to `spawnQuizGenerate`. Verified by settings-store unit tests (each precedence + clamp at 2→3 and 99→10) and a QuizTab test asserting the generate spawn receives the configured count.
- **D5 inline marker**: `parse_plan_outcome` returns `Scope` / `NoMatch` for input where a preamble sentence precedes the marker on the same line with no newline. Verified by the existing codebus-core unit tests (`parse_scope_marker_glued_after_inline_preamble_is_recovered`, `parse_no_match_marker_glued_after_inline_preamble_is_recovered`).
- **In scope**: the five items above. **Out of scope**: progress sidecar/cursor/Review/wikilinks (archived), confirm-flow state machine, generate-spawn internals, SKILL contract, macOS/Linux manual acceptance (deferred to `v3-app-polish-ship`).

## Risks / Trade-offs

- D3 load on mount is async; until it resolves the threshold briefly reflects the default. Acceptable — the summary is only reachable after answering, long after mount; the selector re-renders on resolve.
- D2 overloads tab-click with a reset side effect (unusual). Mitigated: only triggers on re-selecting the already-active Quiz tab, and is non-destructive because progress is persisted.
- D4 clamp diverges silently from an out-of-range configured value. Acceptable and consistent with the core loader, which already rejects out-of-range to the default.

## Migration Plan

No migration. No config schema change (only consuming existing keys). No persisted-data format change. The plan-marker parser change is backward compatible (a line-start marker still parses).
