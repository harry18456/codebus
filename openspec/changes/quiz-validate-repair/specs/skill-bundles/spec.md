## MODIFIED Requirements

### Requirement: Quiz Skill Bundle Content

The `codebus-quiz` SKILL.md SHALL declare a read scope of `wiki/` only and SHALL forbid reading `raw/`, `log/`, and any path escaping the vault root. It SHALL define two prompt modes selected by the prompt prefix: `plan:` (emit `[CODEBUS_QUIZ_SCOPE]` or `[CODEBUS_QUIZ_NO_MATCH]` as the first line, then stop) and `generate:` (emit the quiz markdown body). It SHALL require the `[CODEBUS_QUIZ_VIOLATION] <path>` marker when forced toward `raw/`. It SHALL forbid the agent from authoring `quiz_id`, `topic`, or `generation_token_usage`, and forbid wrapping the whole output in a code fence. Markers and structural tokens SHALL always be English; question stems, choices, and explanations SHALL follow the language of the quizzed wiki pages (Language Override).

The `generate:` mode SHALL additionally instruct the agent to self-validate and self-repair before emitting its final body: after drafting the quiz, the agent SHALL invoke `codebus quiz validate` on its draft via its Bash tool, SHALL correct the questions reported by the findings, and SHALL re-run the validator, repeating up to a fixed internal iteration cap stated explicitly in the SKILL body; when the cap is reached the agent SHALL emit its best current body rather than looping further. The SKILL SHALL reference the validator as the authority for structural and citation correctness and SHALL NOT restate the validator's rule definitions (no parallel schema copy); it SHALL describe acting on the validator's findings, not the rules themselves.

#### Scenario: Quiz bundle declares wiki-only read scope

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its body SHALL state that read scope is `wiki/` only AND SHALL explicitly forbid reading `raw/`

#### Scenario: Quiz bundle defines plan and generate modes

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** it SHALL define the `plan:` mode emitting `[CODEBUS_QUIZ_SCOPE]`/`[CODEBUS_QUIZ_NO_MATCH]` and the `generate:` mode emitting the question body without agent-authored frontmatter

#### Scenario: Generate mode defines a bounded self-validate/self-repair loop

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its `generate:` mode SHALL instruct the agent to invoke `codebus quiz validate` on its draft, correct reported findings, and re-validate up to a fixed internal iteration cap stated in the body AND SHALL instruct the agent to emit its best current body when the cap is reached

#### Scenario: Quiz bundle does not duplicate validator rules

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its body SHALL reference `codebus quiz validate` as the structural/citation authority AND SHALL NOT contain a restated copy of the validator's rule definitions
