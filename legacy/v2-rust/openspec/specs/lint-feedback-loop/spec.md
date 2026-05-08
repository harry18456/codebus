# lint-feedback-loop Specification

## Purpose

TBD - created by archiving change 'lint-feedback-loop'. Update Purpose after archive.

## Requirements

### Requirement: Provide a single lint-and-fix function shared by both entry points

The system SHALL expose a function `lint_and_fix(vault_root, provider, max_iterations)` in the `wiki/fix/` module that performs the full lint feedback loop. Both the `--goal` flow's auto-fix step and the standalone `--fix` mode SHALL call this same function so the loop's behavior is identical regardless of how it was triggered.

#### Scenario: Goal flow auto-fix and --fix mode invoke the same function

- **WHEN** the system runs the auto-fix step at the end of a `--goal` run
- **AND** the system runs the standalone `--fix` mode against the same vault state
- **THEN** both code paths invoke `lint_and_fix(vault_root, provider, max_iterations)` with the same vault state and configuration
- **AND** both produce a `FixReport` with the same shape regardless of entry point

---
### Requirement: Skip the loop entirely when initial lint reports zero issues

When `lint_and_fix` is invoked, the system SHALL run `lint_wiki(vault_root)` once before any LLM invocation. If the initial lint reports zero issues, the system SHALL return immediately with a clean report and SHALL NOT invoke the LLM provider, build a fix prompt, or perform any iteration.

#### Scenario: Clean vault produces zero LLM invocations

- **WHEN** `lint_and_fix` is invoked against a vault whose initial `lint_wiki` returns zero issues
- **THEN** the LLM provider's `invoke` is NOT called
- **AND** the function returns a `FixReport` indicating zero iterations and a clean state

---
### Requirement: Terminate when issues clear or max iterations reached

The fix loop SHALL terminate when either (a) the most recent `lint_wiki` call returns zero issues, or (b) the iteration count reaches the configured `max_iterations`. The system SHALL NOT add additional termination heuristics such as oscillation detection or rate-of-progress checks.

#### Scenario: Loop terminates with clean state when all issues are fixed

- **WHEN** `lint_and_fix` runs and an iteration's post-lint reports zero issues
- **THEN** the loop terminates immediately
- **AND** the returned `FixReport` indicates a clean terminal state with the iteration count it took

#### Scenario: Loop terminates by max iterations cap when issues remain

- **WHEN** `lint_and_fix` runs and the iteration counter reaches `max_iterations` while issues still remain
- **THEN** the loop terminates without invoking the LLM again
- **AND** the returned `FixReport` indicates a max-iterations terminal state and lists the remaining issues

#### Scenario: Loop never gives up due to oscillation

- **WHEN** the issue count rises or stays equal across two consecutive iterations but `max_iterations` is not yet reached
- **THEN** the loop continues to the next iteration
- **AND** the loop does not terminate solely because issue count failed to decrease

---
### Requirement: Per-iteration prompt batches all current issues with prior diff as memory

For each non-terminal iteration, the system SHALL build a single prompt that contains (a) the full current list of lint issues from the most recent `lint_wiki` call, and (b) for iterations after the first, a summary of `git diff` against the vault's `wiki/` subtree showing what changed during the previous iteration. The system SHALL invoke the provider exactly once per iteration with `LlmMode::Ingest`.

#### Scenario: First iteration has no previous-attempt block

- **WHEN** the loop runs its first iteration
- **THEN** the prompt includes all current lint issues
- **AND** the prompt does NOT include a `<previous_attempt>` block

#### Scenario: Subsequent iterations include diff against the snapshot taken at loop start

- **WHEN** the loop runs an iteration after the first
- **THEN** the prompt includes a `<previous_attempt>` block whose contents come from `git diff <snapshot_sha> -- wiki/` where `snapshot_sha` is the vault's HEAD captured before the loop's first iteration
- **AND** the prompt also includes the full current list of remaining lint issues

#### Scenario: Each iteration invokes the provider exactly once

- **WHEN** any non-terminal iteration runs
- **THEN** the system calls `provider.invoke(...)` exactly one time for that iteration

---
### Requirement: All lint rules participate in the fix loop

The fix loop SHALL forward every issue produced by `lint_wiki` to the LLM regardless of which rule produced it. The system SHALL NOT branch on rule name to apply deterministic fixes; rules including `duplicate_slug` and `unexpected_file` SHALL be addressed by the LLM in the same prompt as semantic rules.

#### Scenario: Duplicate slug issues are forwarded to the LLM

- **WHEN** lint reports a `duplicate_slug` issue
- **AND** the loop builds the iteration's prompt
- **THEN** the issue appears in the prompt's issues block exactly like any other rule's issue
- **AND** the system does NOT rename, delete, or move any file outside the LLM's invocation

