## ADDED Requirements

### Requirement: Codebus-Goal Verify Mode

The `codebus-goal` SKILL.md SHALL define a `verify:` prompt mode (selected by the prompt prefix), distinct from its normal ingest workflow, used by the independent content-verify spawn of the `verb-library` capability's `Goal Content Verification and Repair` requirement. The `verify:` mode SHALL instruct the agent to read the supplied changed `wiki/` pages plus the originating goal, and — for grounding the faithfulness check — SHALL explicitly permit reading the `raw/code/` source mirror (read only, for verification; the agent SHALL NOT emit `raw/` contents, only defect judgements). It SHALL instruct judging each changed page against exactly three content defect types — **unfaithful** (a claim not grounded in / contradicting `raw/code/`), **off-goal** (content unrelated to this run's goal), and **taxonomy-misplaced** (content in the wrong page type or folder) — and emitting, for each flagged page, one line `<wiki-relative-path> | <defect-type> | <concrete correction suggestion>`, or exactly `CONTENT_OK` when no page has a defect. The `verify:` mode SHALL NOT restate the deterministic lint rules; the structural lint / fix loop remains a separate concern.

#### Scenario: Goal bundle defines the verify mode and three-item contract

- **WHEN** the `codebus-goal/SKILL.md` is materialized
- **THEN** it SHALL define a `verify:` mode that judges changed pages against the three defect types (unfaithful, off-goal, taxonomy-misplaced) AND requires per-page `path | defect-type | suggestion` output or `CONTENT_OK`

#### Scenario: Verify mode permits raw/code grounding reads

- **WHEN** the `codebus-goal/SKILL.md` is materialized
- **THEN** its `verify:` mode SHALL explicitly permit reading `raw/code/` for the faithfulness check AND SHALL forbid emitting `raw/` contents (only defect judgements)

#### Scenario: Verify mode does not duplicate lint rules

- **WHEN** the `codebus-goal/SKILL.md` is materialized
- **THEN** the `verify:` mode SHALL NOT contain a restated copy of the deterministic lint rule definitions
