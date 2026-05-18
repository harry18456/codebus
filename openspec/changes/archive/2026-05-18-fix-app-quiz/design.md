## Context

`v3-app-quiz` is archived. This change is a Windows-only remediation container that closes four known compliance/verification gaps and absorbs defects found by the deferred Windows manual end-to-end acceptance. The archived `app-workspace` quiz behavior requirements are correct and the implementation is brought into compliance with them — the one contract change is mechanical: rendering the view-generation-log timeline needs a new read-only `read_quiz_events` IPC, taking the quiz command count 5 → 6 / registry total 22 → 23. Work is iterative: the manual acceptance run feeds defects back via `/spectra-ingest` into this same change.

Grounded current state:
- `codebus-app/src/store/settings.ts` exposes no `pass_threshold`; the Quiz tab passes a module-level `DEFAULT_PASS_THRESHOLD = 80` constant (QuizTab.tsx, with an in-code follow-up note) into the answering view.
- The history attempt row renders only the `events_log` string (testid `quiz-view-log-path`); it does not replay the events.
- `codebus-app/src/components/workspace/RunDetailDone.tsx` already replays a run's `events.jsonl` via `phasesFromEvents`, `ThoughtItem`, and `ActivityStreamItem` — this is the existing agent stream rendering to reuse.

## Goals / Non-Goals

### Goals

- Quiz summary pass/fail honors `app.quiz.pass_threshold` from the settings store.
- View-generation-log renders the attempt's generate-spawn events through the existing stream rendering, not a path string.
- The persisted `events_log` frontmatter is verified end-to-end to point at the real generate-spawn events file.
- Frontend `tsc --noEmit` + eslint clean for quiz code.
- Windows manual acceptance executed; discovered defects ingested and fixed in this container.

### Non-Goals

- No macOS / Linux acceptance (stays deferred to `v3-app-polish-ship`).
- No new quiz features or runtime behavior changes beyond the read-only `read_quiz_events` IPC (the only contract delta is the registry count 22 → 23).

## Decisions

### D1: pass_threshold flows settings store → QuizTab → QuizAnswering

Add a `pass_threshold` field to the settings store (`codebus-app/src/store/settings.ts`) sourced from the same app config the SettingsModal already binds (`app.quiz.pass_threshold`), defaulting to 80 only when the key is absent (mirrors the existing `quiz.default_length` default-when-absent pattern). QuizTab reads it from the store and passes it as the `passThreshold` prop already accepted by QuizAnswering. The hardcoded `DEFAULT_PASS_THRESHOLD` constant is removed. Rejected: reading config directly in QuizAnswering — keeps the component prop-driven and unit-testable, consistent with how it is already structured.

### D2: view-generation-log reuses RunDetailDone's stream rendering

Investigation during apply found there is **no existing IPC** that reads an events.jsonl by path: `get_run_detail` only reads by `runId` via RunLog, and quiz history is filesystem-scanned (design D5/D7), not RunLog-correlated — the attempt only carries an `events_log` absolute path. So the timeline render needs a new backend command.

Add `read_quiz_events(vault_path, path) -> Vec<EventEnvelope>` in `codebus-app/src-tauri/src/ipc/quiz.rs`: read the jsonl at `path`, parse one `EventEnvelope` per line (malformed lines skipped, not fatal), and reject any `path` not under the vault `.codebus/` tree with `AppError::Invalid { field: "path" }` — mirroring the existing `read_quiz_attempt` containment guard (not a weaker check). Register it in `ipc/mod.rs` (`generate_ipc_handler!` + `REGISTERED_COMMANDS`); the registry count goes 22 → 23, so the `exactly_twenty_two_commands` / `command_names_match_spec` tests in `ipc/mod.rs` and the count assertion in `tests/keyring_ipc.rs` are updated to 23. Add a typed `readQuizEvents` wrapper in `codebus-app/src/lib/ipc.ts`.

The frontend `QuizGenerationLog.tsx` takes the attempt's `events_log` path, calls `readQuizEvents`, and renders the **existing** stream rendering — `foldTimeline` + `ThoughtItem` + `ActivityStreamItem` from `./ActivityStreamItem` (the same fold `RunDetailDone.tsx` uses). The history row's view-log affordance opens this component instead of showing the bare path. Rejected: a new bespoke renderer (spec mandates "using the existing agent stream rendering"); rejected: a generic read-any-file IPC (the containment guard MUST be scoped to the vault `.codebus/` tree, per the audit Scoundrel lens — an unbounded path read is a traversal sink).

