## MODIFIED Requirements

### Requirement: Quiz Skill Bundle Content

The `codebus-quiz` SKILL.md SHALL declare a read scope of `wiki/` only and SHALL forbid reading `raw/`, `log/`, and any path escaping the vault root. It SHALL define two prompt modes selected by the prompt prefix: `plan:` (emit `[CODEBUS_QUIZ_SCOPE]` or `[CODEBUS_QUIZ_NO_MATCH]` as the first line, then stop) and `generate:` (emit the quiz markdown body). It SHALL require the `[CODEBUS_QUIZ_VIOLATION] <path>` marker when forced toward `raw/`. It SHALL forbid the agent from authoring `quiz_id`, `topic`, or `generation_token_usage`, and forbid wrapping the whole output in a code fence. Markers and structural tokens SHALL always be English; question stems, choices, and explanations SHALL follow the language of the quizzed wiki pages (Language Override).

The `generate:` mode SHALL additionally instruct the agent to self-validate and self-repair before emitting its final body: after drafting the quiz, the agent SHALL invoke `codebus quiz validate` on its draft via its Bash tool, SHALL correct the questions reported by the findings, and SHALL re-run the validator, repeating up to a fixed internal iteration cap stated explicitly in the SKILL body; when the cap is reached the agent SHALL emit its best current body rather than looping further. The SKILL SHALL reference the validator as the authority for structural and citation correctness and SHALL NOT restate the validator rule definitions (no parallel schema copy); it SHALL describe acting on the validator findings, not the rules themselves.

The SKILL SHALL ALSO define a third prompt mode `verify:` selected by the prompt prefix. The `verify:` mode SHALL instruct the agent to read the supplied planned pages plus a generated quiz body and judge each question against exactly five content defect types — answer-wrong (marked option not supported as correct by the planned pages), out-of-scope (a claim the planned pages do not state), not-exactly-one-correct (multiple defensibly-correct options or the marked one wrong), degenerate-distractor (a non-discriminating distractor), and off-topic (not about the supplied topic; evaluated only when a topic is supplied) — and to emit, for each flagged question, its question number, the defect type, and a concrete correction suggestion. The `verify:` mode SHALL NOT restate the deterministic validator structural/citation rules, and the SKILL SHALL keep the deterministic `codebus quiz validate` structural check separate from this content judgement.

The `verify:` mode SHALL additionally contain an explicit output-termination boundary instructing the agent to STOP after the last `Q<n> | <defect-type> | <suggestion>` line (or after `CONTENT_OK`), and to emit no further prose, rationale, evaluation summary, or per-question commentary. The SKILL body SHALL state this boundary as a normative MUST/SHALL clause, parallel in shape to the `plan:` mode boundary that forbids any content before the `[CODEBUS_QUIZ_SCOPE]` line. This requirement closes the prompt-surface-review F78 finding (an empirical 2026-05-24 run observed the verify agent emitting `**Q1 evaluation** / **Q2 evaluation** / **Q3 evaluation** / 驗證: xxx 第 N 行` rationale paragraphs before the closing `CONTENT_OK`, a contract violation that contributed to the unparseable-verify-output incident even though the line-by-line splitn parser silently skipped most prose).

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

<!-- @trace
source: quiz-content-verify, prompt-surface-output-discipline-batch
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

### Requirement: Codebus-Goal Verify Mode

The `codebus-goal` SKILL.md SHALL define a `verify:` prompt mode (selected by the prompt prefix), distinct from its normal ingest workflow, used by the independent content-verify spawn of the `verb-library` capability `Goal Content Verification and Repair` requirement. The `verify:` mode SHALL instruct the agent to read the supplied changed `wiki/` pages plus the originating goal, and — for grounding the faithfulness check — SHALL explicitly permit reading the `raw/code/` source mirror (read only, for verification; the agent SHALL NOT emit `raw/` contents, only defect judgements). It SHALL instruct judging each changed page against exactly three content defect types — **unfaithful** (a claim not grounded in / contradicting `raw/code/`), **off-goal** (content unrelated to this run goal), and **taxonomy-misplaced** (content in the wrong page type or folder) — and emitting, for each flagged page, one line `<wiki-relative-path> | <defect-type> | <concrete correction suggestion>`, or exactly `CONTENT_OK` when no page has a defect. The `verify:` mode SHALL NOT restate the deterministic lint rules; the structural lint / fix loop remains a separate concern.

The `verify:` mode SHALL additionally contain an explicit output-termination boundary instructing the agent to STOP after the last `<wiki-relative-path> | <defect-type> | <suggestion>` line (or after `CONTENT_OK`), and to emit no further prose, rationale, evaluation summary, or per-page commentary. The SKILL body SHALL state this boundary as a normative MUST/SHALL clause. This requirement closes the prompt-surface-review F38 finding (an empirical 2026-05-24 run observed the goal verify agent emitting `已完成所有變更頁面與 raw/code/src/db.py 原始碼的比對。` prose immediately before the closing `CONTENT_OK`; the current line-by-line parser tolerated it but the contract was violated and the behavior is unpredictable across runs).

#### Scenario: Goal bundle defines the verify mode and three-item contract

- **WHEN** the `codebus-goal/SKILL.md` is materialized
- **THEN** it SHALL define a `verify:` mode that judges changed pages against the three defect types (unfaithful, off-goal, taxonomy-misplaced) AND requires per-page `path | defect-type | suggestion` output or `CONTENT_OK`

#### Scenario: Verify mode permits raw/code grounding reads

- **WHEN** the `codebus-goal/SKILL.md` is materialized
- **THEN** its `verify:` mode SHALL explicitly permit reading `raw/code/` for the faithfulness check AND SHALL forbid emitting `raw/` contents (only defect judgements)

#### Scenario: Verify mode does not duplicate lint rules

- **WHEN** the `codebus-goal/SKILL.md` is materialized
- **THEN** the `verify:` mode SHALL NOT contain a restated copy of the deterministic lint rule definitions

#### Scenario: Verify mode declares STOP boundary after defect lines

- **WHEN** the `codebus-goal/SKILL.md` is materialized
- **THEN** the `verify:` mode body SHALL contain a normative clause instructing the agent to STOP after the last `<wiki-relative-path> | <defect-type> | <suggestion>` line or after `CONTENT_OK` AND SHALL forbid emitting any further prose, rationale, evaluation summary, or per-page commentary

##### Example: STOP boundary shape

- **GIVEN** the SKILL body for the goal `verify:` mode
- **WHEN** a reader looks for output-termination language
- **THEN** the body SHALL contain a sentence stating substantively that after the last `<wiki-relative-path> | <defect-type> | <suggestion>` line (or after `CONTENT_OK`) the agent SHALL stop emitting content AND SHALL NOT emit further prose, rationale, or summary

<!-- @trace
source: goal-content-verify, prompt-surface-output-discipline-batch
updated: 2026-05-24
code:
  - codebus-core/src/config/mod.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/verb/mod.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/verb/content_verify.rs
  - codebus-core/src/git/nested_repo.rs
  - codebus-core/src/skill_bundle/mod.rs
tests:
  - codebus-core/tests/verb_library_surface.rs
-->
