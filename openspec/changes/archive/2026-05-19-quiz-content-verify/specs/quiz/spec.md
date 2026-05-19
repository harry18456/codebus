## ADDED Requirements

### Requirement: Quiz Content Verification and Repair

The system SHALL provide an optional model-based content verification stage for generated quizzes, gated by a `quiz.content_verify` configuration key (boolean, default `false`). When the key is absent or `false`, `run_quiz_generate` SHALL behave exactly as without this requirement (no verify spawn, no `content_review` frontmatter). The `codebus` CLI SHALL read `quiz.content_verify` from the shared `quiz.*` namespace and SHALL NOT read the app-only `app.*` namespace for it.

When `quiz.content_verify` is `true`, after the deterministic final-verify completes, `run_quiz_generate` SHALL run one **independent verify spawn** (a separate agent from the generate spawn) that reads the planned pages plus the generated quiz body and judges each question against exactly this five-item defect contract, emitting per flagged question its question number, defect type, and a concrete correction suggestion:

1. **answer-wrong** — the marked correct option is not supported as correct by the planned pages.
2. **out-of-scope** — the stem, an option, or the explanation asserts something the planned pages do not state.
3. **not-exactly-one-correct** — more than one option is defensibly correct, or the marked option is not correct.
4. **degenerate-distractor** — a distractor is non-discriminating (blank, a "none/all of the above" cop-out, or absurd).
5. **off-topic** — the question is not about the user's requested topic. This item SHALL be judged ONLY when an originating topic is supplied (the Goal flow); when no topic is supplied (the Page flow) item 5 SHALL be skipped and the other four SHALL still be judged.

`run_quiz_generate` SHALL accept the originating topic as an optional input; the Goal flow SHALL supply it and the Page flow SHALL supply none. Verify findings SHALL be emitted through the same event fan-out used by the generate spawn (one `events.jsonl` + `on_event` for the run) in the existing lint-finding event shape, and SHALL drive a caller-orchestrated repair loop: when the verify spawn reports defects, a repair spawn (reusing the generate mode, given the same pages + count + the previous quiz body + the verify defect list, instructed to revise only the flagged questions and keep the question count) SHALL produce a revised body, which SHALL then be re-verified. This verify→repair→re-verify loop SHALL be bounded by a fixed caller-counted cap of 3 iterations; when the cap is reached the current best body SHALL be emitted rather than looping further. This is a new caller-orchestrated mechanism (an independent verify model judging, then a bounded repair) — not the Stage-1 intra-spawn agent self-repair.

The persisted quiz's caller frontmatter SHALL carry a `content_review` field whose value is `ok` when the final verify reported zero defects and `flagged` otherwise; when `flagged`, the persisted frontmatter SHALL also list the flagged question numbers. Residual defects after the cap SHALL be best-effort: a non-fatal warning SHALL be emitted, no question SHALL be dropped, and `run_quiz_generate` SHALL NOT return a `VerbError` solely because content defects remain (exit semantics unchanged). A verify spawn failure or unparseable verify output SHALL be treated as non-fatal: a warning SHALL be emitted, the quiz SHALL be persisted with `content_review: flagged` (never silently `ok`), and the verb SHALL NOT fail solely for that reason. An absent `content_review` field SHALL be read as "content not verified" and SHALL NOT be treated as `ok`.

The verify spawn SHALL be read-only (no Bash, no validator invocation) — content judgement is distinct from the deterministic `codebus quiz validate` structural/citation check, which this requirement does not change.

#### Scenario: Content verify disabled by default

- **WHEN** `run_quiz_generate` runs and `quiz.content_verify` is absent or `false`
- **THEN** no verify spawn SHALL run AND the persisted quiz SHALL NOT contain a `content_review` field

#### Scenario: Clean content marks ok

- **WHEN** `quiz.content_verify` is `true` and the verify spawn reports zero defects
- **THEN** the persisted quiz frontmatter SHALL be `content_review: ok` AND no content warning SHALL be emitted

#### Scenario: Defect triggers bounded repair then marks state

- **WHEN** `quiz.content_verify` is `true` and the verify spawn flags question 3 as answer-wrong
- **THEN** question 3's defect SHALL be fed back to the generate agent for revision AND the stage SHALL re-verify, repeating at most the fixed cap of iterations AND the final persisted frontmatter SHALL be `content_review: ok` if cleared or `content_review: flagged` listing the still-flagged question numbers if not

#### Scenario: Residual defects are best-effort

- **WHEN** defects remain after the iteration cap is reached
- **THEN** the quiz SHALL be persisted with `content_review: flagged` and the flagged question numbers AND a non-fatal warning SHALL be emitted AND no question SHALL be dropped AND the verb exit status SHALL be unchanged

#### Scenario: Off-topic item is Goal-flow only

- **WHEN** the verify spawn runs for a Page-flow generation (no originating topic supplied)
- **THEN** the off-topic item SHALL NOT be evaluated AND the other four defect items SHALL still be evaluated

#### Scenario: Verify infrastructure failure is non-fatal

- **WHEN** the verify spawn fails or its output cannot be parsed
- **THEN** a warning SHALL be emitted AND the quiz SHALL be persisted with `content_review: flagged` AND `run_quiz_generate` SHALL NOT fail solely for that reason