### D3: events_log verified at the CLI layer with mock-claude

Per the established test-layering convention (core = unit, CLI = end-to-end mock-claude spawn), add a CLI-level assertion that after a real generate spawn the persisted quiz markdown's `events_log` frontmatter resolves to a file on disk whose contents are that spawn's events. If the wiring is wrong, fix the sink/frontmatter path in `codebus-core/src/verb/quiz.rs` / `codebus-cli/src/commands/quiz.rs`. This replaces the prior mock-path-only core assertion.

### D4: manual acceptance is a task with a written checklist; defects ingested

The Windows manual end-to-end run is a tracked task whose verification target is the five-area checklist (CLI quiz; GUI plan-confirm-generate; wiki-preview Page flow; history + view-log; shared config namespace isolation). It is run during `/spectra-apply`. Any defect becomes a new task via `/spectra-ingest` into this change before the change is considered complete. On completion the roadmap deferred-acceptance registry note is updated so the Windows line is accurate.

### D5: Quiz tab default is history; `+ New quiz` opens a distinct topic-input view (manual-e2e defect #2)

Manual e2e found `+ New quiz` is a dead button on the default screen: the `idle` phase conflates the topic input and the history list, and the control only runs `setPhase("idle")`, a no-op when already `idle`. The `app-workspace` Quiz Tab Plan-Confirm-Generate Flow requirement says `+ New quiz` SHALL *open* a free-text topic input — i.e. a distinct action, not a view that is always present.

Resolution: split the conflated `idle` into two states. A new mount-default `history` phase renders the history list only (no topic input). The `idle` phase renders the topic-input + Start only (no history list). `+ New quiz` transitions `history → idle` (and remains the reset target from `ready`); the input view carries a `← History` affordance back to `history`. Empty history still shows its existing "no quizzes yet" hint in the `history` view.

The `[Quiz me on this]` Page flow MUST keep working: its `pendingPage` effect currently guards on `phase === "idle"`, but the mount default is no longer `idle`. The guard is changed to fire when `pendingPage` is set on mount/prop-change regardless of the history/idle default (it already calls `onPendingConsumed` and generates directly), so wiki-preview Page-scope still skips planning. Scope boundary: this is a GUI state-machine compliance fix for the archived Quiz Tab requirement — no change to plan/generate/persist behavior, no new IPC.

### D6: Plan-marker parser tolerant recovery + diagnostic (manual-e2e defect #3)

Manual e2e hit `VerbError::Internal { message: "quiz plan spawn produced no [CODEBUS_QUIZ_SCOPE]/[CODEBUS_QUIZ_NO_MATCH] marker on its first line" }` against a real claude spawn. Two grounded faults, neither a guess:

