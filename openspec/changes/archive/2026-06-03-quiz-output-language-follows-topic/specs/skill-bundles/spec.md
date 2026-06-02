## MODIFIED Requirements

### Requirement: NEUTRAL_RULES Language Policy

The `NEUTRAL_RULES` schema document (materialized as `<vault>/CLAUDE.md` on the Claude provider path and as `<vault>/AGENTS.md` on the codex provider path) SHALL contain a `§0 Language Policy` section preceding `§1 Workspace Layout` that defines the following normative rules:

1. The natural language of agent output — page bodies, stdout summary lines, and answer text — SHALL follow the prompt context language (the language of the user's goal/query/chat text), and SHALL NOT default to the language of any existing wiki page or raw source content read along the way.
2. Structural tokens and YAML keys (`type:`, `sources:`, `created:`, `updated:`, marker lines such as `[CODEBUS_*]`) SHALL always be literal English regardless of the prompt context language.
3. For the `quiz` verb specifically, the natural language of question stems, choices, and explanations SHALL follow the language of the quiz **topic** when an originating topic is supplied (the Goal flow); when no topic is supplied (the Page flow / wiki-preview "Quiz me on this"), the language SHALL fall back to auto-detecting the language of the quizzed wiki pages. Quiz structural tokens and markers (`[CODEBUS_QUIZ_*]`, `## Answer:`, `## Explanation:`) SHALL always remain literal English per rule 2. Quiz output SHALL NOT default to the auto-detected dominant language of the quizzed pages when a topic is present, because technical pages mixing prose with English slugs, frontmatter, code identifiers, and `[[wikilink]]` targets frequently auto-detect to English even when the prose is not English.

This requirement makes the contract real for the SKILL workflow body references (in `codebus-core/src/skill_bundle/mod.rs`, including goal Step 5, query Step 4, fix Step 5, and the quiz `## Language Override` section) that cite "the §0 Language Policy in cwd CLAUDE.md" as the authority for output-language selection. Without `§0` the contract is dangling: agent behavior on multi-language prompts falls back to the underlying model's heuristic, producing inconsistent output language across providers and across model versions. Rule 3 closes the specific defect where a Chinese wiki produced an English quiz because the quiz SKILL auto-detected the dominant page language instead of following the topic.

#### Scenario: Multi-language goal produces same-language summary

- **WHEN** a user runs `codebus goal "把支付模組的時序圖整理出來"` (Traditional Chinese goal text) against a vault whose existing wiki pages are written in English
- **THEN** the agent's stdout summary line and any newly written or updated wiki page body content SHALL be in Traditional Chinese
- **AND** structural frontmatter keys (`type:`, `sources:`, `created:`, `updated:`, `[CODEBUS_*]` markers) SHALL remain literal English

##### Example: Mixed-language vault, Japanese goal

- **GIVEN** a vault containing `wiki/modules/payment-gateway.md` authored in English and `wiki/concepts/checkout-flow.md` authored in Traditional Chinese
- **WHEN** a user runs `codebus goal "決済処理の主要なコンポーネントを把握したい"` (Japanese goal text)
- **THEN** the stdout summary line SHALL be in Japanese
- **AND** any new `## from goal: ... (YYYY-MM-DD)` section appended to existing pages SHALL have its body in Japanese (per Language Override) while the `## from goal:` heading literal and date stay English/numeric
- **AND** `type:`, `sources:`, `goals:`, `created:`, `updated:` keys in frontmatter SHALL remain literal English

#### Scenario: Schema document materialized with Language Policy preceding workspace layout

- **WHEN** `codebus init` materializes `NEUTRAL_RULES` to the vault's `CLAUDE.md` (Claude provider path) or `AGENTS.md` (codex provider path)
- **THEN** the materialized file SHALL contain `## 0. Language Policy` as a section ordered before `## 1. Workspace Layout`
- **AND** the section body SHALL define both the agent-output-language rule and the structural-tokens-stay-English rule

#### Scenario: Quiz output language follows the topic, falling back to page language when absent

- **WHEN** a quiz is generated
- **THEN** the natural language of question stems, choices, and explanations SHALL follow the quiz topic language when an originating topic is supplied AND SHALL fall back to the auto-detected language of the quizzed wiki pages when no topic is supplied
- **AND** quiz structural tokens (`## Answer:`, `## Explanation:`, `[CODEBUS_QUIZ_*]`) SHALL remain literal English in every case

##### Example: Topic-follows enumeration

| Flow | Topic | Quizzed pages | Question/choice/explanation language |
| ---- | ----- | ------------- | ------------------------------------ |
| Goal | Traditional Chinese topic | mixed Traditional Chinese prose + English slugs/identifiers | Traditional Chinese (follows topic) |
| Goal | English topic | mixed Traditional Chinese prose + English slugs/identifiers | English (follows topic) |
| Page | (none supplied) | Traditional Chinese page | Traditional Chinese (page auto-detect fallback) |

### Requirement: Quiz Skill Bundle Content

The `codebus-quiz` SKILL.md SHALL declare a read scope of `wiki/` only and SHALL forbid reading `raw/`, `log/`, and any path escaping the vault root. It SHALL define two prompt modes selected by the prompt prefix: `plan:` (emit `[CODEBUS_QUIZ_SCOPE]` or `[CODEBUS_QUIZ_NO_MATCH]` as the first line, then stop) and `generate:` (emit the quiz markdown body). It SHALL require the `[CODEBUS_QUIZ_VIOLATION] <path>` marker when forced toward `raw/`. It SHALL forbid the agent from authoring `quiz_id`, `topic`, or `generation_token_usage`, and forbid wrapping the whole output in a code fence. Markers and structural tokens SHALL always be English. Question stems, choices, and explanations SHALL follow the language of the quiz **topic** when the `generate:` prompt supplies an optional `topic=<...>` field, and SHALL fall back to auto-detecting the language of the quizzed wiki pages when no `topic=` field is supplied (Language Override, aligned with the §0 Language Policy rule 3 — the SKILL SHALL NOT instruct following the dominant quizzed-page language when a topic is present).

The `generate:` mode prompt contract SHALL be `generate: pages=[<path1>,<path2>,...] count=<N>` and SHALL accept an OPTIONAL `topic=<...>` field carried alongside `pages=[...] count=<N>` as the language signal. The `topic=<...>` field SHALL be present for the Goal flow (where the caller has an originating topic) and SHALL be absent for the Page flow. The agent SHALL treat the presence/absence of `topic=<...>` as the switch between topic-follows and page-auto-detect language selection.

The `generate:` mode SHALL additionally instruct the agent to self-validate and self-repair before emitting its final body: after drafting the quiz, the agent SHALL invoke `codebus quiz validate` on its draft via its Bash tool, SHALL correct the questions reported by the findings, and SHALL re-run the validator, repeating up to a fixed internal iteration cap stated explicitly in the SKILL body; when the cap is reached the agent SHALL emit its best current body rather than looping further. The SKILL SHALL reference the validator as the authority for structural and citation correctness and SHALL NOT restate the validator rule definitions (no parallel schema copy); it SHALL describe acting on the validator findings, not the rules themselves.

The SKILL SHALL ALSO define a third prompt mode `verify:` selected by the prompt prefix. The `verify:` mode SHALL instruct the agent to read the supplied planned pages plus a generated quiz body and judge each question against exactly five content defect types — answer-wrong (marked option not supported as correct by the planned pages), out-of-scope (a claim the planned pages do not state), not-exactly-one-correct (multiple defensibly-correct options or the marked one wrong), degenerate-distractor (a non-discriminating distractor), and off-topic (not about the supplied topic; evaluated only when a topic is supplied) — and to emit, for each flagged question, its question number, the defect type, and a concrete correction suggestion. The `verify:` mode SHALL NOT restate the deterministic validator structural/citation rules, and the SKILL SHALL keep the deterministic `codebus quiz validate` structural check separate from this content judgement.

The `verify:` mode SHALL additionally contain an explicit output-termination boundary instructing the agent to STOP after the last `Q<n> | <defect-type> | <suggestion>` line (or after `CONTENT_OK`), and to emit no further prose, rationale, evaluation summary, or per-question commentary. The SKILL body SHALL state this boundary as a normative MUST/SHALL clause, parallel in shape to the `plan:` mode boundary that forbids any content before the `[CODEBUS_QUIZ_SCOPE]` line. This requirement closes the prompt-surface-review F78 finding (an empirical 2026-05-24 run observed the verify agent emitting `**Q1 evaluation** / **Q2 evaluation** / **Q3 evaluation** / 驗證: xxx 第 N 行` rationale paragraphs before the closing `CONTENT_OK`, a contract violation that contributed to the unparseable-verify-output incident even though the line-by-line splitn parser silently skipped most prose).

#### Scenario: Quiz bundle declares wiki-only read scope

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its body SHALL state that read scope is `wiki/` only AND SHALL explicitly forbid reading `raw/`

#### Scenario: Quiz bundle defines plan and generate modes

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** it SHALL define the `plan:` mode emitting `[CODEBUS_QUIZ_SCOPE]`/`[CODEBUS_QUIZ_NO_MATCH]` and the `generate:` mode emitting the question body without agent-authored frontmatter

#### Scenario: Generate mode language override follows topic with page fallback

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its `## Language Override` section SHALL instruct that question stems, choices, and explanations follow the quiz topic language when a `topic=<...>` field is supplied in the `generate:` prompt AND fall back to auto-detecting the quizzed wiki pages' language when no `topic=` field is supplied AND SHALL NOT instruct following the dominant quizzed-page language when a topic is present

#### Scenario: Generate mode contract accepts an optional topic field

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its `generate:` mode contract SHALL document `pages=[...] count=<N>` with an OPTIONAL `topic=<...>` field AND SHALL state that `topic=<...>` is present for the Goal flow and absent for the Page flow

#### Scenario: Generate mode defines a bounded self-validate/self-repair loop

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its `generate:` mode SHALL instruct the agent to invoke `codebus quiz validate` on its draft, correct reported findings, and re-validate up to a fixed internal iteration cap stated in the body AND SHALL instruct the agent to emit its best current body when the cap is reached

#### Scenario: Quiz bundle does not duplicate validator rules

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its body SHALL reference `codebus quiz validate` as the structural/citation authority AND SHALL NOT contain a restated copy of the validator rule definitions

#### Scenario: Verify mode defines the five-item content defect contract

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** it SHALL define a `verify:` mode that judges each question against the five defect types (answer-wrong, out-of-scope, not-exactly-one-correct, degenerate-distractor, off-topic) AND instructs emitting per flagged question its number, defect type, and correction suggestion AND SHALL keep this content judgement separate from the deterministic `codebus quiz validate` structural check

#### Scenario: Verify mode declares STOP boundary after defect lines

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** the `verify:` mode body SHALL contain a normative clause instructing the agent to STOP after the last `Q<n> | <defect-type> | <suggestion>` line or after `CONTENT_OK` AND SHALL forbid emitting any further prose, rationale, evaluation summary, or per-question commentary

##### Example: STOP boundary shape

- **GIVEN** the SKILL body for the `verify:` mode
- **WHEN** a reader looks for output-termination language
- **THEN** the body SHALL contain a sentence stating substantively that after the last `Q<n> | <defect-type> | <suggestion>` line (or after `CONTENT_OK`) the agent SHALL stop emitting content AND SHALL NOT emit further prose, rationale, or summary
