## ADDED Requirements

### Requirement: Final Quiz Body Extraction Deduplicates Self-Validate Drafts

When the generating agent self-validates (the claude Mode B workflow), it emits the quiz body more than once in a single turn — a draft before invoking the validator and a final (possibly revised) body after — and `run_quiz_generate` accumulates the agent's entire assistant text stream into one body string. The system SHALL extract ONLY the final quiz body from that accumulated text before persistence, so a correct N-question quiz is never persisted as a duplicated 2N (or draft-plus-final) body.

The system SHALL treat the LAST `## Q1.` header (a question whose number is exactly `1`) in the accumulated body as the start of the final body, and SHALL discard everything before it. When the accumulated text contains no `## Q1.` header, the system SHALL fall back to stripping only the preamble before the first question heading (the prior single-emission behavior), so a single-emission body and the codex path — which emits the body once and therefore has no draft copy — are unchanged. This extraction is deterministic and independent of agent compliance; it runs on both the generate and the content-repair body paths.

#### Scenario: Draft-plus-final emission persists only the final body

- **WHEN** the accumulated generate body contains a complete draft quiz body (numbered from `## Q1.`) followed by non-question text and then a final quiz body (again numbered from `## Q1.`)
- **THEN** the extracted body SHALL begin at the final body's `## Q1.` AND SHALL contain only the final body's question blocks AND SHALL NOT contain any question block from the draft copy

##### Example: five-question quiz emitted twice

- **GIVEN** the agent emitted a 5-question draft, then ran `codebus quiz validate`, then re-emitted the same 5 questions as the final body (total `## Q` headers in the accumulated text: 10)
- **WHEN** the final body is extracted
- **THEN** the persisted body SHALL contain exactly 5 `## Q` question blocks (the final copy), not 10

#### Scenario: Single emission is unchanged

- **WHEN** the accumulated body contains exactly one quiz body (one `## Q1.` header)
- **THEN** the extracted body SHALL equal the prior preamble-stripped body (no behavioral change for single-emission generate runs or the codex path)

## MODIFIED Requirements

### Requirement: Quiz Output Validation and Repair

The system SHALL validate a generated quiz markdown body before it is persisted, and SHALL drive deterministic self-repair through the generating agent (trust-agent model), mirroring the single-spawn / agent-internal-loop / caller-final-verifier pattern the `lint-feedback-loop` capability defines for `codebus fix`.

A deterministic quiz validator SHALL be the sole authority for structural correctness. It SHALL accept a quiz markdown body AND an optional expected question count, and SHALL produce findings of severity `error` for each of:

1. **Schema findings** — a question block (delimited by `## Q<n>.`) whose stem is empty, OR that does not have exactly the four choice keys `A`, `B`, `C`, `D`, OR that has no `## Answer: X` line where `X` is one of `A`/`B`/`C`/`D`, OR that has no `## Explanation:` line. Surrounding whitespace and blank lines SHALL be tolerated; the structural requirements SHALL NOT be.
2. **Wikilink-existence findings** — a `[[slug]]` citation appearing in any `## Explanation` that does not resolve to an existing page in the vault wiki index.
3. **Question-count findings** — when an expected question count is supplied AND the number of `## Q<n>.` question blocks in the body differs from it, the validator SHALL produce exactly one body-level `error` finding stating the expected and the actual count. When no expected question count is supplied, the validator SHALL NOT produce a question-count finding (so callers that do not know the intended count are unaffected). This finding shares the same finding shape as the schema and wikilink-existence findings so it flows through the same agent self-repair feedback path and the same caller final-verify pipeline without a parallel mechanism.

`run_quiz_generate` SHALL, after the generate spawn returns and the body is fence/preamble-stripped, run the deterministic validator exactly once as the final verifier, supplying the run's requested question count as the expected count. Validator findings SHALL be emitted through the same event fan-out used by the generate spawn (the same single `events.jsonl` sink and `on_event` callback for the run), using the existing lint-finding event shape so the CLI renderer, the GUI live stream, and the persisted-events replay surface them identically. The generate spawn, the agent's internal self-validate/self-repair iterations, and the final verify SHALL be one run with one `RunLog`.

When the final verifier still reports `error` findings, the system SHALL persist the quiz best-effort: the caller-injected frontmatter SHALL carry a `validation:` field whose value is `ok` when the final verifier reported zero `error` findings and `failed` otherwise; a non-fatal warning event SHALL be emitted; no question block SHALL be dropped; and `run_quiz_generate` SHALL NOT return a `VerbError` solely because validation failed. An absent `validation:` field SHALL be interpreted by readers as "not validated" and SHALL NOT be treated as `ok`. A residual question-count mismatch SHALL be surfaced through this same `validation: failed` marker; the verb SHALL NOT drop or synthesize question blocks to force the count (count enforcement is the agent self-repair loop's responsibility, not the caller's).

A validator internal error or an unreadable wiki index SHALL be treated as non-fatal: a warning event SHALL be emitted, the quiz SHALL still be persisted with `validation: failed`, and `run_quiz_generate` SHALL NOT fail solely for that reason. Spawn failure and cancellation semantics SHALL remain unchanged from the pre-existing `run_quiz_generate` contract.

The deterministic validator SHALL be the single source of truth for structural rules; the codebus-quiz SKILL SHALL NOT embed a parallel copy of the rule definitions (it references the validator and acts on its findings — see `skill-bundles`). Model-based content verification (a separate verify spawn) is OUT OF SCOPE for this requirement; the self-repair feedback path SHALL accept findings of the same shape regardless of source so a future model-verify stage can feed into it without rework.

#### Scenario: Generated quiz with a malformed question is flagged

- **WHEN** a generated quiz body contains a question block missing its `## Answer:` line
- **THEN** the deterministic validator SHALL produce an `error` finding identifying that question AND the finding SHALL be emitted through the run's event fan-out in the lint-finding event shape

#### Scenario: Broken explanation citation is flagged

- **WHEN** a generated quiz explanation cites `[[no-such-page]]` and no page named `no-such-page` exists in the vault wiki index
- **THEN** the deterministic validator SHALL produce an `error` finding for that citation

#### Scenario: Question count mismatch is flagged when an expected count is supplied

- **WHEN** the deterministic validator runs over a body containing nine `## Q<n>.` blocks AND the expected question count supplied is five
- **THEN** the validator SHALL produce exactly one body-level `error` question-count finding whose message states the expected count (five) and the actual count (nine)

#### Scenario: Matching question count produces no count finding

- **WHEN** the deterministic validator runs over a body containing five `## Q<n>.` blocks AND the expected question count supplied is five
- **THEN** the validator SHALL produce no question-count finding

#### Scenario: No expected count supplied skips the count check

- **WHEN** the deterministic validator runs over a body containing nine `## Q<n>.` blocks AND no expected question count is supplied
- **THEN** the validator SHALL produce no question-count finding (the body is judged only on schema and wikilink-existence)

#### Scenario: Clean quiz validates and is persisted as ok

- **WHEN** a generated quiz passes every schema check, all `[[slug]]` citations resolve, AND its question count equals the run's requested count
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
