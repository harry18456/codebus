## Context

`v3-app-quiz` persists each generated attempt as an immutable timestamped markdown (`<vault>/.codebus/quiz/<slug>/<quiz_id>.md`: caller frontmatter + `## Q/## Answer/## Explanation` body). The user's selected answers are not in that file and are not derivable from anywhere. `QuizAnswering` keeps answer state in React `useState` only — lost on unmount. History opens the raw markdown in a `<pre>`. The redesign and its UX decisions were converged in `docs/2026-05-18-quiz-progress-redesign-discussion.md` (read it for the full rationale and the rejected alternatives: write-back-to-md breaks the archived immutability contract; frontend-only loses on data clear / not vault-portable).

`.spectra.yaml`: tdd, audit, parallel_tasks enabled; locale tw.

## Goals / Non-Goals

### Goals

- Persist the user's per-attempt answers so the history list shows real progress, answering resumes after app restart, and a completed attempt opens a proper Review (not raw markdown).
- Keep the generated attempt markdown immutable (preserve the `v3-app-quiz` storage/retry contract); the sidecar is additive and separate.
- One source of truth for the sidecar (codebus-core), shared by GUI (and a possible future CLI).

### Non-Goals

- No change to generated-markdown format, plan/generate flow, `+ New quiz`/`[Quiz me on this]`, live stream, or the `看過程` modal (settled by `fix-app-quiz`).
- No spaced repetition / charts / cross-attempt aggregates / CLI answering UI.
- No authoritative `answered`/`score` fields (recomputed).

## Decisions

### D1: Immutable md + mutable progress sidecar

`<slug>/<quiz_id>.md` stays immutable. A sibling `<slug>/<quiz_id>.progress.json` holds ONLY: `schema_version` (int), `answers` (ordered `[{ q: 1-based int, selected: "A"|"B"|"C"|"D", correct: bool }]`), `status` (`in_progress` | `completed`), `started_at`, `completed_at` (RFC3339; `completed_at` null until completed). `total`, `answered_count`, `correct_count`, `score`, pass/fail are DERIVED at read time (not stored) — single source of truth, no contradictory fields. Absent sidecar ⇒ not-started (answered 0; `total` = parsed from the markdown body's `## Q` count). Rejected: write-back into the attempt md (breaks archived `quiz` immutability contract); frontend-only (lost on data clear, not vault-portable, CLI-invisible).

### D2: codebus-core owns the sidecar; GUI via two thin IPC commands

A new `codebus-core/src/verb/quiz_progress.rs` owns: the `QuizProgress` struct + serde, forward-compatible parse (unknown keys ignored, missing optional fields defaulted, malformed file treated as not-started rather than panicking — mirrors codebus config tolerance), and an atomic write (serialize to `<file>.tmp` then rename over `<file>.progress.json`). `codebus-app/src-tauri/src/ipc/quiz.rs` adds `read_quiz_progress(vault_path, path)` and `write_quiz_progress(vault_path, path, progress)` that resolve `path` under the vault `.codebus/` tree (reuse the exact `read_quiz_attempt` containment guard strength — audit Scoundrel: no unbounded path read/write) and delegate to the core unit. Registered in `ipc/mod.rs`; `REGISTERED_COMMANDS` and the `ipc/mod.rs` count tests + `tests/keyring_ipc.rs` go 23 → 25. Typed `readQuizProgress`/`writeQuizProgress` wrappers in `codebus-app/src/lib/ipc.ts`.

### D3: Per-submit atomic persistence + resume

On each answer submission the answering view computes the updated `answers` and calls `write_quiz_progress` (status `in_progress`; on the final question status `completed` + `completed_at`). Write is atomic (tmp+rename) so a crash mid-write cannot corrupt the sidecar. Opening an attempt loads the sidecar: completed ⇒ Review; not-started ⇒ answering at question 1 (blank).

**Resume point (revised twice — final: precise cursor; manual-e2e feedback):** the sidecar gains an optional `cursor: { q: 1-based int, revealed: bool }` recording the question the user is currently viewing and whether it was already submitted. `cursor` is written on **every submission** (`{ q: current, revealed: true }`) AND on **every Next** (`{ q: nextQ, revealed: false }`), so reopening restores the user's exact position: a blank next question if they had pressed Next, or the submitted question (stored `selected` + verdict + explanation with D6 wikilinks) if they had not. This **supersedes** the interim "last answered, revealed" approximation and its accepted one-extra-Next trade-off (real use showed returning to the already-answered previous question is confusing). Backward/forward compatible: `cursor` is `#[serde(default)]` Optional; a sidecar **without** it (legacy or prior build) falls back to the interim rule — restore `max(q)` in `answers`, revealed. No `schema_version` bump (serde field-default + unknown-key tolerance already cover absent/extra fields per the existing D1/D2 contract). Persisting on Next means `QuizAnswering` calls the persist callback on Next too (answers unchanged, status still `in_progress`, cursor advanced). not-started (no `answers`, no `cursor`) ⇒ Q1 blank; completed ⇒ Review (cursor irrelevant — routed by QuizTab).

### D4: History routing by derived status; Review replaces raw md

`list_quiz_attempts` is unchanged (filesystem scan of `*.md`); the GUI, per row, calls `read_quiz_progress` to derive status. Badge: not-started `○ 0/N`, in-progress `⏵ X/N`, completed `✓ X/N · score% · pass|fail` (pass/fail via `app.quiz.pass_threshold` from the settings store — reuse `fix-app-quiz` D1 wiring). Click routes by status. The opened-attempt view (`quiz-attempt-view`) is replaced: instead of `<pre>{attemptMd}</pre>`, a `QuizReview` component renders each question with the user's choice vs the correct answer + explanation (read-only); it carries `[重做此份]` (reset this attempt's sidecar, re-enter answering with the same questions) and the existing `看過程` modal affordance. Not-started/in-progress rows enter `QuizAnswering` instead.

