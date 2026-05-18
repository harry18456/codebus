## MODIFIED Requirements

### Requirement: Quiz Verb Two-Shot Flow

The system SHALL provide a `quiz` verb that produces a multiple-choice quiz from wiki pages via at most two agent spawns, split across two separately-invokable library functions so a caller (the GUI) can interpose a confirmation between them. The Goal flow SHALL call `run_quiz_plan` (a **plan** spawn) and then, only when the plan returned a scope, `run_quiz_generate` (a **generate** spawn). The Page flow (wiki-preview `[Quiz me on this]`) SHALL call `run_quiz_generate` directly with `pages = [target]`, skipping the plan spawn; the SKILL expands the target to its one-hop wikilinked pages.

The plan spawn SHALL take a free-text topic and emit, as the first line of its response, either `[CODEBUS_QUIZ_SCOPE] <wiki-path>, <wiki-path>, ...` (2–5 vault-relative `wiki/` paths, most relevant first) when matching pages exist, or `[CODEBUS_QUIZ_NO_MATCH] <reason>` when no wiki page covers the topic. The generate spawn SHALL emit a quiz markdown document body (no agent-authored frontmatter).

The caller's plan-marker parser SHALL tolerantly recover the marker rather than hard-failing on minor agent deviations, mirroring the tolerant leading/trailing code-fence handling already applied to the generate body: it SHALL strip a leading Markdown code fence, and it SHALL accept the first line that begins with `[CODEBUS_QUIZ_SCOPE]` or `[CODEBUS_QUIZ_NO_MATCH]` even when the agent emitted preamble lines before it. The SKILL still instructs the agent to emit the marker as the very first line; tolerant recovery is a caller-side robustness measure, not a relaxation of the agent-side contract. When no marker is recoverable from the spawn output, `run_quiz_plan` SHALL return an error whose message includes a truncated head (at most 200 characters) of the actual spawn output, so the failure is diagnosable rather than opaque.

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

#### Scenario: Plan marker recovered despite agent preamble

- **GIVEN** a plan spawn whose response is `Sure, here is the scope.\n[CODEBUS_QUIZ_SCOPE] wiki/a.md`
- **WHEN** the caller parses the plan output
- **THEN** `run_quiz_plan` SHALL return `QuizPlanOutcome::Scope(["wiki/a.md"])` (the preamble line is tolerated, not a hard failure)

#### Scenario: Plan marker recovered despite a wrapping code fence

- **GIVEN** a plan spawn whose response is a Markdown code fence wrapping `[CODEBUS_QUIZ_NO_MATCH] vault only covers web auth`
- **WHEN** the caller parses the plan output
- **THEN** `run_quiz_plan` SHALL return `QuizPlanOutcome::NoMatch("vault only covers web auth")`

#### Scenario: Unrecoverable plan output surfaces a diagnostic head

- **WHEN** the plan spawn output contains neither marker on any line
- **THEN** `run_quiz_plan` SHALL return an error whose message includes a truncated head (≤200 chars) of the actual spawn output AND SHALL NOT silently succeed

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
