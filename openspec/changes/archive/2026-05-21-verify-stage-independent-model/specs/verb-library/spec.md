## MODIFIED Requirements

### Requirement: Goal Content Verification and Repair

The system SHALL provide an optional independent model-based content verification stage for the `goal` verb, sharing the verify→repair orchestration with the `quiz` verb through a common core. The orchestration core SHALL expose a content-review status type (values `ok`, or `flagged` with the list of still-flagged item identifiers), a verify-output parser (an explicit `CONTENT_OK` line yields no defects; lines of the form `<id> | <defect-type> | <suggestion>` yield per-item defects; output containing neither is unparseable), and a bounded repair loop (an independent verify step, then on defects a repair step fed those defects, then re-verify, hard-capped at three iterations with the best body kept when the cap is reached). The `quiz` verb's externally observable behavior — its persisted `content_review` value format, its events, its cap, and its best-effort semantics — SHALL be unchanged by this refactor.

`run_goal` SHALL run this stage **after the fix loop and before `auto_commit`**, gated by a `goal.content_verify` configuration key (boolean, default `false`). When the key is absent or `false`, `run_goal` SHALL behave exactly as without this requirement (no verify spawn, no content-review status). When `true`, `run_goal` SHALL determine the wiki pages this run created or modified by diffing the vault git repository against the revision captured before the goal agent spawn, restricted to the `wiki/` subtree. If no wiki page changed, the stage SHALL resolve to `ok` without spawning anything. Otherwise it SHALL run one independent, read-only verify spawn (permitted to read `raw/code/` for grounding) that judges each changed page against exactly this three-item defect contract:

1. **unfaithful** — the page asserts something not grounded in (or contradicting) the `raw/code/` source mirror.
2. **off-goal** — the page's content is unrelated to this run's goal.
3. **taxonomy-misplaced** — the content is in the wrong page type / folder.

The verify spawn SHALL resolve its model and effort via `cc_cfg.resolve(Verb::Verify)`, NOT `Verb::Goal` (`claude-code-config` Endpoint Profile Schema requirement defines the `Verb::Verify` resolution path). This ensures the verify spawn uses the dedicated `claude_code.system.verify` / `claude_code.azure.verify` sub-block, which is independent of the main goal spawn and the repair spawn that both continue to use `Verb::Goal`. The motivating use case is "expensive verification + cheaper main writing" (e.g., sonnet for goal main spawn, opus for verify).

On defects, `run_goal` SHALL run a Write-capable repair spawn fed the defect list, instructed to fix only the flagged pages in place, then re-verify; this loop SHALL be bounded by the shared cap of three iterations. The repair spawn SHALL resolve its model via `cc_cfg.resolve(Verb::Goal)` (NOT `Verb::Verify`) — the repair stage continues to use the same model as the original goal main spawn, so the cost profile is "verify with the dedicated verify model, repair with the same goal model used for main writing". Verify and repair events SHALL flow through the same event fan-out the goal spawn uses.

Residual defects after the cap SHALL be best-effort: a non-fatal warning SHALL be surfaced, no page SHALL be reverted, the `GoalReport` SHALL carry a content-review status (`ok` / `flagged` with pages / not-run), `run_goal` SHALL NOT return an error solely because content defects remain, the exit code SHALL be unchanged, and `auto_commit` SHALL still run (content verification SHALL NOT block the commit). A verify spawn failure or unparseable output SHALL be treated as non-fatal: a warning SHALL be surfaced and the status SHALL be `flagged` (never silently `ok`). An absent content-review status SHALL be read as "not verified" and SHALL NOT be treated as `ok`.

`run_goal` SHALL NOT emit verify-spawn model / effort metadata into the per-run `RunLog` entry. The `RunLog` `model` and `effort` fields SHALL continue to record the main goal spawn's model (`Verb::Goal` resolution); the verify spawn's model is observable via the `events.jsonl` per-run timeline (which already records every spawn's `SpawnStart` event including the model in use), but SHALL NOT appear in the consolidated `RunLog` row.

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

#### Scenario: Goal verify spawn uses Verb::Verify model not Verb::Goal

- **WHEN** `goal.content_verify` is `true`, `claude_code.system.goal` resolves to `model: sonnet-4-6`, AND `claude_code.system.verify` resolves to `model: opus-4-6`
- **THEN** the verify spawn SHALL be invoked with `--model claude-opus-4-6` AND the goal main spawn and the repair spawn SHALL be invoked with `--model claude-sonnet-4-6`

#### Scenario: Goal repair spawn uses Verb::Goal model not Verb::Verify

- **WHEN** `goal.content_verify` is `true`, the verify spawn flags a page, AND `claude_code.system.verify` resolves to `model: opus-4-6` while `claude_code.system.goal` resolves to `model: sonnet-4-6`
- **THEN** the repair spawn SHALL be invoked with `--model claude-sonnet-4-6` (the goal model, NOT the verify model) — repair keeps the same model profile as the goal main spawn

#### Scenario: Goal RunLog model field records main spawn not verify

- **WHEN** `goal.content_verify` is `true`, the main goal spawn resolves to `sonnet-4-6`, AND the verify spawn resolves to `opus-4-6`
- **THEN** the per-run `RunLog` entry SHALL record `model: claude-sonnet-4-6` (the main spawn's model) AND SHALL NOT record the verify spawn's model in any RunLog field

<!-- @trace
source: goal-content-verify, verify-stage-independent-model
updated: 2026-05-20
code:
  - codebus-core/src/config/mod.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/verb/mod.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/verb/content_verify.rs
  - codebus-core/src/git/nested_repo.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/git/mod.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/config/goal.rs
tests:
  - codebus-cli/tests/bins/mock_claude.rs
  - codebus-cli/tests/goal_content_verify_cli.rs
  - codebus-cli/tests/goal_flow.rs
-->