### D5: Retake = two explicit affordances

"Redo this" (in Review/Summary): same generated questions, delete/zero this attempt's sidecar, re-enter answering. "Retry-new": `+ New quiz` same topic → fresh generate → new `<quiz_id>.md` (unchanged `v3-app-quiz` D5 retry = plain re-spawn). The two are visually distinct controls; "Redo this" never re-spawns the agent, "Retry-new" always does.

### D6: Explanation citations are navigable wikilinks (supersedes the back-to-wiki button)

The `codebus-quiz` SKILL contractually requires every question's `## Explanation` to cite its source via `[[slug]]` wikilink syntax (verified in real generated attempts: each question carries ≥1 `[[slug]]`). Today `QuizAnswering` renders the explanation as plain text, so citations show as literal `[[slug]]`, and a separate `[← Back to wiki page]` button (spec'd for incorrect answers only) is never wired in `QuizTab` (dead) and is ambiguous in the Goal flow (which of the multiple planned pages?).

Decision: render each `## Explanation`'s `[[slug]]` citations as clickable wikilinks in **both** `QuizAnswering` (post-submit, correct **and** incorrect) and `QuizReview`, and **remove** the `[← Back to wiki page]` button + `onBackToWiki` prop. `quiz-parse.ts` `parseQuiz` extracts a per-question `sources: string[]` (every `[[slug]]` in that question's explanation, in order, de-duplicated). Rendering mirrors the app's primary wiki-link presentation in `WikiPreview` (NOT the v1 bracketed `WikilinkLink`, which renders the raw `[[slug]]` and is inconsistent with the rest of the app): the displayed link text is the page's frontmatter title (`pages[slug].title`), falling back to the bare `slug` only when the page is unknown, and **never** the `[[ ]]`-bracketed form. Resolvable citations (slug present in the workspace wiki page index, from `useWikiStore`) render as a clickable anchor; unresolvable ones render as dimmed plain text (title-or-slug), matching `WikiPreview`'s not-found presentation. Each rendered citation keeps `data-testid="wikilink-<slug>"` for test/locator stability. Activating a resolvable citation calls an `onOpenWikiPage(slug)` handler threaded `Workspace → QuizTab → QuizAnswering`/`QuizReview`; `Workspace` implements it by the same path as `onSelectPage` (switch `activeTab` to `wiki` + load the page via `useWikiStore`). Per-question citations make navigation precise and unambiguous for both Goal and Page flows. Rejected: keep the button and also add links (the button's "which page" is undefined in Goal flow; two affordances for the same intent is redundant) — this is a deliberate spec change to the `Quiz Answering and Summary` requirement.

