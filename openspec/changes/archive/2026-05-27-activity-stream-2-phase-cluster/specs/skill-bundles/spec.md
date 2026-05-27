## ADDED Requirements

### Requirement: Codebus-Goal Shell Tool Classification Emission

The `codebus-goal` skill bundle SKILL.md SHALL define an emission contract requiring every emitted `Bash` / `Shell` tool-use event to carry a `tool_kind` field classifying the call into one of five enum values defined by the agent-stream-rendering capability: `read`, `inspect`, `mutation`, `other_read`, `other_write`.

The contract SHALL state the semantic of each value (mirroring the agent-stream-rendering Stream Event Tool Classification table) and SHALL state that when the skill cannot determine the intent of a shell call (e.g. a heuristic-derived custom command not on the canonical table), it SHALL default to `inspect` as the safest unknown. The skill bundle MUST NOT default to `mutation` for unknown calls because that would mis-place the call under the WRITING WIKI cluster.

The contract SHALL apply only to the `codebus-goal` bundle. The `codebus-quiz` skill SHALL be explicitly excluded because it does not exist in the production `.codebus/.claude/skills/` tree (grep verification 2026-05-27 confirmed presence only in `docs/spike-artifacts/quiz-fixture-vault/.claude/skills/`).

The emission contract SHALL NOT prescribe runtime enforcement on the codebus-core side beyond the optional-field parser behavior defined in agent-stream-rendering. Skills that fail to emit `tool_kind` SHALL gracefully degrade to the Inspect fallback on the consumer side and SHALL NOT cause stream rejection.

#### Scenario: codebus-goal SKILL.md documents the tool_kind contract

- **WHEN** an implementer reads `.codebus/.claude/skills/codebus-goal/SKILL.md` after the change is applied
- **THEN** the file SHALL contain a section that names the five `tool_kind` values, gives at least one example command per value, AND states explicitly that `inspect` is the safe default when intent cannot be determined

#### Scenario: codebus-quiz is not modified

- **WHEN** the change is applied
- **THEN** no file under `.codebus/.claude/skills/codebus-quiz/` SHALL be created or modified AND files under `docs/spike-artifacts/quiz-fixture-vault/.claude/skills/codebus-quiz/` SHALL NOT be modified
