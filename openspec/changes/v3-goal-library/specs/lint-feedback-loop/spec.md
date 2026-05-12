## ADDED Requirements

### Requirement: Fix Loop Library Invocation Entry Point

The single-shot fix loop SHALL be reachable from two entry points: (a) `codebus_core::verb::fix::run_fix` (the `fix` verb library function defined by the `verb-library` capability) AND (b) `codebus_core::verb::goal::run_goal` (the goal verb's post-agent lint-and-fix phase, when `GoalOptions.no_fix` is false). Both entry points SHALL invoke the same `codebus_core::wiki::fix::run_fix_loop` primitive with identical semantics — the existing fix loop behavior contracts in this capability (Fix Loop Agent Sandbox, Fix Single-Shot Verification, Fix Loop Configuration, Fix Bash Hook Installation, Standalone Fix Mode) SHALL apply unchanged at both entry points.

The CLI subcommand handlers in `codebus-cli/src/commands/{goal,fix}.rs` SHALL NOT invoke `run_fix_loop` directly. Direct invocation of `run_fix_loop` from the CLI binary crate SHALL be forbidden — the only callers SHALL be the verb library functions in `codebus_core::verb`. This delegation contract preserves a single source of truth for the fix loop's orchestration shape so future GUI callers (e.g., `v3-app-workspace-goal`) reuse identical behavior without re-implementing the spawn, hook installation, and verification sequence.

The fix loop's caller-observable behavior — Bash tool gated to `codebus lint *`, single-shot agent spawn, CLI as final-only verifier, `auto_commit` message strings — SHALL remain byte-equivalent after this change.

#### Scenario: Fix loop reachable from run_fix library function

- **WHEN** `codebus_core::verb::fix::run_fix` is invoked AND the lint pre-check finds at least one issue
- **THEN** the function SHALL invoke `codebus_core::wiki::fix::run_fix_loop` exactly once AND the loop's behavior contracts (Bash hook gate, agent toolset, final-only lint verification) SHALL be identical to the behavior reachable via `codebus fix` CLI invocation

#### Scenario: Fix loop reachable from run_goal library function

- **WHEN** `codebus_core::verb::goal::run_goal` is invoked with `GoalOptions { no_fix: false, .. }` AND the post-agent lint detects at least one issue AND `lint.fix.enabled` is true in config
- **THEN** the function SHALL invoke `codebus_core::wiki::fix::run_fix_loop` exactly once after the goal agent terminates AND before the final auto-commit step

#### Scenario: CLI binary does not call run_fix_loop directly

- **WHEN** a static search is performed across `codebus-cli/src/**/*.rs` for direct references to `codebus_core::wiki::fix::run_fix_loop`
- **THEN** the search SHALL return zero matches (the only callers SHALL be inside `codebus-core/src/verb/{goal,fix}.rs`)
