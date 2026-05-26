# quiz Specification

## Purpose

TBD - created by archiving change 'v3-app-quiz'. Update Purpose after archive.

## Requirements

### Requirement: Quiz Verb Two-Shot Flow

The system SHALL provide a `quiz` verb that produces a multiple-choice quiz from wiki pages via at most two agent spawns, split across two separately-invokable library functions so a caller (the GUI) can interpose a confirmation between them. The Goal flow SHALL call `run_quiz_plan` (a **plan** spawn) and then, only when the plan returned a scope, `run_quiz_generate` (a **generate** spawn). The Page flow (wiki-preview `[Quiz me on this]`) SHALL call `run_quiz_generate` directly with `pages = [target]`, skipping the plan spawn; the SKILL expands the target to its one-hop wikilinked pages.

The plan spawn SHALL take a free-text topic and emit, as the first line of its response, either `[CODEBUS_QUIZ_SCOPE] <wiki-path>, <wiki-path>, ...` (2–5 vault-relative `wiki/` paths, most relevant first) when matching pages exist, or `[CODEBUS_QUIZ_NO_MATCH] <reason>` when no wiki page covers the topic. The generate spawn SHALL emit a quiz markdown document body (no agent-authored frontmatter).

The caller's plan-marker parser SHALL tolerantly recover the marker rather than hard-failing on minor agent deviations, mirroring the tolerant leading/trailing code-fence handling and the same-line preamble tolerance already applied to the generate body: it SHALL strip a leading Markdown code fence, and it SHALL accept the first line that **contains** `[CODEBUS_QUIZ_SCOPE]` or `[CODEBUS_QUIZ_NO_MATCH]` — taking the payload after the first marker occurrence — even when the agent emitted preamble lines before it OR glued a preamble sentence onto the same line as the marker with no intervening newline. The SKILL still instructs the agent to emit the marker as the very first line; tolerant recovery is a caller-side robustness measure, not a relaxation of the agent-side contract. When no marker is recoverable from the spawn output, `run_quiz_plan` SHALL return an error whose message includes a truncated head (at most 200 characters) of the actual spawn output, so the failure is diagnosable rather than opaque.

#### Scenario: Goal flow runs plan then (on scope) generate

- **WHEN** `run_quiz_plan` is invoked with `QuizPlanOptions { topic }` and the topic matches wiki pages, and the caller then invokes `run_quiz_generate` with the returned pages
- **THEN** the plan spawn SHALL emit `[CODEBUS_QUIZ_SCOPE]` with 2–5 `wiki/` paths AND `run_quiz_plan` SHALL return `QuizPlanOutcome::Scope(pages)` AND the subsequent `run_quiz_generate` SHALL emit the quiz markdown body for those pages

#### Scenario: Goal flow with no matching pages

- **WHEN** `run_quiz_plan` is invoked with `QuizPlanOptions { topic }` and no wiki page covers the topic
- **THEN** the plan spawn SHALL emit `[CODEBUS_QUIZ_NO_MATCH] <reason>` as its first line AND `run_quiz_plan` SHALL return `QuizPlanOutcome::NoMatch(reason)` AND surface a `QuizNoMatch { reason }` lifecycle event AND no generate spawn SHALL run

#### Scenario: No-match reason is specific

- **GIVEN** a vault whose wiki only covers web auth
- **WHEN** the plan spawn receives topic "how to bake sourdough bread"
- **THEN** the first response line SHALL be `[CODEBUS_QUIZ_NO_MATCH] vault only covers web auth; no page relates to sourdough bread baking`

#### Scenario: Page flow skips planning

- **WHEN** `run_quiz_generate` is invoked with `QuizGenerateOptions { pages: [target], question_count }` (the wiki-preview Page flow)
- **THEN** no plan spawn SHALL run AND the generate spawn SHALL build its context from `target` plus the pages `target` wikilinks to (one hop)

#### Scenario: Plan marker recovered despite agent preamble

- **GIVEN** a plan spawn whose response is `Sure, here is the scope.\n[CODEBUS_QUIZ_SCOPE] wiki/a.md`
- **WHEN** the caller parses the plan output
- **THEN** `run_quiz_plan` SHALL return `QuizPlanOutcome::Scope(["wiki/a.md"])` (the preamble line is tolerated, not a hard failure)

#### Scenario: Plan marker recovered despite a same-line preamble

- **GIVEN** a plan spawn whose response is `先掃描 wiki 找相關頁面。[CODEBUS_QUIZ_SCOPE] wiki/synthesis/jwt-auth-system.md, wiki/concepts/jwt-pitfalls.md` (a preamble glued onto the marker line with no newline)
- **WHEN** the caller parses the plan output
- **THEN** `run_quiz_plan` SHALL return `QuizPlanOutcome::Scope(["wiki/synthesis/jwt-auth-system.md", "wiki/concepts/jwt-pitfalls.md"])` (the inline preamble is tolerated, not a hard failure)

#### Scenario: Plan marker recovered despite a wrapping code fence

- **GIVEN** a plan spawn whose response is a Markdown code fence wrapping `[CODEBUS_QUIZ_NO_MATCH] vault only covers web auth`
- **WHEN** the caller parses the plan output
- **THEN** `run_quiz_plan` SHALL return `QuizPlanOutcome::NoMatch("vault only covers web auth")`

