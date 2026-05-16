## Why

The Workspace `Quiz` tab currently renders only a placeholder ("Coming soon — quiz flow ships in v3-app-quiz"). Codebus's core differentiator is "author self-validates understanding via auto-generated quizzes" — without the quiz flow the product loop (explore → build wiki → self-validate) is incomplete. This change ships the quiz capability as the standard codebus three-piece pattern (verb library + skill bundle + CLI thin wrapper) so both `codebus quiz` CLI users and the GUI consume one validated surface, consistent with goal/query/fix/chat.

## What Changes

- **New `codebus quiz` verb**: a read-only two-shot flow. Spawn 1 (plan) takes a free-text topic and emits a structured wiki-page scope; Spawn 2 (generate) reads the planned pages and emits a quiz markdown document (N questions, 4 choices each, answer + explanation). Answer grading is client-side (no LLM) by comparing the user's choice to the `Answer` field.
- **New `codebus-quiz` skill bundle**: read-only invariant scoped to `wiki/` only (MUST NOT read `raw/`), two-mode routing (`plan:` / `generate:`), `[CODEBUS_QUIZ_SCOPE]` / `[CODEBUS_QUIZ_NO_MATCH]` line markers, quiz-md schema rules, Language Override.
- **`codebus_core::verb::quiz` library**: `run_quiz(QuizOptions { scope, question_count }, on_event, cancel) -> QuizReport`. `scope` is `Page { target }` (from wiki-preview trigger) or `Goal { text }` (from sidebar goal-input). `question_count` is caller-injected (NOT read from config by the library).
- **GUI Quiz flow**: sidebar `+ New quiz` → goal-text input → plan spawn (live stream) → AI-planned scope shown for user confirm → generate spawn (live stream) → one-question-per-screen answering → summary → history list. `[Quiz me on this]` from wiki preview skips planning and uses target page + 1-hop scope. Both triggers converge downstream.
- **BREAKING — config migration**: `app.quiz.default_length` moves OUT of the `app.*` namespace into a new shared `quiz.*` namespace (CLI + app both read it), superseding part of app-shell's `AppConfig Namespace Isolation` requirement. `app.quiz.pass_threshold` stays in `app.*` (client-side grading UI concept; CLI quiz has no pass/fail screen).
- **Storage**: each quiz attempt is one timestamped markdown file under `<vault>/.codebus/quiz/<page-or-topic-slug>/<timestamp>.md` (questions + result combined; retry never overwrites). Generation events persist to events.jsonl via existing run-log infrastructure; quiz history rows expose a `[看過程]` (view-generation-log) affordance.

## Non-Goals

(captured in design.md Goals/Non-Goals)

## Capabilities

### New Capabilities

- `quiz`: the `codebus quiz` verb behavior contract — two-shot plan/generate flow, `wiki/`-only read scope (raw/ forbidden), scope/no-match line markers, quiz-md output schema, caller-injected question count, shared `quiz.*` config namespace, retry-is-plain-respawn semantics, per-attempt timestamped storage.

### Modified Capabilities

- `verb-library`: add a fifth `quiz` sub-module exporting `run_quiz` + `QuizOptions`/`QuizReport`/`QuizScope`; extend the documented module surface (currently fixed at four sub-modules).
- `cli`: register an eighth `quiz` subcommand; define its sandbox flags, `--count` flag (overrides shared `quiz.default_length`), exit-code and read-only (no auto-commit) policy.
- `skill-bundles`: add a fifth `codebus-quiz` bundle at the vault-internal and repo-level skill locations.
- `app-workspace`: replace the Quiz tab placeholder requirement with the real quiz flow (prep/confirm, one-question-per-screen, summary, history) and the two trigger points.
- `app-shell`: supersede `AppConfig Namespace Isolation` so `default_length` is no longer in `app.*`; `app.*` retains only `app.quiz.pass_threshold`.

(The quiz plan/generate spawns reuse the existing agent stream rendering capability as-is — no requirement change there, so it is intentionally not listed as a modified capability.)

## Impact

- Affected specs: `quiz` (new); `verb-library`, `cli`, `skill-bundles`, `app-workspace`, `app-shell` (modified)
- Affected code:
  - New:
    - codebus-core/src/verb/quiz.rs
    - codebus-core/src/config/quiz.rs
    - codebus-cli/src/commands/quiz.rs
    - codebus-app/src/components/workspace/QuizPrep.tsx
    - codebus-app/src/components/workspace/QuizQuestion.tsx
    - codebus-app/src/components/workspace/QuizSummary.tsx
    - codebus-app/src/components/workspace/QuizHistory.tsx
    - .codebus/.claude/skills/codebus-quiz/SKILL.md
    - .claude/skills/codebus-quiz/SKILL.md
  - Modified:
    - codebus-core/src/verb/mod.rs
    - codebus-core/src/verb/event.rs
    - codebus-core/src/config/mod.rs
    - codebus-cli/src/commands/mod.rs
    - codebus-app/src-tauri/src/config.rs
    - codebus-app/src/components/workspace/QuizTab.tsx
    - codebus-app/src/store/settings.ts
    - codebus-app/src/components/settings/SettingsModal.tsx
    - openspec/specs/app-shell/spec.md
    - openspec/specs/app-workspace/spec.md
    - openspec/specs/verb-library/spec.md
    - openspec/specs/cli/spec.md
    - openspec/specs/skill-bundles/spec.md
  - Removed: (none)
