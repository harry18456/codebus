## Why

`v3-app-quiz` shipped quiz as: generate → answer in one in-memory sitting → summary → an immutable timestamped markdown record. Manual e2e (during `fix-app-quiz`) surfaced that this model is poor in use: opening a past attempt shows raw markdown with answers exposed, there is no notion of "this quiz has N questions / I answered X", you cannot resume after closing the app, and you cannot review which questions you got wrong. The generated quiz markdown contains the questions, the correct answers, and explanations — but NOT what the user picked, and that fact is not derivable from anywhere. To give the history list real progress, support resume, and replace the raw-markdown attempt view with a proper review, the user's answers must be persisted. This was decided in `docs/2026-05-18-quiz-progress-redesign-discussion.md`.

## What Changes

- **Per-attempt progress sidecar**: each generated attempt `<vault>/.codebus/quiz/<slug>/<quiz_id>.md` (immutable, unchanged) gains an optional sibling `<vault>/.codebus/quiz/<slug>/<quiz_id>.progress.json` storing only the non-derivable data: the user's answers, a status, and timestamps. `answered` / `correct` / `score` are NOT stored — they are recomputed from `answers` (single source of truth). Absent sidecar = "not started" (0 / N where N is parsed from the markdown).
- **Progress is persisted per submitted question** (atomic temp-then-rename write), so closing or leaving mid-quiz preserves exact resume position.
- **codebus-core owns the sidecar contract** (read/parse with forward-compatible `schema_version` tolerance + atomic write), colocated with `persist_quiz`, so a future CLI surface and the GUI share one source of truth.
- **Two new Tauri IPC commands** (`read_quiz_progress`, `write_quiz_progress`) thin-wrap the core unit with the same vault `.codebus/` containment guard as `read_quiz_attempt`. IPC registry total goes 23 → 25.
- **GUI history list** shows a per-attempt status badge — not-started `0/N`, in-progress `X/N`, completed `X/N · score% · pass|fail` — and routes on click: not-started/in-progress → answering (resuming at the first unanswered question), completed → a read-only Review view (each question with the user's choice vs the correct answer + explanation) that **replaces the current raw `<pre>` markdown attempt view**.
- **Answering view persists** each submission to the sidecar and resumes from saved progress.
- **Retake semantics split into two explicit affordances**: "Redo this" (same generated questions, reset that attempt's sidecar) lives in Review/Summary; "Retry-new" remains `+ New quiz` on the same topic producing a fresh generated attempt (unchanged `v3-app-quiz` retry = plain re-spawn).
- **Answer-explanation citations become navigable wikilinks (supersedes the back-to-wiki button — manual-e2e finding)**: in the answering view (after submit) and in the Review view, each question's `## Explanation` `[[slug]]` citations render as clickable wikilinks (reusing the existing `WikilinkLink` renderer + the workspace's wiki page index), shown on **both correct and incorrect** submissions; activating one navigates the workspace to that wiki page (reusing the same tab-switch + page-load path as `Workspace.onSelectPage`). This **replaces** the spec's incorrect-answer-only `[← Back to wiki page]` affordance, which was never wired in `QuizTab` (dead button) and was ambiguous in the Goal flow (a quiz spans multiple planned pages); per-question citations give precise, unambiguous navigation for both Goal and Page flows.
- **Resume restores the exact position the user left (precise cursor)**: the QuizProgress sidecar gains an optional `cursor` ({ `q`, `revealed` }) recording the currently-viewed question and whether it was already submitted. It is written on every submission AND on every Next, so reopening an attempt restores precisely where the user was — a blank next question if they had advanced, or the submitted question (choice + verdict + explanation) if they had not. This supersedes the earlier "last answered, revealed" approximation and its one-extra-Next trade-off (manual-e2e feedback: returning to the previous already-answered question is confusing). The `cursor` field is **optional and backward-compatible**: a sidecar without it (legacy / written by a prior build) is tolerated and falls back to the "last answered, revealed" behavior; no `schema_version` bump is required (serde field-default + unknown-key tolerance already cover it).
- **Plan-confirm screen clarity + i18n**: the scope-confirm view's description states the quiz will be generated from the listed wiki pages, the page list is presented (not a bare path list), the revise control is relabeled from the ambiguous `改` to `重新規劃` (behavior unchanged — it returns to the topic-input/idle view to re-plan), and the confirm-view description + the `重新規劃`/`確認` button labels are routed through the existing i18n system (`useT()`/`messages.ts`, `en` + `zh-tw` keys) instead of the current hardcoded English string.

## Non-Goals

- Not changing the generated quiz markdown format or the "attempt markdown is immutable, retry never overwrites" contract from `v3-app-quiz` — the sidecar is additive and separate.
- Not changing plan/generate/`+ New quiz`/`[Quiz me on this]`/live-stream/`看過程` modal behavior — those are settled by `fix-app-quiz` and remain.
- Not adding spaced repetition, history charts, cross-attempt aggregate stats, or a CLI answering UI (CLI quiz remains generate-only).
- Not storing `answered`/`correct`/`score` as authoritative fields (recomputed from `answers` to avoid internal contradiction).
- A from-scratch human GUI re-sweep of the whole quiz tab is owned by this change's own acceptance, not reopening archived changes.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `quiz`: add the per-attempt progress sidecar storage contract (schema, atomic per-submit write, recompute-not-store derived fields, absent = not-started) alongside the existing immutable attempt markdown.
- `app-workspace`: history list shows per-attempt status/progress and routes by status (resume vs review); the opened-attempt view becomes a read-only Review (replacing raw markdown) with "Redo this"; answering persists progress and resumes (restoring the last-answered question in its submitted state, not the next blank one); the answering and Review explanations render `[[slug]]` citations as navigable wikilinks (replacing the removed incorrect-answer-only `[← Back to wiki page]` affordance); the plan-confirm view states it will generate from the listed pages, relabels the revise control `改` → `重新規劃`, and routes its copy through i18n; the quiz Tauri IPC requirement goes 6 → 8 commands and the registry total 23 → 25 (the `app-shell` IPC Command Registry requirement is foundation-scoped — "no other command by that change" — and is NOT modified here; the running total is owned by the `app-workspace` quiz-IPC requirement, consistent with how `fix-app-quiz` tracked 22 → 23).

## Impact

- Affected specs: `quiz`, `app-workspace`
- Affected code:
  - New:
    - codebus-core/src/verb/quiz_progress.rs
    - codebus-app/src/components/workspace/QuizReview.tsx
    - codebus-app/src/components/workspace/QuizReview.test.tsx
    - codebus-app/src/lib/quiz-parse.test.ts (new — created by task 6.1 RED)
  - Modified:
    - codebus-core/src/verb/quiz.rs
    - codebus-core/src/verb/mod.rs
    - codebus-app/src-tauri/src/ipc/quiz.rs
    - codebus-app/src-tauri/src/ipc/mod.rs
    - codebus-app/src-tauri/tests/keyring_ipc.rs
    - codebus-app/src/lib/ipc.ts
    - codebus-app/src/lib/quiz-parse.ts (extract per-question citation slugs from each `## Explanation`)
    - codebus-app/src/components/workspace/QuizTab.tsx (confirm-view copy/relabel/i18n; thread the wiki-navigate handler down)
    - codebus-app/src/components/workspace/QuizTab.test.tsx
    - codebus-app/src/components/workspace/QuizAnswering.tsx (render explanation wikilinks; remove back-to-wiki button; resume = last-answered revealed)
    - codebus-app/src/components/workspace/QuizAnswering.test.tsx
    - codebus-app/src/components/workspace/QuizReview.tsx (render explanation wikilinks)
    - codebus-app/src/components/workspace/QuizReview.test.tsx
    - codebus-app/src/components/workspace/Workspace.tsx (provide quiz→wiki-page navigate handler, reusing the `onSelectPage` tab-switch + page-load path)
    - codebus-app/src/i18n/messages.ts (new confirm-view + button-label keys, `en` + `zh-tw`)
  - Reused (not modified): codebus-app/src/lib/milkdown-wikilink.tsx (`WikilinkLink`)
  - Removed: the `[← Back to wiki page]` button + `onBackToWiki` prop in `QuizAnswering.tsx` (superseded by explanation wikilinks)