#### Scenario: Unrecoverable plan output surfaces a diagnostic head

- **WHEN** the plan spawn output contains neither marker on any line
- **THEN** `run_quiz_plan` SHALL return an error whose message includes a truncated head (≤200 chars) of the actual spawn output AND SHALL NOT silently succeed

---
### Requirement: Quiz Read Scope Enforcement

The quiz verb SHALL read only paths under `wiki/` relative to the vault root. The verb SHALL NOT read `raw/`, `raw/code/`, `log/`, or any path escaping the vault root. Enforcement SHALL be by the `codebus-quiz` skill prompt invariant; the system SHALL NOT add a library-level tool_use path interceptor in this change. If the user prompt asks the agent to read source code or `raw/`, the agent SHALL refuse, redirect to the corresponding `wiki/` page, and SHALL NOT issue any tool call whose path resolves under `raw/`.

The events.jsonl record produced for the generate spawn SHALL retain the full `tool_use` trace so that read-scope violations are auditable after the fact.

#### Scenario: Prompt steering toward source code is refused

- **WHEN** the plan spawn receives a topic that explicitly asks to read source code (e.g. "show me the source code of the middleware")
- **THEN** the agent SHALL NOT emit any `tool_use` with a path under `raw/` AND SHALL still emit a `[CODEBUS_QUIZ_SCOPE]` line pointing only at `wiki/` pages

#### Scenario: Violation marker on forced raw access

- **WHEN** the agent determines it is being compelled to access a `raw/` path
- **THEN** it SHALL emit `[CODEBUS_QUIZ_VIOLATION] <attempted-path>` and stop rather than perform the read


<!-- @trace
source: v3-app-quiz
updated: 2026-05-16
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/spike-artifacts/quiz-fixture-vault/manifest.yaml
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/jwt-token-lifecycle.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/index.md
  - docs/spike-artifacts/spike-quiz-7-F5.jsonl
  - codebus-app/src-tauri/src/ipc/quiz.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/config/quiz.rs
  - docs/spike-artifacts/spike-quiz-7-F1.jsonl
  - codebus-app/src-tauri/src/ipc/config.rs
  - docs/2026-05-15-v3-app-quiz-spike-plan.md
  - docs/spike-artifacts/spike-quiz-7-F6.jsonl
  - docs/spike-artifacts/spike-quiz-8-E3.jsonl
  - docs/spike-artifacts/spike-quiz-9-S1.jsonl
  - codebus-core/src/verb/quiz.rs
  - docs/v3-app-roadmap.md
  - codebus-cli/src/commands/mod.rs
  - codebus-core/src/config/claude_code.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run2.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC1.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/user-store.md
  - docs/spike-artifacts/spike-quiz-10-R1-run1.jsonl
  - codebus-app/src-tauri/src/config.rs
  - codebus-app/src/lib/quiz-parse.ts
  - codebus-core/src/skill_bundle/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/wiki/log.md
  - docs/spike-artifacts/spike-quiz-7-F2.jsonl
  - docs/spike-artifacts/spike-quiz-8-E4.jsonl
  - codebus-app/src/components/workspace/QuizAnswering.tsx
  - docs/2026-05-15-v3-app-quiz-discussion.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/session-vs-token.md
  - docs/spike-artifacts/spike-quiz-8-E5.jsonl
  - codebus-cli/src/commands/quiz.rs
  - docs/spike-artifacts/spike-quiz-9-S3.jsonl
  - codebus-core/src/config/mod.rs
  - codebus-core/src/log/events/sink.rs
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - docs/spike-artifacts/spike-quiz-runbook.md
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/verb/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/CLAUDE.md
  - codebus-core/src/verb/event.rs
  - codebus-app/src/components/settings/SettingsModal.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - docs/spike-artifacts/spike-quiz-8-E2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/raw/code/auth.py
  - docs/spike-artifacts/spike-quiz-8-E1.jsonl
  - docs/spike-artifacts/spike-quiz-7-F3.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/.claude/skills/codebus-quiz/SKILL.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/auth-middleware.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/processes/login-flow.md
  - docs/spike-artifacts/spike-quiz-9-S2.jsonl
  - codebus-core/src/vault/source_gitignore.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run3.jsonl
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/spike-artifacts/spike-quiz-7-F4.jsonl
tests:
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-core/tests/vault_init.rs
  - codebus-cli/tests/bins/mock_claude.rs
  - codebus-cli/tests/quiz_flow.rs
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-core/tests/verb_library_surface.rs
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-app/src/components/workspace/QuizAnswering.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: Quiz Markdown Schema and Caller Frontmatter Injection

The generate spawn SHALL emit a markdown body containing exactly `count` question sections. Each question SHALL be a `## Q<i>.` heading (i from 1 to count) followed by exactly four choices labelled `A)` through `D)`, exactly one `## Answer: <A|B|C|D>` line, and exactly one `## Explanation:` line citing a source via `[[slug]]` wikilink syntax. The agent SHALL NOT wrap the whole output in a code fence. The agent SHALL NOT author `quiz_id`, `topic`, `planned_pages`, `generation_token_usage`, or any frontmatter.

