## ADDED Requirements

### Requirement: Goal flow auto-runs lint_and_fix after ingest completes

When a `--goal` invocation finishes its ingest step (raw_sync, LLM invocation, source enrichment, stale detection, lint), the system SHALL invoke `lint_and_fix` against the vault before calling `auto_commit`, unless auto-fix is disabled by the resolved configuration. The fix loop's configuration (`enabled` flag and `max_iterations`) SHALL be resolved by combining `~/.codebus/config.yaml` `lint.auto_fix` defaults with any CLI override flags supplied for this invocation.

#### Scenario: Default goal run triggers the fix loop after lint

- **WHEN** the user runs `codebus --repo X --goal "Y"` with no override flags
- **AND** `~/.codebus/config.yaml` has no `lint.auto_fix` section or sets `lint.auto_fix.enabled: true`
- **THEN** the goal flow runs `lint_wiki` and then `lint_and_fix` against the vault before `auto_commit` is called
- **AND** `lint_and_fix` is given the same vault root, the same provider, and `max_iterations` resolved from config (default 5)

#### Scenario: --no-fix flag skips the fix loop in goal flow

- **WHEN** the user runs `codebus --repo X --goal "Y" --no-fix`
- **THEN** the goal flow runs `lint_wiki` but does NOT call `lint_and_fix`
- **AND** the goal flow proceeds directly to `auto_commit`

#### Scenario: Disabled config skips the fix loop in goal flow

- **WHEN** `~/.codebus/config.yaml` has `lint.auto_fix.enabled: false`
- **AND** the user runs `codebus --repo X --goal "Y"` with no override flags
- **THEN** the goal flow runs `lint_wiki` but does NOT call `lint_and_fix`
- **AND** the goal flow proceeds directly to `auto_commit`

#### Scenario: Auto-commit happens once after fix loop terminates

- **WHEN** the goal flow's fix loop runs to its terminal state (clean or max-iterations)
- **THEN** the system calls `auto_commit` exactly once after the fix loop returns
- **AND** the commit captures both the goal's wiki writes and any subsequent fix-loop edits in a single commit
