## ADDED Requirements

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
