## Why

Two artifacts (one spec, one agent prompt) have drifted from the code they describe — the same class of defect: the artifact claims behavior the code does not implement.

- **Part A** — the `claude-code-config` spec describes the system-profile `model` field as a closed `SystemModel` enum (four variants; unversioned / foreign aliases rejected at load). The code has no such enum: `codebus-core/src/config/endpoint.rs` declares `pub model: String` and `system_model_to_cli_flag` simply ensures a `claude-` prefix so a newly-released model needs no code change. The spec's reject claims are false — `model: gpt-4` and `model: haiku` both load (tests `system_model_accepts_arbitrary_string` and `system_model_to_cli_flag_future_and_passthrough` prove it).
- **Part B** — the `codebus-fix` skill body tells the agent "Issue `rule_id` selects the repair shape", but the lint JSON the agent actually reads serializes that field as `rule` (`codebus-core/src/wiki/lint/output.rs`: the `JsonIssue.rule` field; test `json_uses_rule_field_name_per_spec` asserts the JSON key is `rule`, not `rule_id`). `rule_id` is only the internal Rust field name. The agent is instructed to key on a field that never appears in the JSON.

## What Changes

- **Part A (spec only, code unchanged)**: Rewrite the `System Profile Model Aliases` requirement and correct the `Endpoint Profile Schema` requirement so the system `model` is described as a free-string alias (translated to the `--model` flag by ensuring a `claude-` prefix, forward-compatible with future models) rather than a closed enum. Replace the false `Invalid SystemModel value rejected` scenario with one reflecting reality (an arbitrary system model string loads), and replace the false `Unversioned alias rejected` scenario with the true behavior (an unversioned alias loads and gets the `claude-` prefix). Correct the Purpose paragraph and the stale `SystemModel enum literal` phrase in the `Azure Profile Model String Passthrough` requirement.
- **Part B (prompt source + two materialized files + one guard test)**: Change the drifted sentence from `rule_id` to `rule` in the `FIX_WORKFLOW` constant in `codebus-core/src/skill_bundle/mod.rs` (the claude source of truth — the codex body is derived from it at materialization time via the translation table, which has no `rule_id` entry, so the same edit flows through to codex automatically and touches no translation `from` literal). Also hand-edit the two already-materialized skill files (`.claude` and `.codex`), which are write-if-missing and do not auto-update. Add one guard assertion in the skill_bundle tests that `FIX_WORKFLOW` does not present `rule_id` as the lint field name.

## Non-Goals

- **Do NOT reintroduce a `SystemModel` enum or any closed-set validation on the system `model` field** — that would break forward-compatibility with newly-released Claude models (the reason the gate was removed). Part A aligns the spec to the code, never the reverse; `codebus-core/src/config/endpoint.rs` is not touched.
- No change to azure model passthrough behavior, effort closed-set validation, or any config-loader code.
- No change to lint rule semantics or the internal Rust `rule_id` field name (only the agent-facing JSON field name `rule` and the skill prose that references it).
- No broad rewrite of the `codebus-fix` skill beyond the single drifted sentence.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `claude-code-config`: the `Endpoint Profile Schema`, `System Profile Model Aliases`, and `Azure Profile Model String Passthrough` requirements are reworded so the system `model` field is a free-form string alias (not a closed `SystemModel` enum), and the false reject scenarios are replaced with reality-matching ones.

## Success Criteria

- The `claude-code-config` spec no longer claims `SystemModel` is a closed enum or that arbitrary / unversioned system model strings are rejected; it describes the free-string + `claude-` prefix forward-compat behavior, consistent with `system_model_accepts_arbitrary_string`.
- `codebus-core/src/config/endpoint.rs` is unchanged; `system_model_accepts_arbitrary_string` and `system_model_to_cli_flag_future_and_passthrough` still pass.
- The `FIX_WORKFLOW` source and both materialized `codebus-fix` skill files (`.codebus/.claude/...` and `.codebus/.codex/...`) say `rule` instead of `rule_id` on the drifted sentence.
- The codex body translation drift-guard tests (`every_codex_translation_from_appears_in_a_claude_body`, `drift_guard_detects_unmatched_from`, `drift_guard_detects_leaked_claude_token`) stay green — the edit touches no translation `from` literal.
- The lint output serialization test (`json_uses_rule_field_name_per_spec`) still passes, and the new guard assertion fails if `FIX_WORKFLOW` reintroduces `rule_id` as the lint field name.

## Impact

- Affected specs: `claude-code-config` (MODIFIED — `Endpoint Profile Schema`, `System Profile Model Aliases`, `Azure Profile Model String Passthrough`)
- Affected code:
  - Modified: openspec/specs/claude-code-config/spec.md
  - Modified: codebus-core/src/skill_bundle/mod.rs
  - Modified: .codebus/.claude/skills/codebus-fix/SKILL.md
  - Modified: .codebus/.codex/skills/codebus-fix/SKILL.md
  - New: one guard test in the skill_bundle test module of codebus-core/src/skill_bundle/mod.rs
