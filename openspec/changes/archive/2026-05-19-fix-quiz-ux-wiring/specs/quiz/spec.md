## MODIFIED Requirements

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