1. **The error hides its own evidence.** `parse_plan_outcome` returns `None` and the caller raises an opaque error that does not include the actual `plan_text`; both CLI and GUI then print only the opaque message, so the failure cannot be diagnosed without code changes. This is the same anti-pattern called out in the grounded-debugging guidance (don't bury the failure).
2. **The parser is brittle where the sibling path is tolerant.** `parse_plan_outcome` only `trim_start()`s then requires the marker at offset 0. D4 already established that the SKILL prohibits code fences / preamble for the generate body **but the caller tolerantly strips** them (`strip_code_fence`). The plan marker never got the same tolerance, so any real-LLM preamble ("Sure, here is…") or wrapping fence hard-fails an otherwise-correct plan.

Resolution (mirror D4, do not relax the agent contract): `parse_plan_outcome` SHALL strip a leading code fence (reuse `strip_code_fence`) and SHALL scan for the first line beginning with either marker rather than requiring offset 0; the SKILL still mandates the marker as the first line. The `None` path SHALL build the `Internal` error with a truncated head (≤200 chars) of `plan_text` so every future failure is self-diagnosing. If, with the diagnostic in place, real output shows the agent emits no marker at all (SKILL not activating / ignored), that is a separate SKILL-prompt follow-up — the diagnostic makes that determinable instead of guessed.

Scope boundary: caller-side parser robustness + error diagnostics in `codebus-core/src/verb/quiz.rs`; no change to the SKILL agent contract, spawn invocation, plan/generate/persist behavior, or IPC.

### D7: Strip generate-body preamble before the first question (manual-e2e defect #4)

A grounded CLI end-to-end run (throwaway vault with the codebus-quiz skill + real wiki) succeeded, but the persisted quiz markdown began (after caller frontmatter) with `讀取三個指定的 wiki 頁面以產生測驗題目。## Q1. <stem>` — the generate agent emitted a preamble sentence and glued it onto the first `## Q1.` heading on the same line. `quiz_md` is only run through `strip_code_fence` (`run_quiz_generate`, the single cleanup point feeding both the GUI `QuizReport` and CLI `persist_quiz`); a non-fence preamble is therefore persisted verbatim AND breaks parsing — `parseQuiz` / the answering view splits on `^##\s+Q\d+\.` (line-anchored), so a preamble glued ahead of `## Q1.` makes the first question unparseable.

Resolution (same shape as D4/D6 — SKILL prohibits, caller tolerantly cleans): add a `strip_preamble_before_first_question(body)` helper in `codebus-core/src/verb/quiz.rs` that finds the first `## Q<n>.` heading (even when it is not at a line start because preamble was glued in front of it) and returns the body from that heading onward, trimmed, guaranteeing `## Q1.` starts a line. Apply it in `run_quiz_generate` immediately after `strip_code_fence` so the cleaned `quiz_md` is the single source for both GUI answering and CLI persistence. If no `## Q` heading exists at all, return the body unchanged (the existing downstream "no well-formed questions" handling owns that case — do not mask it here).

Scope boundary: caller-side body cleanup in `codebus-core/src/verb/quiz.rs`; no change to the SKILL agent contract, spawn invocation, frontmatter injection, or IPC. The SKILL still prohibits any preamble.

### D8: Quiz tab renders the live agent stream during plan/generate (manual-e2e defect #5)

Manual e2e: running `+ New quiz` showed only the final no-match message — no live agent activity, unlike the goal flow. `app-workspace` Quiz Tab Plan-Confirm-Generate Flow already requires "a plan spawn whose agent activity is rendered live via the existing agent stream rendering" / "the generate spawn with live activity rendering". Grounded: `QuizTab.tsx` subscribes only to the `quiz-plan-terminal` and `quiz-generate-terminal` channels and renders static text (`Planning quiz scope…` / `Generating questions…`) for the `planning`/`generating` phases; it never subscribes to the `quiz-stream` channel even though the backend (`ipc/quiz.rs`) already emits each `VerbEvent` as a `QuizStreamPayload` there. So the live activity is dropped on the floor — a pure frontend compliance gap, no backend change needed.

Resolution: in `QuizTab.tsx`, subscribe to `quiz-stream` for the active run, accumulate the streamed `VerbEvent`s into a `liveEvents` state (reset when a new plan/generate starts and on confirm), and render them in the `planning` and `generating` phases through the existing agent stream rendering — `foldTimeline` + `ThoughtItem`/`ActivityStreamItem` from `./ActivityStreamItem` (the same pipeline `QuizGenerationLog`/`RunDetailDone` use), reusing it rather than reimplementing. The static labels MAY remain as a heading above the live stream but no longer stand alone. Listener lifecycle mirrors the existing terminal-channel handling (stored in the same unlisten ref pattern, cleaned up on terminal/unmount).

Scope boundary: frontend `QuizTab.tsx` only (plus its test); reuse the existing stream-render components; no backend, IPC, spawn, or persist change. The `quiz-stream` payload contract is unchanged.

### D9: View-generation-log moves into the attempt detail view as a centered modal (manual-e2e UX feedback #6)

Manual e2e UX feedback: the current view-generation-log (defect #2 / D2) lives on each history row and inline-expands a panel in the list — the user finds this poor and wants it reached *after entering a specific quiz attempt*, shown as a floating centered modal. Decision (user-confirmed): (a) placement = inside the opened attempt detail view (`quiz-attempt-view`), not the history row; (b) presentation = a centered modal with backdrop, dismiss returns to the attempt view, consistent with the app's existing modal pattern.

Resolution: reuse the existing `Dialog` primitive (`components/ui/dialog.tsx`, the same one `SettingsModal` uses) — do not hand-roll an overlay. `openAttempt` keeps the selected attempt's `events_log` (store the attempt meta, not just its markdown) so the attempt view can decide whether to show the affordance. In `quiz-attempt-view`, when the opened attempt's `events_log` is non-null, render a "看過程" button; clicking it opens a `Dialog` whose body is the existing `QuizGenerationLog` (unchanged — it already renders the timeline via the shared `foldTimeline`/stream items). Remove the row-level `quiz-view-log` button and the inline `quiz-view-log-panel` from the history list entirely. Modal open state is local component state; closing returns to the attempt view (no phase change). Attempts with a null `events_log` show no affordance.

Scope boundary: frontend `QuizTab.tsx` only (relocate affordance + wrap `QuizGenerationLog` in `Dialog`) plus its test; `QuizGenerationLog`, IPC, backend, spawn, persist all unchanged. This supersedes the row-level affordance described in D2 — D2's `read_quiz_events` IPC and `QuizGenerationLog` component remain; only the entry point and presentation change.

### D10: `+ New quiz` is hidden once inside a quiz (manual-e2e defect #7)

Manual e2e: inside the answering view the top `+ New quiz` button overlapped the window min/max/close controls. The shared Quiz-tab header already reserves `pr-[160px]` (D4/defect #1) so the collision is layout-context dependent and hard to reproduce from code alone — but the user's sharper observation is the real fix: `+ New quiz` has no business being shown while you are *inside a quiz* at all. It belongs only to the browse/compose context. Showing it during a quiz flow is both the collision's trigger surface and a UX wart.

Resolution: render the header `+ New quiz` button only when `phase` is `history` or `idle` (the history list and the topic-input compose screen); do not render it for `planning`, `confirm`, `generating`, `ready`, `no_match`, `error`, or `attempt`. This replaces the previous "always rendered, `disabled` outside a few phases" approach with conditional rendering, which also removes the window-controls overlap in the answering view (defect #7) since the button is simply absent there. The `data-tauri-drag-region` header bar itself stays in all phases (drag affordance + the `pr-[160px]` reservation are unchanged); only the button's presence becomes phase-gated.

Scope boundary: frontend `QuizTab.tsx` only (conditional render of one button) plus its test. No change to the header layout, IPC, backend, spawn, or persist.

## Risks / Trade-offs

- Manual acceptance may surface defects larger than the four known gaps, expanding scope. Mitigation: ingest as discrete tasks; if a defect is genuinely a new feature rather than a compliance gap, it is split out rather than absorbed (keeps this container a bug-fix).
- Reusing RunDetailDone internals may require a small refactor to make the replay reusable; the refactor must not change run-detail behavior — guarded by the existing run-detail tests staying green.

## Implementation Contract

- **pass_threshold**: With settings `app.quiz.pass_threshold = T`, a finished quiz with score `s%` shows passing iff `s >= T`; absent key ⇒ T defaults to 80. Verified by a test that drives the settings store value (not a component prop) for T=80 (4/5 pass) and T=90 (4/5 fail).
- **view-generation-log**: A new `read_quiz_events(vault_path, path)` IPC reads the attempt's events.jsonl into `Vec<EventEnvelope>` and rejects any path outside the vault `.codebus/` tree with `AppError::Invalid { field: "path" }`. Registered in `ipc/mod.rs`; registry count 22 → 23 with the `ipc/mod.rs` count/name tests and `tests/keyring_ipc.rs` count assertion updated to 23. Activating the affordance on an attempt row renders that attempt's events through the existing `foldTimeline` + `ThoughtItem`/`ActivityStreamItem` rendering — Thought/ToolUse/result items present, not a path string. Verified by a Rust test for `read_quiz_events` (parse + out-of-tree rejection) and a frontend test asserting stream-rendered items.
- **events_log**: After a generate spawn, the persisted quiz markdown `events_log` points to an on-disk file containing that spawn's events. Verified by a CLI mock-claude end-to-end assertion.
- **hygiene**: `npx tsc --noEmit` and eslint exit clean for codebus-app with no quiz-attributable errors.
- **manual acceptance**: All five Windows checklist areas executed; each item passes or its defect is ingested into this change and fixed; roadmap registry Windows note made accurate.
- **Out of scope**: macOS/Linux acceptance; new quiz runtime behavior or features beyond the read-only `read_quiz_events` IPC. (The IPC registry count delta 22 → 23 IS in scope — it is the minimum contract change the view-log timeline requires.)
