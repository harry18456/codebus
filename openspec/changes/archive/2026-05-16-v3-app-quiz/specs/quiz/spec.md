## ADDED Requirements

### Requirement: Quiz Verb Two-Shot Flow

The system SHALL provide a `quiz` verb that produces a multiple-choice quiz from wiki pages via at most two agent spawns, split across two separately-invokable library functions so a caller (the GUI) can interpose a confirmation between them. The Goal flow SHALL call `run_quiz_plan` (a **plan** spawn) and then, only when the plan returned a scope, `run_quiz_generate` (a **generate** spawn). The Page flow (wiki-preview `[Quiz me on this]`) SHALL call `run_quiz_generate` directly with `pages = [target]`, skipping the plan spawn; the SKILL expands the target to its one-hop wikilinked pages.

The plan spawn SHALL take a free-text topic and emit, as the first line of its response, either `[CODEBUS_QUIZ_SCOPE] <wiki-path>, <wiki-path>, ...` (2–5 vault-relative `wiki/` paths, most relevant first) when matching pages exist, or `[CODEBUS_QUIZ_NO_MATCH] <reason>` when no wiki page covers the topic. The generate spawn SHALL emit a quiz markdown document body (no agent-authored frontmatter).

`run_quiz_plan` SHALL NOT continue to generation; the caller decides whether/when to call `run_quiz_generate` (CLI proceeds immediately with no gate; GUI waits for the user to confirm or revise the scope).

Answer grading SHALL be performed by the caller comparing the user's selected choice against the quiz markdown `Answer` field. The agent SHALL NOT grade and SHALL NOT receive the user's answers.

#### Scenario: Goal flow runs plan then (on scope) generate

- **WHEN** `run_quiz_plan` is invoked with `QuizPlanOptions { topic }` and the topic matches wiki pages, and the caller then invokes `run_quiz_generate` with the returned pages
- **THEN** the plan spawn SHALL emit `[CODEBUS_QUIZ_SCOPE]` with 2–5 `wiki/` paths AND `run_quiz_plan` SHALL return `QuizPlanOutcome::Scope(pages)` AND the subsequent `run_quiz_generate` SHALL emit the quiz markdown body for those pages

#### Scenario: Goal flow with no matching pages

- **WHEN** `run_quiz_plan` is invoked with `QuizPlanOptions { topic }` and no wiki page covers the topic
- **THEN** the plan spawn SHALL emit `[CODEBUS_QUIZ_NO_MATCH] <reason>` as its first line AND `run_quiz_plan` SHALL return `QuizPlanOutcome::NoMatch(reason)` AND surface a `QuizNoMatch { reason }` lifecycle event AND no generate spawn SHALL run

##### Example: off-topic plan request

- **GIVEN** a vault whose wiki only covers web auth
- **WHEN** the plan spawn receives topic "how to bake sourdough bread"
- **THEN** the first response line SHALL be `[CODEBUS_QUIZ_NO_MATCH] vault only covers web auth; no page relates to sourdough bread baking`

#### Scenario: Page flow skips planning

- **WHEN** `run_quiz_generate` is invoked with `QuizGenerateOptions { pages: [target], question_count }` (the wiki-preview Page flow)
- **THEN** no plan spawn SHALL run AND the generate spawn SHALL build its context from `target` plus the pages `target` wikilinks to (one hop)

### Requirement: Quiz Read Scope Enforcement

The quiz verb SHALL read only paths under `wiki/` relative to the vault root. The verb SHALL NOT read `raw/`, `raw/code/`, `log/`, or any path escaping the vault root. Enforcement SHALL be by the `codebus-quiz` skill prompt invariant; the system SHALL NOT add a library-level tool_use path interceptor in this change. If the user prompt asks the agent to read source code or `raw/`, the agent SHALL refuse, redirect to the corresponding `wiki/` page, and SHALL NOT issue any tool call whose path resolves under `raw/`.

The events.jsonl record produced for the generate spawn SHALL retain the full `tool_use` trace so that read-scope violations are auditable after the fact.

#### Scenario: Prompt steering toward source code is refused

- **WHEN** the plan spawn receives a topic that explicitly asks to read source code (e.g. "show me the source code of the middleware")
- **THEN** the agent SHALL NOT emit any `tool_use` with a path under `raw/` AND SHALL still emit a `[CODEBUS_QUIZ_SCOPE]` line pointing only at `wiki/` pages

