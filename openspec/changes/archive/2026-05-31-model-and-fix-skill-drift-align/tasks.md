# Tasks

## 1. Part B — guard test first (TDD RED)

- [x] 1.1 In the skill_bundle test module of `codebus-core/src/skill_bundle/mod.rs` (near the existing `fix_workflow_*` tests), add ONE guard assertion that the `FIX_WORKFLOW` constant does not present `rule_id` as the agent-facing lint field name. Assert that `FIX_WORKFLOW` does NOT contain the code-span ``` `rule_id` ```. Rationale: the lint JSON the agent reads serializes that field under the key `rule` (`codebus-core/src/wiki/lint/output.rs` `JsonIssue.rule`; test `json_uses_rule_field_name_per_spec`); `rule_id` is only the internal Rust field name and must never leak into the SKILL prose. Run `cargo test -p codebus-core skill_bundle` and confirm the new test FAILS (RED) against the current `FIX_WORKFLOW` (which still says `rule_id`).

## 2. Part B — fix the drifted sentence at the source and both materialized files

- [x] 2.1 In the `FIX_WORKFLOW` constant in `codebus-core/src/skill_bundle/mod.rs` (the claude source of truth, step 3 "Apply repairs"), change the sentence `Issue \`rule_id\` selects the repair shape:` so the field name reads `rule` instead of `rule_id`. Change ONLY the field name on that sentence; leave the rest of the step and the per-rule bullet list verbatim. This is the source from which both the claude and the codex skill bodies are generated.
- [x] 2.2 Confirm the codex body stays consistent automatically: the codex fix body is derived from `FIX_WORKFLOW` by `claude_to_codex_translate` (the `CODEX_BODY_TRANSLATIONS` table), and that table has NO entry matching the `rule_id` sentence, so the edit in 2.1 flows through to the codex body unchanged and touches no translation `from` literal. Verify the codex translation drift-guard tests (`every_codex_translation_from_appears_in_a_claude_body`, `drift_guard_detects_unmatched_from`, `drift_guard_detects_leaked_claude_token`) still pass — do NOT add any translation table entry for this change.
- [x] 2.3 Hand-edit the already-materialized claude skill file `.codebus/.claude/skills/codebus-fix/SKILL.md` (step 3 "Apply repairs" sentence), changing `rule_id` to `rule`. Materialization is write-if-missing, so this existing file does not auto-update from 2.1.
- [x] 2.4 Hand-edit the already-materialized codex skill file `.codebus/.codex/skills/codebus-fix/SKILL.md` (same step 3 sentence), changing `rule_id` to `rule`. Same write-if-missing reason as 2.3.

## 3. Part A — align spec to code (spec delta done; code NOT touched)

- [x] 3.1 Re-read the `claude-code-config` spec delta at `openspec/changes/model-and-fix-skill-drift-align/specs/claude-code-config/spec.md` (created during propose: MODIFIED `Endpoint Profile Schema`, `System Profile Model Aliases`, `Azure Profile Model String Passthrough`). Confirm it describes the system `model` as a free-string alias (NOT a closed `SystemModel` enum), replaces the false reject scenarios with reality-matching ones, and does NOT reintroduce any closed-enum / closed-set claim on the system `model` field. Confirm `codebus-core/src/config/endpoint.rs` is left untouched by this change.
- [x] 3.2 Fix the remaining `SystemModel enum` claim in the live spec `Purpose` paragraph of `openspec/specs/claude-code-config/spec.md` (the capability `Purpose` is not carried by spec deltas, so edit the live file directly): change the phrase `the \`SystemModel\` enum and its \`--model\` flag mapping` to describe a free-string system `model` alias and its `--model` flag mapping. Touch only that phrase; leave the rest of the Purpose sentence verbatim.

## 4. Verify (no regression + guards green)

- [x] 4.1 Run `cargo test -p codebus-core skill_bundle`: the new guard assertion from task 1.1 now PASSES (GREEN), and the three codex translation drift-guard tests (`every_codex_translation_from_appears_in_a_claude_body`, `drift_guard_detects_unmatched_from`, `drift_guard_detects_leaked_claude_token`) still pass.
- [x] 4.2 Run `cargo test -p codebus-core wiki::lint::output`: `json_uses_rule_field_name_per_spec` and the other lint output serialization tests still pass.
- [x] 4.3 Run `cargo test -p codebus-core config::endpoint`: `system_model_accepts_arbitrary_string`, `system_model_to_cli_flag_future_and_passthrough`, and the endpoint config-load suite all still pass (confirming code was not changed and the spec now matches observed behavior).
- [x] 4.4 Run `spectra validate model-and-fix-skill-drift-align` and confirm the change validates clean.