#### Scenario: Unexpected-file issues are forwarded to the LLM

- **WHEN** lint reports an `unexpected_file` issue
- **AND** the loop builds the iteration's prompt
- **THEN** the issue appears in the prompt's issues block exactly like any other rule's issue
- **AND** the system does NOT move the file outside the LLM's invocation

---
### Requirement: LlmProvider trait is unchanged

The `lint_and_fix` implementation SHALL use only the existing `LlmProvider::invoke` method as defined before this change. The system SHALL NOT add session identifiers, conversation history parameters, or any other multi-turn extension to the trait. Any cross-iteration memory SHALL live entirely inside the prompt body assembled by `wiki/fix/prompt.rs`.

#### Scenario: Trait surface is preserved

- **WHEN** this change is fully implemented
- **THEN** the `LlmProvider` trait has the same methods and signatures as before this change
- **AND** the `InvokeOptions` struct has no fields added for session continuity or history

---
### Requirement: Auto-fix is configurable via global config and CLI overrides

The system SHALL read auto-fix settings from `~/.codebus/config.yaml` `lint.auto_fix.enabled` (boolean) and `lint.auto_fix.max_iterations` (positive integer). Defaults SHALL be `enabled: true` and `max_iterations: 5`. The CLI SHALL accept two override flags: `--no-fix` (forces `enabled` to false for this invocation regardless of config) and `--fix-max-iter <N>` (overrides `max_iterations` to N for this invocation).

#### Scenario: Default config enables fix with max iterations five

- **WHEN** `~/.codebus/config.yaml` is missing or has no `lint.auto_fix` section
- **AND** the user runs `codebus --goal "X"` with no override flags
- **THEN** the goal flow's auto-fix step runs with `enabled = true` and `max_iterations = 5`

#### Scenario: --no-fix flag disables fix even when config enables it

- **WHEN** `~/.codebus/config.yaml` has `lint.auto_fix.enabled: true`
- **AND** the user runs `codebus --goal "X" --no-fix`
- **THEN** the goal flow's auto-fix step is skipped
- **AND** `lint_and_fix` is NOT invoked

#### Scenario: --fix-max-iter overrides config max_iterations

- **WHEN** `~/.codebus/config.yaml` has `lint.auto_fix.max_iterations: 5`
- **AND** the user runs `codebus --goal "X" --fix-max-iter 10`
- **THEN** the loop uses `max_iterations = 10` for that invocation

#### Scenario: --no-fix wins when both flags are present

- **WHEN** the user runs `codebus --goal "X" --no-fix --fix-max-iter 10`
- **THEN** the goal flow's auto-fix step is skipped
- **AND** `--fix-max-iter` has no observable effect

---
### Requirement: Standalone --fix CLI mode targets existing vaults without ingest

The system SHALL accept a `--fix` CLI flag that, when supplied, runs `lint_and_fix` against the repo's existing vault and exits. The mode SHALL NOT perform `sync_repo_to_raw`, SHALL NOT invoke the LLM with goal-style prompts, and SHALL NOT modify any source code under the repo's working tree. After the loop completes, the system SHALL call `auto_commit` on the vault's nested git repo with a commit message identifying the fix loop run.

#### Scenario: --fix mode skips ingest

- **WHEN** the user runs `codebus --repo X --fix`
- **THEN** the system does NOT call `sync_repo_to_raw`
- **AND** the system does NOT call any goal-style provider invocation
- **AND** the system does call `lint_and_fix` against the existing vault

#### Scenario: --fix mode commits its results to the nested vault git repo

- **WHEN** `--fix` mode runs and the fix loop produces any change under `wiki/`
- **THEN** the system runs `auto_commit` on the vault's nested git repo
- **AND** the commit message identifies this as a lint fix loop run

#### Scenario: --fix mode requires an existing vault

- **WHEN** the user runs `codebus --repo X --fix`
- **AND** `<repo>/.codebus/` does not exist
- **THEN** the system writes a user-facing error to stderr suggesting `codebus init` or `codebus --goal "..."`
- **AND** the process exits with a non-zero exit code

---
### Requirement: --check mode is unchanged by this capability

The `--check` CLI mode SHALL remain a pure read operation that runs `lint_wiki` and emits its report. The system SHALL NOT invoke the LLM provider, run the fix loop, or modify any vault content during `--check`.

#### Scenario: --check stays read-only

- **WHEN** the user runs `codebus --repo X --check`
- **THEN** the system does NOT invoke `lint_and_fix`
- **AND** the system does NOT invoke any LLM provider
- **AND** the vault contents on disk are byte-for-byte identical before and after the run