The caller SHALL inject frontmatter on persistence: `quiz_id` (real ISO timestamp), `trigger` (`ai_planned` or `wiki_preview`), `topic` (plan topic) or `target_page` (page-scope target), `planned_pages` (list), `generation_token_usage` (from the report), and `events_log` (the generate spawn's events.jsonl path). The caller parser SHALL tolerantly strip a leading/trailing code fence if the agent emits one despite the prohibition. The caller parser SHALL ALSO tolerantly discard any preamble text that precedes the first `## Q1.` question heading — including a preamble that the agent placed on the same line as `## Q1.` — so the cleaned body begins exactly at the first question heading and `## Q1.` starts a line. This mirrors the tolerant code-fence handling: the SKILL still prohibits any preamble; tolerant stripping is a caller-side robustness measure, not a relaxation of the agent contract.

#### Scenario: Question block well-formed

- **WHEN** the generate spawn runs with `count=5`
- **THEN** the body SHALL contain `## Q1.` through `## Q5.`, each with four `A)`–`D)` choices, one `## Answer:` line, and one `## Explanation:` line

#### Scenario: Caller injects authoritative frontmatter

- **WHEN** the caller persists a generated quiz
- **THEN** the persisted file's frontmatter `quiz_id` SHALL be a caller-generated timestamp (not any value the agent emitted) AND `events_log` SHALL point to the events.jsonl file for that generate spawn (from `QuizReport.events_log`)

#### Scenario: Tolerant fence stripping

- **WHEN** the agent emits the body wrapped in a leading and trailing markdown code fence despite the prohibition
- **THEN** the caller SHALL strip the surrounding fence before persisting AND the persisted file SHALL begin with caller frontmatter, not a fence

#### Scenario: Tolerant preamble stripping before the first question

- **GIVEN** a generate spawn body whose first line is `讀取三個指定的 wiki 頁面以產生測驗題目。## Q1. <stem>` (an agent preamble glued onto the first question heading)
- **WHEN** the caller cleans the body before persisting and before parsing
- **THEN** the cleaned body SHALL begin with `## Q1.` at the start of a line AND the preamble text SHALL NOT appear in the persisted file AND the first question SHALL parse correctly


<!-- @trace
source: fix-app-quiz
updated: 2026-05-18
code:
  - codebus-app/src/components/workspace/QuizGenerationLog.tsx
  - codebus-app/src/store/settings.ts
  - codebus-app/src/lib/ipc.ts
  - codebus-core/src/verb/quiz.rs
  - docs/v3-app-roadmap.md
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src-tauri/src/ipc/quiz.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
tests:
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/workspace/QuizGenerationLog.test.tsx
  - codebus-app/src/store/settings.test.ts
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-cli/tests/quiz_flow.rs
-->

---
### Requirement: Quiz Storage Layout and Retry Semantics

Each quiz attempt SHALL be persisted as one file at `<vault>/.codebus/quiz/<slug>/<iso-timestamp>.md`, where `<slug>` is the page slug for `wiki_preview` trigger or the topic slug for `ai_planned` trigger. Persisting an attempt SHALL NOT overwrite or delete any prior attempt file. The generated attempt markdown SHALL remain immutable after persistence. Quiz history SHALL be derived by scanning this directory tree, not by correlating run-log entries.

Each attempt MAY additionally have a sibling progress sidecar at `<vault>/.codebus/quiz/<slug>/<iso-timestamp>.progress.json` recording the user's answering state for that attempt. The sidecar SHALL store ONLY the non-derivable data: `schema_version` (integer), `answers` (ordered list of `{ q: 1-based integer, selected: "A"|"B"|"C"|"D", correct: boolean }`), `status` (`in_progress` or `completed`), `started_at`, `completed_at` (RFC3339; `completed_at` null until completed), and an OPTIONAL `cursor` (`{ q: 1-based integer, revealed: boolean }`) recording the question the user is currently viewing and whether it was already submitted. Derived quantities — total question count, answered count, correct count, score, pass/fail — SHALL NOT be stored in the sidecar; they SHALL be recomputed from `answers` and the attempt markdown so the sidecar has a single source of truth and cannot hold self-contradictory fields. `cursor` is navigation state (not a derived quantity) and MAY be absent: a sidecar without `cursor` SHALL remain valid and SHALL be read by treating the resume position as the last answered question in its submitted state. An absent sidecar SHALL mean the attempt is not started (answered 0; total parsed from the markdown body's `## Q` headings).

The sidecar SHALL be written atomically (write to a temporary file in the same directory, then rename over the target) so an interrupted write cannot corrupt it. Reading the sidecar SHALL be tolerant: a missing file yields the not-started state (not an error); a malformed or unparseable file SHALL be treated as not-started rather than panicking; unknown JSON keys SHALL be ignored; a `schema_version` newer than known SHALL still best-effort read the known fields. The sidecar is additive — it SHALL NOT modify or replace the immutable attempt markdown, and the retry semantics below are unchanged by it.

Retry SHALL be a plain re-invocation of the same flow (Goal: `run_quiz_plan` then `run_quiz_generate`; Page: `run_quiz_generate`) with the same inputs. The system SHALL NOT inject previous question stems as negative context and SHALL NOT guarantee that a retry produces different questions. User-facing surfaces SHALL NOT claim that retry always yields new questions.

#### Scenario: Retry creates a new non-destructive file

- **WHEN** a quiz on the same scope is generated twice
- **THEN** two distinct timestamped files SHALL exist under the same `<slug>` directory AND the earlier file's contents SHALL be unchanged