### D7: Plan-confirm view clarity + i18n

The scope-confirm view currently shows a hardcoded English line (`Planned scope — confirm to generate the quiz:`), a bare path list, and a `改` button whose label is ambiguous (it returns to the topic-input/idle view to re-plan — it does **not** regenerate with the same scope). Decision: (1) the description states the quiz will be generated from the listed wiki pages; (2) the revise control is relabeled `改` → `重新規劃` with **behavior unchanged** (still `reset()` → idle/topic-input; the label is corrected because no questions exist yet at confirm time, so `重新出題` would be wrong); (3) the confirm-view description and the `重新規劃`/`確認` button labels are routed through the existing i18n system (`useT()` + new `messages.ts` keys, `en` + `zh-tw`) rather than a hardcoded string. The confirm flow/state machine and the generate spawn are otherwise unchanged.

## Risks / Trade-offs

- Two files per attempt: history scan now does N extra `read_quiz_progress` calls. Acceptable (small JSON, < a few KB; same order as the existing per-row `read_quiz_attempt`). If it ever matters, batching is a later optimization, not now (YAGNI).
- Sidecar/markdown divergence (e.g., md regenerated? it never is — immutable; `total` mismatch only if a sidecar is hand-edited): treat malformed/over-long `answers` defensively (clamp to parsed `total`, drop `q` out of range) rather than trust blindly — audit Confused-Developer lens.
- Atomic write on Windows: rename-over-existing must use a replace that overwrites (std `fs::rename` replaces on Windows for files) — covered by a test that overwrites an existing sidecar.

## Implementation Contract