#### Scenario: Violation marker on forced raw access

- **WHEN** the agent determines it is being compelled to access a `raw/` path
- **THEN** it SHALL emit `[CODEBUS_QUIZ_VIOLATION] <attempted-path>` and stop rather than perform the read

### Requirement: Quiz Markdown Schema and Caller Frontmatter Injection

The generate spawn SHALL emit a markdown body containing exactly `count` question sections. Each question SHALL be a `## Q<i>.` heading (i from 1 to count) followed by exactly four choices labelled `A)` through `D)`, exactly one `## Answer: <A|B|C|D>` line, and exactly one `## Explanation:` line citing a source via `[[slug]]` wikilink syntax. The agent SHALL NOT wrap the whole output in a code fence. The agent SHALL NOT author `quiz_id`, `topic`, `planned_pages`, `generation_token_usage`, or any frontmatter.

The caller SHALL inject frontmatter on persistence: `quiz_id` (real ISO timestamp), `trigger` (`ai_planned` or `wiki_preview`), `topic` (plan topic) or `target_page` (page-scope target), `planned_pages` (list), `generation_token_usage` (from the report), and `events_log` (the generate spawn's events.jsonl path). The caller parser SHALL tolerantly strip a leading/trailing code fence if the agent emits one despite the prohibition.

#### Scenario: Question block well-formed

- **WHEN** the generate spawn runs with `count=5`
- **THEN** the body SHALL contain `## Q1.` through `## Q5.`, each with four `A)`–`D)` choices, one `## Answer:` line, and one `## Explanation:` line

#### Scenario: Caller injects authoritative frontmatter

- **WHEN** the caller persists a generated quiz
- **THEN** the persisted file's frontmatter `quiz_id` SHALL be a caller-generated timestamp (not any value the agent emitted) AND `events_log` SHALL point to the events.jsonl file for that generate spawn (from `QuizReport.events_log`)

#### Scenario: Tolerant fence stripping

- **WHEN** the agent emits the body wrapped in a leading and trailing markdown code fence despite the prohibition
- **THEN** the caller SHALL strip the surrounding fence before persisting AND the persisted file SHALL begin with caller frontmatter, not a fence

### Requirement: Quiz Storage Layout and Retry Semantics

Each quiz attempt SHALL be persisted as one file at `<vault>/.codebus/quiz/<slug>/<iso-timestamp>.md`, where `<slug>` is the page slug for `wiki_preview` trigger or the topic slug for `ai_planned` trigger. Persisting an attempt SHALL NOT overwrite or delete any prior attempt file. Quiz history SHALL be derived by scanning this directory tree, not by correlating run-log entries.

Retry SHALL be a plain re-invocation of the same flow (Goal: `run_quiz_plan` then `run_quiz_generate`; Page: `run_quiz_generate`) with the same inputs. The system SHALL NOT inject previous question stems as negative context and SHALL NOT guarantee that a retry produces different questions. User-facing surfaces SHALL NOT claim that retry always yields new questions.

#### Scenario: Retry creates a new non-destructive file

- **WHEN** a quiz on the same scope is generated twice
- **THEN** two distinct timestamped files SHALL exist under the same `<slug>` directory AND the earlier file's contents SHALL be unchanged

#### Scenario: Retry questions are not guaranteed distinct

- **WHEN** the same flow is re-invoked with identical inputs
- **THEN** the system SHALL NOT pass any record of the prior questions into the second invocation AND the second quiz MAY repeat questions from the first

### Requirement: Shared Quiz Config Namespace

The system SHALL introduce a shared `quiz.*` namespace in `~/.codebus/config.yaml` containing `quiz.default_length` (integer, 3–10, default 5). Both the `codebus quiz` CLI and the codebus-app SHALL read this key. A missing `quiz.default_length` SHALL resolve to default 5. Neither `run_quiz_plan` nor `run_quiz_generate` SHALL read config; `question_count` SHALL be supplied by the caller to `run_quiz_generate`.

#### Scenario: Missing key resolves to default

- **WHEN** config is loaded and `quiz.default_length` is absent
- **THEN** the resolved value SHALL be 5

#### Scenario: Library does not read config

- **WHEN** `run_quiz_generate` is invoked
- **THEN** it SHALL use the caller-supplied `question_count` from `QuizGenerateOptions` AND SHALL NOT read `quiz.default_length` or any config key itself

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
