## ADDED Requirements

### Requirement: Goal Content Verification and Repair

The system SHALL provide an optional independent model-based content verification stage for the `goal` verb, sharing the verify→repair orchestration with the `quiz` verb through a common core. The orchestration core SHALL expose a content-review status type (values `ok`, or `flagged` with the list of still-flagged item identifiers), a verify-output parser (an explicit `CONTENT_OK` line yields no defects; lines of the form `<id> | <defect-type> | <suggestion>` yield per-item defects; output containing neither is unparseable), and a bounded repair loop (an independent verify step, then on defects a repair step fed those defects, then re-verify, hard-capped at three iterations with the best body kept when the cap is reached). The `quiz` verb's externally observable behavior — its persisted `content_review` value format, its events, its cap, and its best-effort semantics — SHALL be unchanged by this refactor.

`run_goal` SHALL run this stage **after the fix loop and before `auto_commit`**, gated by a `goal.content_verify` configuration key (boolean, default `false`). When the key is absent or `false`, `run_goal` SHALL behave exactly as without this requirement (no verify spawn, no content-review status). When `true`, `run_goal` SHALL determine the wiki pages this run created or modified by diffing the vault git repository against the revision captured before the goal agent spawn, restricted to the `wiki/` subtree. If no wiki page changed, the stage SHALL resolve to `ok` without spawning anything. Otherwise it SHALL run one independent, read-only verify spawn (permitted to read `raw/code/` for grounding) that judges each changed page against exactly this three-item defect contract:

1. **unfaithful** — the page asserts something not grounded in (or contradicting) the `raw/code/` source mirror.
2. **off-goal** — the page's content is unrelated to this run's goal.
3. **taxonomy-misplaced** — the content is in the wrong page type / folder.

On defects, `run_goal` SHALL run a Write-capable repair spawn fed the defect list, instructed to fix only the flagged pages in place, then re-verify; this loop SHALL be bounded by the shared cap of three iterations. Verify and repair events SHALL flow through the same event fan-out the goal spawn uses.

Residual defects after the cap SHALL be best-effort: a non-fatal warning SHALL be surfaced, no page SHALL be reverted, the `GoalReport` SHALL carry a content-review status (`ok` / `flagged` with pages / not-run), `run_goal` SHALL NOT return an error solely because content defects remain, the exit code SHALL be unchanged, and `auto_commit` SHALL still run (content verification SHALL NOT block the commit). A verify spawn failure or unparseable output SHALL be treated as non-fatal: a warning SHALL be surfaced and the status SHALL be `flagged` (never silently `ok`). An absent content-review status SHALL be read as "not verified" and SHALL NOT be treated as `ok`.

#### Scenario: Disabled by default

- **WHEN** `run_goal` runs and `goal.content_verify` is absent or `false`
- **THEN** no verify spawn SHALL run AND `GoalReport` SHALL carry no content-review status AND `auto_commit` and exit code SHALL be unchanged

#### Scenario: No changed wiki pages short-circuits

- **WHEN** `goal.content_verify` is `true` and the run modified no `wiki/` page
- **THEN** the stage SHALL resolve to `ok` without running any verify spawn

#### Scenario: Clean content marks ok

- **WHEN** `goal.content_verify` is `true` and the verify spawn reports `CONTENT_OK` for the changed pages
- **THEN** the `GoalReport` content-review status SHALL be `ok` AND no content warning SHALL be surfaced AND `auto_commit` SHALL run normally

#### Scenario: Defect triggers bounded repair

- **WHEN** the verify spawn flags a page as `unfaithful`
- **THEN** a repair spawn SHALL revise only the flagged page AND the stage SHALL re-verify, repeating at most three iterations AND the final content-review status SHALL be `ok` if cleared or `flagged` with the still-flagged pages otherwise

#### Scenario: Residual defects are best-effort and do not block commit

- **WHEN** defects remain after the iteration cap
- **THEN** a non-fatal warning SHALL be surfaced AND no page SHALL be reverted AND `run_goal` SHALL NOT return an error solely for this AND the exit code SHALL be unchanged AND `auto_commit` SHALL still run

#### Scenario: Quiz behavior unchanged by the shared refactor

- **WHEN** `run_quiz_generate` is exercised after the verify→repair orchestration is factored into the shared core
- **THEN** its persisted `content_review` value format, events, cap, and best-effort semantics SHALL be identical to before the refactor