- **Sidecar schema & tolerance**: `QuizProgress { schema_version, answers: Vec<{q,selected,correct}>, status, started_at, completed_at }`. Parsing a missing file ⇒ `None` (not error); a malformed/garbage file ⇒ treated as not-started (logged, not panic); unknown JSON keys ignored; future `schema_version` higher than known ⇒ best-effort read of known fields. Verified by core unit tests (missing→none, garbage→not-started, round-trip, unknown-key-ignored).
- **Atomic write**: `write_quiz_progress` writes `<dir>/<quiz_id>.progress.json` via temp file in the same dir + rename; overwriting an existing sidecar succeeds. Verified by a core test that writes twice and asserts final content + no `.tmp` left.
- **Containment**: both IPC commands reject any `path` not resolving under `<vault>/.codebus/` with `AppError::Invalid { field: "path" }` (same strength as `read_quiz_attempt`). Verified by tauri tests (out-of-tree rejected; in-tree ok).
- **Registry**: `REGISTERED_COMMANDS` length 25; names include `read_quiz_progress` + `read_quiz_attempt` etc.; `ipc/mod.rs` count test + `tests/keyring_ipc.rs` assert 25. Verified by those tests.
- **Derived status**: given a sidecar with k answers of n total: k<n & status in_progress ⇒ badge `X/N` in-progress; k==n & status completed ⇒ `X/N · score% · pass|fail` where score = correct/n, pass = score*100 >= app.quiz.pass_threshold; no sidecar ⇒ `0/N` not-started. Verified by frontend tests driving each case.
- **Resume (final, D3 precise cursor)**: `QuizProgress` carries optional `cursor { q, revealed }`. Opening an in-progress attempt with a `cursor` restores exactly `q`/`revealed` (revealed ⇒ show stored `selected` + verdict + explanation; not revealed ⇒ blank question `q`). `cursor` is written on every submit and every Next. A sidecar WITHOUT `cursor` (legacy) falls back to `max(q)` in `answers`, revealed. Optional `#[serde(default)]`; no schema_version bump. Verified by: a core unit test (round-trip with `cursor`; a sidecar JSON omitting `cursor` parses with `cursor: None`); a QuizAnswering test (cursor `{q:4,revealed:false}` over answers Q1-3 ⇒ Q4 blank; cursor `{q:3,revealed:true}` ⇒ Q3 shown revealed with stored choice; absent cursor ⇒ legacy last-answered).
- **Explanation wikilinks (D6)**: `parseQuiz` exposes per-question `sources: string[]` = the `[[slug]]` citations in that question's explanation (ordered, de-duplicated). `QuizAnswering` (post-submit, correct AND incorrect) and `QuizReview` render the explanation's citations the same way `WikiPreview` does — displayed text is `pages[slug].title` (fallback `slug`), no `[[ ]]` brackets; resolvable = clickable anchor, unresolvable = dimmed plain text — each keeping `data-testid="wikilink-<slug>"`. Activating a resolvable one invokes the threaded `onOpenWikiPage(slug)` which navigates the workspace to that wiki page (same path as `Workspace.onSelectPage`). The `[← Back to wiki page]` button + `onBackToWiki` prop are removed. Verified by: a `quiz-parse` unit test (explanation `[[a]] ... [[b]]` ⇒ `sources: ["a","b"]`); a QuizAnswering test (submit correct → explanation renders the citation as the page **title** with no `[[ ]]`, click → `onOpenWikiPage` called with that slug; no `quiz-back-to-wiki` testid present); a QuizReview test (per-question explanation renders the title-text wikilink).
- **Review replaces raw md**: opening a completed attempt renders `QuizReview` (per-question user-choice vs correct + explanation, `[重做此份]`, `看過程`) and NOT a raw `<pre>` of the markdown. Verified by a QuizTab test asserting QuizReview present and no raw-md `<pre>` testid.
- **Redo this**: activating `[重做此份]` clears that attempt's sidecar (write not-started / delete) and enters `QuizAnswering` from question 1 with the same parsed questions; does not spawn an agent. Verified by a frontend test asserting no `spawn_quiz_*` invoked and answering restarts.
- **Plan-confirm view (D7)**: the confirm view's description + the `重新規劃`/`確認` button labels come from i18n keys (present in `messages.ts` `en` and `zh-tw`); the revise button reads `重新規劃` and still calls `reset()` (returns to idle/topic-input — behavior unchanged). Verified by a QuizTab test asserting the relabeled control, the from-i18n description text, and that activating it returns to the topic-input view (no `spawn_quiz_*`).
- **In scope (this ingest)**: explanation-citation wikilink rendering + quiz→wiki navigation wiring; removal of the back-to-wiki button + spec requirement change; revised resume point; plan-confirm copy/relabel/i18n. **Out of scope**: generated-md format, plan/generate/`+ New quiz`/`[Quiz me on this]`/live-stream/`看過程`-internals, the confirm-flow state machine itself, QuizProgress sidecar schema (unchanged), spaced repetition, CLI answering, macOS/Linux manual acceptance (deferred to `v3-app-polish-ship` per roadmap registry).

## Migration Plan

No migration. Pre-existing attempts simply have no sidecar ⇒ shown as not-started; opening one with no sidecar starts a fresh answering session (and then begins persisting). No data format version bump; sidecars are created lazily on first answer.