#### Scenario: Retry questions are not guaranteed distinct

- **WHEN** the same flow is re-invoked with identical inputs
- **THEN** the system SHALL NOT pass any record of the prior questions into the second invocation AND the second quiz MAY repeat questions from the first

#### Scenario: Absent sidecar means not started

- **GIVEN** an attempt markdown with 5 `## Q` headings and no sidecar file
- **WHEN** the attempt's progress is read
- **THEN** the result SHALL be the not-started state with total 5 and answered 0

#### Scenario: Sidecar stores only non-derivable data

- **WHEN** the user has answered 3 of 5 questions and the sidecar is written
- **THEN** the sidecar SHALL contain `answers` with 3 entries, `status: in_progress`, `started_at`, and `completed_at: null` AND SHALL NOT contain stored `answered`, `correct`, `score`, or pass/fail fields

#### Scenario: Malformed sidecar is treated as not started

- **GIVEN** a `*.progress.json` whose contents are not valid progress JSON
- **WHEN** the attempt's progress is read
- **THEN** the read SHALL NOT panic AND SHALL yield the not-started state

#### Scenario: Sidecar write is atomic and non-destructive to markdown

- **WHEN** progress is written twice for the same attempt
- **THEN** the final sidecar SHALL reflect the second write AND no temporary file SHALL remain AND the attempt's `.md` file SHALL be byte-unchanged


<!-- @trace
source: quiz-attempt-progress
updated: 2026-05-19
code:
  - codebus-core/src/verb/quiz.rs
-->

---
### Requirement: Shared Quiz Config Namespace

The system SHALL introduce a shared `quiz.*` namespace in `~/.codebus/config.yaml` containing `quiz.default_length` (integer, 3–10, default 5). Both the `codebus quiz` CLI and the codebus-app SHALL read this key. A missing `quiz.default_length` SHALL resolve to default 5. Neither `run_quiz_plan` nor `run_quiz_generate` SHALL read config; `question_count` SHALL be supplied by the caller to `run_quiz_generate`.

The codebus-app SHALL supply the generate spawn's `question_count` from this shared configuration: it SHALL resolve the value as the shared top-level `quiz.default_length`, falling back to a legacy `app.quiz.default_length` for un-migrated configs, then to 5, clamped to the inclusive 3–10 range. This resolved value SHALL come from the persisted configuration without requiring the Settings modal to have been opened in the session. The app SHALL NOT pass a hardcoded constant question count when a configured value exists.

#### Scenario: Missing key resolves to default

- **WHEN** config is loaded and `quiz.default_length` is absent
- **THEN** the resolved value SHALL be 5

#### Scenario: Library does not read config

- **WHEN** `run_quiz_generate` is invoked
- **THEN** it SHALL use the caller-supplied `question_count` from `QuizGenerateOptions` AND SHALL NOT read `quiz.default_length` or any config key itself

#### Scenario: App generate uses the configured length

- **GIVEN** the persisted config has `quiz.default_length` of 10 and the Settings modal has not been opened this session
- **WHEN** the codebus-app starts a quiz generate spawn
- **THEN** the spawn's `question_count` SHALL be 10

#### Scenario: App clamps an out-of-range configured length

- **GIVEN** the persisted config has a quiz length of 2 (or 99)
- **WHEN** the codebus-app resolves the generate question count
- **THEN** the value SHALL be clamped to 3 (or 10 respectively)

---
### Requirement: Quiz Verb Library Functions

The system SHALL export two orchestration functions under `codebus_core::verb::quiz`:

- `run_quiz_plan(repo, QuizPlanOptions, on_event, cancel) -> Result<QuizPlanReport, VerbError>` where `QuizPlanOptions` carries `topic: String`. `QuizPlanReport` SHALL carry `outcome: QuizPlanOutcome` (`Scope(Vec<String>)` or `NoMatch(String)`), token usage, start/finish timestamps, and the agent exit code. It SHALL run only the plan spawn and SHALL NOT persist a RunLog (planning is a sub-step).
- `run_quiz_generate(repo, QuizGenerateOptions, on_event, cancel) -> Result<QuizReport, VerbError>` where `QuizGenerateOptions` carries `pages: Vec<String>` and `question_count: u8`. `QuizReport` SHALL carry the fence-stripped quiz markdown body, `planned_pages`, token usage, start/finish timestamps, the agent exit code, and `events_log` (the generate spawn's events.jsonl path, or `None` under the null sink). It SHALL persist a RunLog (mode `quiz`) + events.jsonl.

Cancellation SHALL use the existing `Option<Arc<AtomicBool>>` mechanism. For the Goal flow, `run_quiz_plan`'s `on_event` callback SHALL receive plan-spawn `VerbEvent`s then a terminal `QuizScopePlanned { pages }` or `QuizNoMatch { reason }` lifecycle event; `run_quiz_generate`'s `on_event` callback SHALL receive generate-spawn `VerbEvent`s.

#### Scenario: Downstream crate resolves the quiz orchestration surface

- **WHEN** a downstream crate writes `use codebus_core::verb::quiz::{run_quiz_plan, run_quiz_generate, QuizPlanOptions, QuizGenerateOptions, QuizPlanOutcome, QuizReport};`
- **THEN** compilation SHALL succeed AND both functions SHALL resolve to the signatures above

#### Scenario: Scope planned event precedes the generate call

- **WHEN** `run_quiz_plan` runs and the topic matches pages
- **THEN** its `on_event` callback SHALL receive `QuizScopePlanned { pages }` AND no generate spawn SHALL have run inside `run_quiz_plan` (the caller invokes `run_quiz_generate` separately)

<!-- @trace
source: v3-app-quiz
updated: 2026-05-16
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/spike-artifacts/quiz-fixture-vault/manifest.yaml
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/jwt-token-lifecycle.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/index.md
  - docs/spike-artifacts/spike-quiz-7-F5.jsonl
  - codebus-app/src-tauri/src/ipc/quiz.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/config/quiz.rs
  - docs/spike-artifacts/spike-quiz-7-F1.jsonl
  - codebus-app/src-tauri/src/ipc/config.rs
  - docs/2026-05-15-v3-app-quiz-spike-plan.md
  - docs/spike-artifacts/spike-quiz-7-F6.jsonl
  - docs/spike-artifacts/spike-quiz-8-E3.jsonl
  - docs/spike-artifacts/spike-quiz-9-S1.jsonl
  - codebus-core/src/verb/quiz.rs
  - docs/v3-app-roadmap.md
  - codebus-cli/src/commands/mod.rs
  - codebus-core/src/config/claude_code.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run2.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC1.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/user-store.md
  - docs/spike-artifacts/spike-quiz-10-R1-run1.jsonl
  - codebus-app/src-tauri/src/config.rs
  - codebus-app/src/lib/quiz-parse.ts
  - codebus-core/src/skill_bundle/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/wiki/log.md
  - docs/spike-artifacts/spike-quiz-7-F2.jsonl
  - docs/spike-artifacts/spike-quiz-8-E4.jsonl
  - codebus-app/src/components/workspace/QuizAnswering.tsx
  - docs/2026-05-15-v3-app-quiz-discussion.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/session-vs-token.md
  - docs/spike-artifacts/spike-quiz-8-E5.jsonl
  - codebus-cli/src/commands/quiz.rs
  - docs/spike-artifacts/spike-quiz-9-S3.jsonl
  - codebus-core/src/config/mod.rs
  - codebus-core/src/log/events/sink.rs
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - docs/spike-artifacts/spike-quiz-runbook.md
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/verb/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/CLAUDE.md
  - codebus-core/src/verb/event.rs
  - codebus-app/src/components/settings/SettingsModal.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - docs/spike-artifacts/spike-quiz-8-E2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/raw/code/auth.py
  - docs/spike-artifacts/spike-quiz-8-E1.jsonl
  - docs/spike-artifacts/spike-quiz-7-F3.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/.claude/skills/codebus-quiz/SKILL.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/auth-middleware.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/processes/login-flow.md
  - docs/spike-artifacts/spike-quiz-9-S2.jsonl
  - codebus-core/src/vault/source_gitignore.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run3.jsonl
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/spike-artifacts/spike-quiz-7-F4.jsonl
tests:
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-core/tests/vault_init.rs
  - codebus-cli/tests/bins/mock_claude.rs
  - codebus-cli/tests/quiz_flow.rs
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-core/tests/verb_library_surface.rs
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-app/src/components/workspace/QuizAnswering.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: Quiz Output Validation and Repair

The system SHALL validate a generated quiz markdown body before it is persisted, and SHALL drive deterministic self-repair through the generating agent (trust-agent model), mirroring the single-spawn / agent-internal-loop / caller-final-verifier pattern the `lint-feedback-loop` capability defines for `codebus fix`.

A deterministic quiz validator SHALL be the sole authority for structural correctness. It SHALL accept a quiz markdown body and SHALL produce findings of severity `error` for each of:

1. **Schema findings** — a question block (delimited by `## Q<n>.`) whose stem is empty, OR that does not have exactly the four choice keys `A`, `B`, `C`, `D`, OR that has no `## Answer: X` line where `X` is one of `A`/`B`/`C`/`D`, OR that has no `## Explanation:` line. Surrounding whitespace and blank lines SHALL be tolerated; the structural requirements SHALL NOT be.
2. **Wikilink-existence findings** — a `[[slug]]` citation appearing in any `## Explanation` that does not resolve to an existing page in the vault wiki index.

`run_quiz_generate` SHALL, after the generate spawn returns and the body is fence/preamble-stripped, run the deterministic validator exactly once as the final verifier. Validator findings SHALL be emitted through the same event fan-out used by the generate spawn (the same single `events.jsonl` sink and `on_event` callback for the run), using the existing lint-finding event shape so the CLI renderer, the GUI live stream, and the persisted-events replay surface them identically. The generate spawn, the agent's internal self-validate/self-repair iterations, and the final verify SHALL be one run with one `RunLog`.

When the final verifier still reports `error` findings, the system SHALL persist the quiz best-effort: the caller-injected frontmatter SHALL carry a `validation:` field whose value is `ok` when the final verifier reported zero `error` findings and `failed` otherwise; a non-fatal warning event SHALL be emitted; no question block SHALL be dropped; and `run_quiz_generate` SHALL NOT return a `VerbError` solely because validation failed. An absent `validation:` field SHALL be interpreted by readers as "not validated" and SHALL NOT be treated as `ok`.

A validator internal error or an unreadable wiki index SHALL be treated as non-fatal: a warning event SHALL be emitted, the quiz SHALL still be persisted with `validation: failed`, and `run_quiz_generate` SHALL NOT fail solely for that reason. Spawn failure and cancellation semantics SHALL remain unchanged from the pre-existing `run_quiz_generate` contract.

The deterministic validator SHALL be the single source of truth for structural rules; the codebus-quiz SKILL SHALL NOT embed a parallel copy of the rule definitions (it references the validator and acts on its findings — see `skill-bundles`). Model-based content verification (a separate verify spawn) is OUT OF SCOPE for this requirement; the self-repair feedback path SHALL accept findings of the same shape regardless of source so a future model-verify stage can feed into it without rework.

#### Scenario: Generated quiz with a malformed question is flagged

- **WHEN** a generated quiz body contains a question block missing its `## Answer:` line
- **THEN** the deterministic validator SHALL produce an `error` finding identifying that question AND the finding SHALL be emitted through the run's event fan-out in the lint-finding event shape

#### Scenario: Broken explanation citation is flagged

- **WHEN** a generated quiz explanation cites `[[no-such-page]]` and no page named `no-such-page` exists in the vault wiki index
- **THEN** the deterministic validator SHALL produce an `error` finding for that citation

#### Scenario: Clean quiz validates and is persisted as ok

- **WHEN** a generated quiz passes every schema check and all `[[slug]]` citations resolve
- **THEN** `run_quiz_generate` SHALL persist the quiz with frontmatter `validation: ok` AND SHALL NOT emit a validation warning event

#### Scenario: Residual failure persists best-effort with a marker

- **WHEN** the final verifier still reports `error` findings after the generate spawn returns
- **THEN** the quiz SHALL be persisted with frontmatter `validation: failed` AND a non-fatal warning event SHALL be emitted AND no question block SHALL be dropped AND `run_quiz_generate` SHALL NOT return a `VerbError` solely for the validation failure

#### Scenario: Validator infrastructure error is non-fatal

- **WHEN** the vault wiki index cannot be read while validating
- **THEN** a warning event SHALL be emitted AND the quiz SHALL still be persisted with `validation: failed` AND `run_quiz_generate` SHALL NOT fail solely for that reason

#### Scenario: One run, one events.jsonl, one RunLog

- **WHEN** a quiz is generated and the agent performs internal self-validate/self-repair iterations followed by the caller final verify
- **THEN** all generate, self-repair, and final-verify events SHALL be written to a single `events.jsonl` for that run AND exactly one `RunLog` SHALL be recorded for the run

---
### Requirement: Quiz Content Verification and Repair

The system SHALL provide an optional model-based content verification stage for generated quizzes, gated by a `quiz.content_verify` configuration key (boolean, default `false`). When the key is absent or `false`, `run_quiz_generate` SHALL behave exactly as without this requirement (no verify spawn, no `content_review` frontmatter). The `codebus` CLI SHALL read `quiz.content_verify` from the shared `quiz.*` namespace and SHALL NOT read the app-only `app.*` namespace for it.

When `quiz.content_verify` is `true`, after the deterministic final-verify completes, `run_quiz_generate` SHALL run one **independent verify spawn** (a separate agent from the generate spawn) that reads the planned pages plus the generated quiz body and judges each question against exactly this five-item defect contract, emitting per flagged question its question number, defect type, and a concrete correction suggestion:

1. **answer-wrong** — the marked correct option is not supported as correct by the planned pages.
2. **out-of-scope** — the stem, an option, or the explanation asserts something the planned pages do not state.
3. **not-exactly-one-correct** — more than one option is defensibly correct, or the marked option is not correct.
4. **degenerate-distractor** — a distractor is non-discriminating (blank, a `none-or-all-of-the-above` cop-out, or absurd).
5. **off-topic** — the question is not about the user requested topic. This item SHALL be judged ONLY when an originating topic is supplied (the Goal flow); when no topic is supplied (the Page flow) item 5 SHALL be skipped and the other four SHALL still be judged.

The verify spawn input SHALL include the planned page list as a structured segment so the agent does not have to reverse-engineer the scope from the quiz body `[[slug]]` citations. The spawn input SHALL contain at least: an originating-topic segment (`topic=<topic-or-empty>`), a `PLANNED PAGES:` block listing each planned page on its own line (vault-relative path starting with `wiki/`), and a `QUIZ:` block containing the generated quiz body. The `PLANNED PAGES:` block SHALL be present even when the planned-page list is empty (an empty `PLANNED PAGES:` block plus a closing blank line is permitted, signalling no pages). This requirement closes the prompt-surface-review F93 finding (verify spawn previously received only the topic and quiz body; an empirical 2026-05-24 run confirmed the agent reverse-engineered page coverage from `[[slug]]` citations and missed two of three planned pages, producing unparseable verify output and an unconditional `content_review: flagged` for clean quizzes).

The verify spawn SHALL resolve its model and effort via `cc_cfg.resolve(Verb::Verify)`, NOT `Verb::Quiz` (`claude-code-config` Endpoint Profile Schema requirement defines the `Verb::Verify` resolution path). This ensures the verify spawn uses the dedicated `claude_code.system.verify` / `claude_code.azure.verify` sub-block, which is independent of the plan / generate / repair spawns that continue to use `Verb::Quiz`. The motivating use case is cheap generation paired with expensive verification (e.g., haiku for quiz plan/generate, opus for verify).

`run_quiz_generate` SHALL accept the originating topic as an optional input; the Goal flow SHALL supply it and the Page flow SHALL supply none. Verify findings SHALL be emitted through the same event fan-out used by the generate spawn (one `events.jsonl` + `on_event` for the run) in the existing lint-finding event shape, and SHALL drive a caller-orchestrated repair loop: when the verify spawn reports defects, a repair spawn (reusing the generate mode, given the same pages + count + the previous quiz body + the verify defect list, instructed to revise only the flagged questions and keep the question count) SHALL produce a revised body, which SHALL then be re-verified. The repair spawn SHALL resolve its model via `cc_cfg.resolve(Verb::Quiz)` (NOT `Verb::Verify`) — the repair stage continues to use the same model as the original generate stage, so the cost profile is verify-with-expensive-model paired with repair-using-the-same-cheap-model-used-for-generate. This verify-then-repair-then-re-verify loop SHALL be bounded by a fixed caller-counted cap of 3 iterations; when the cap is reached the current best body SHALL be emitted rather than looping further. This is a new caller-orchestrated mechanism (an independent verify model judging, then a bounded repair) — not the Stage-1 intra-spawn agent self-repair.

The persisted quiz caller frontmatter SHALL carry a `content_review` field whose value is `ok` when the final verify reported zero defects and `flagged` otherwise; when `flagged`, the persisted frontmatter SHALL also list the flagged question numbers. Residual defects after the cap SHALL be best-effort: a non-fatal warning SHALL be emitted, no question SHALL be dropped, and `run_quiz_generate` SHALL NOT return a `VerbError` solely because content defects remain (exit semantics unchanged). A verify spawn failure or unparseable verify output SHALL be treated as non-fatal: a warning SHALL be emitted, the quiz SHALL be persisted with `content_review: flagged` (never silently `ok`), and the verb SHALL NOT fail solely for that reason. An absent `content_review` field SHALL be read as content-not-verified AND SHALL NOT be treated as `ok`.

The verify spawn SHALL be read-only (no Bash, no validator invocation) — content judgement is distinct from the deterministic `codebus quiz validate` structural/citation check, which this requirement does not change.

`run_quiz_generate` SHALL NOT emit verify-spawn model / effort metadata into the per-run `RunLog` entry. The `RunLog` `model` and `effort` fields SHALL continue to record the main quiz spawn model (`Verb::Quiz` resolution); the verify spawn model is observable via the `events.jsonl` per-run timeline (which already records every spawn `SpawnStart` event including the model in use), but SHALL NOT appear in the consolidated `RunLog` row.

#### Scenario: Content verify disabled by default

- **WHEN** `run_quiz_generate` runs and `quiz.content_verify` is absent or `false`
- **THEN** no verify spawn SHALL run AND the persisted quiz SHALL NOT contain a `content_review` field

#### Scenario: Clean content marks ok

- **WHEN** `quiz.content_verify` is `true` and the verify spawn reports zero defects
- **THEN** the persisted quiz frontmatter SHALL be `content_review: ok` AND no content warning SHALL be emitted

#### Scenario: Defect triggers bounded repair then marks state

- **WHEN** `quiz.content_verify` is `true` and the verify spawn flags question 3 as answer-wrong
- **THEN** question 3 defect SHALL be fed back to the generate agent for revision AND the stage SHALL re-verify, repeating at most the fixed cap of iterations AND the final persisted frontmatter SHALL be `content_review: ok` if cleared or `content_review: flagged` listing the still-flagged question numbers if not

#### Scenario: Residual defects are best-effort

- **WHEN** defects remain after the iteration cap is reached
- **THEN** the quiz SHALL be persisted with `content_review: flagged` and the flagged question numbers AND a non-fatal warning SHALL be emitted AND no question SHALL be dropped AND the verb exit status SHALL be unchanged

#### Scenario: Off-topic item is Goal-flow only

- **WHEN** the verify spawn runs for a Page-flow generation (no originating topic supplied)
- **THEN** the off-topic item SHALL NOT be evaluated AND the other four defect items SHALL still be evaluated

#### Scenario: Verify infrastructure failure is non-fatal

- **WHEN** the verify spawn fails or its output cannot be parsed
- **THEN** a warning SHALL be emitted AND the quiz SHALL be persisted with `content_review: flagged` AND `run_quiz_generate` SHALL NOT fail solely for that reason

#### Scenario: Verify spawn uses Verb::Verify model not Verb::Quiz

- **WHEN** `quiz.content_verify` is `true`, `claude_code.system.quiz` resolves to `model: haiku-4-5`, AND `claude_code.system.verify` resolves to `model: opus-4-6`
- **THEN** the verify spawn SHALL be invoked with `--model claude-opus-4-6` AND the generate / plan / repair spawns SHALL be invoked with `--model claude-haiku-4-5`

#### Scenario: Repair spawn uses Verb::Quiz model not Verb::Verify

- **WHEN** `quiz.content_verify` is `true`, the verify spawn flags a question, AND `claude_code.system.verify` resolves to `model: opus-4-6` while `claude_code.system.quiz` resolves to `model: haiku-4-5`
- **THEN** the repair spawn SHALL be invoked with `--model claude-haiku-4-5` (the quiz model, NOT the verify model) — repair keeps the same model profile as generate

#### Scenario: RunLog model field records main spawn not verify

- **WHEN** `quiz.content_verify` is `true`, the main quiz spawn resolves to `haiku-4-5`, AND the verify spawn resolves to `opus-4-6`
- **THEN** the per-run `RunLog` entry SHALL record `model: claude-haiku-4-5` (the main spawn model) AND SHALL NOT record the verify spawn model in any RunLog field

#### Scenario: Verify spawn input includes planned page list

- **WHEN** `quiz.content_verify` is `true`, `run_quiz_generate` is invoked with three planned pages (`wiki/processes/jwt-issue-and-verify.md`, `wiki/modules/auth-module.md`, `wiki/entities/jwt-payload.md`) and topic `JWT issuance and verification`, and the generate spawn returns a body
- **THEN** the verify spawn `input` field SHALL contain the literal substring `PLANNED PAGES:` AND SHALL contain each of the three planned page paths on its own line within that block AND SHALL contain a `QUIZ:` block carrying the generated body AND SHALL contain `topic=JWT issuance and verification`

##### Example: Verify spawn input shape

- **GIVEN** the planned pages are `wiki/processes/jwt-issue-and-verify.md` and `wiki/modules/auth-module.md`, the topic is `JWT issuance and verification`, and the generated quiz body starts with `## Q1.`
- **WHEN** the verify spawn is composed
- **THEN** the spawn `input` SHALL match the shape (modulo whitespace) `topic=JWT issuance and verification` followed by a `PLANNED PAGES:` block listing both planned pages on their own lines, followed by a `QUIZ:` block carrying the body starting `## Q1. ...`

#### Scenario: Verify spawn input includes empty page block when none planned

- **WHEN** `run_quiz_generate` is invoked with an empty planned-pages list (a degenerate Page-flow case) and `quiz.content_verify` is `true`
- **THEN** the verify spawn `input` SHALL still contain a `PLANNED PAGES:` header AND the block beneath it SHALL be empty (no page paths) AND the spawn SHALL still emit its content judgement

<!-- @trace
source: quiz-content-verify, verify-stage-independent-model, prompt-surface-output-discipline-batch
updated: 2026-05-24
code:
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/config/quiz.rs
  - codebus-cli/src/commands/quiz.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-app/src-tauri/src/ipc/quiz.rs
tests:
  - codebus-cli/tests/quiz_flow.rs
  - codebus-core/tests/verb_library_surface.rs
  - codebus-cli/tests/bins/mock_claude.rs
-->

<!-- @trace
source: prompt-surface-output-discipline-batch
updated: 2026-05-24
code:
  - docs/2026-05-23-prompt-surface-inventory.md
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/verb/content_verify.rs
-->

---
### Requirement: Quiz History Row Title Displays User-Authored Topic

The codebus-app Quiz history list SHALL display each quiz attempt row using the user-authored topic (the free-text string the user typed when starting the quiz, or the page title for the Page flow) as the row's primary title. The internal hash-derived slug used for filesystem layout (e.g., `topic-a7fb67fc`, derived from `<vault>/.codebus/quiz/<slug>/` per the Quiz Storage Layout requirement) SHALL NOT be the row's primary title.

When the user-authored topic is available — either from the quiz file's caller-injected `topic` frontmatter (Goal flow) or `target_page` frontmatter (Page flow) — the row's primary title SHALL be that value verbatim. The slug MAY be retained as supporting metadata in a secondary visual position (subtitle, tooltip, or hidden) but SHALL NOT visually dominate the row.

When neither `topic` nor `target_page` frontmatter is present (e.g., a legacy attempt file or an unparseable frontmatter), the row SHALL fall back to the slug as the primary title rather than rendering an empty string, so the row remains identifiable even for degraded data.

#### Scenario: Goal-flow quiz shows the user's topic, not the slug

- **GIVEN** a quiz attempt persisted under `<vault>/.codebus/quiz/topic-a7fb67fc/2026-05-25T16-53-17Z.md` whose caller-injected frontmatter is `topic: 專案目的`
- **WHEN** the codebus-app Quiz history list renders this attempt's row
- **THEN** the row's primary title SHALL be `專案目的` AND SHALL NOT be `topic-a7fb67fc`

#### Scenario: Page-flow quiz shows the target page name

- **GIVEN** a quiz attempt persisted under `<vault>/.codebus/quiz/desktop-workspace/2026-05-25T17-10-04Z.md` whose caller-injected frontmatter is `target_page: 桌面工作台` and has no `topic` field
- **WHEN** the codebus-app Quiz history list renders this attempt's row
- **THEN** the row's primary title SHALL be `桌面工作台`

#### Scenario: Legacy attempt without topic frontmatter falls back to slug

- **GIVEN** a quiz attempt whose frontmatter contains neither `topic` nor `target_page`
- **WHEN** the codebus-app Quiz history list renders this attempt's row
- **THEN** the row's primary title SHALL be the slug from the attempt's directory path AND SHALL NOT be an empty string

<!-- @trace
source: critical-bugs-ql1-x1-qgen1
updated: 2026-05-26
code:
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src/i18n/messages.ts
tests:
  - codebus-app/src/components/workspace/ActivityStreamItem.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
-->