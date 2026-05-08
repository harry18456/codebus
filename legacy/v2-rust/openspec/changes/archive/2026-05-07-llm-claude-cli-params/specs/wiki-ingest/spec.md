## MODIFIED Requirements

### Requirement: Spawn LLM agent with sandbox flags and cwd isolation

The system SHALL spawn the LLM provider with cwd set to `.codebus/` (system-level user-source-repo isolation) and SHALL pass `--tools` (toolset whitelist gate) and `--allowedTools` (auto-approval safety net) with identical lists so that only `Read,Glob,Grep,Write,Edit` are visible to the agent — Bash/WebFetch/WebSearch/AskUserQuestion/Task/NotebookEdit/MCP and any future Claude Code tools are not in the agent's toolbox at all. (See §3.2.4 of the design spec for why `--tools` is the gate, not `--allowedTools`; `--allowedTools` was misused as a toolset filter in iter-1 ~ iter-8.)

When the resolved `ProviderConfig::ClaudeCli` carries a non-empty `model` value, the system SHALL append `--model <value>` to the spawned argv. When the resolved `ProviderConfig::ClaudeCli` carries a non-empty `effort` value, the system SHALL append `--effort <value>` to the spawned argv. When either field is `None`, the corresponding flag SHALL NOT appear in argv (the Claude CLI's default for that flag applies).

The system SHALL NOT pass any of the following sandbox-breaking flags under any combination of mode, model, or effort: `--add-dir`, `--allow-dangerously-skip-permissions`, `--dangerously-skip-permissions`. These flags widen the agent's filesystem reach or bypass permission gates and are pinned out by an architecture-level negative assertion.

#### Scenario: Provider spawn receives required cwd

- **WHEN** the system invokes the LLM provider for ingest mode against repo X
- **THEN** the spawn cwd equals `<X>/.codebus/`, so the agent cannot write outside the vault without explicit permission grant

#### Scenario: Required argv flags are present in ingest mode

- **WHEN** the system builds argv for ingest mode
- **THEN** argv contains `-p`, `--output-format stream-json`, `--input-format stream-json`, `--verbose`, `--permission-mode acceptEdits`, `--tools Read,Glob,Grep,Write,Edit`, and `--allowedTools Read,Glob,Grep,Write,Edit`

#### Scenario: Model flag is injected when ClaudeCli config sets model

- **WHEN** the system builds argv for ingest mode with `ProviderConfig::ClaudeCli { model: Some("sonnet"), ... }`
- **THEN** argv contains `--model sonnet`

#### Scenario: Effort flag is injected when ClaudeCli config sets effort

- **WHEN** the system builds argv for ingest mode with `ProviderConfig::ClaudeCli { effort: Some("high"), ... }`
- **THEN** argv contains `--effort high`

#### Scenario: Model and effort flags are absent when config leaves them None

- **WHEN** the system builds argv for ingest mode with `ProviderConfig::ClaudeCli { model: None, effort: None, ... }`
- **THEN** argv contains neither `--model` nor `--effort`, so the Claude CLI defaults take effect

#### Scenario: Forbidden sandbox-breaking flags never appear in argv

- **WHEN** the system builds argv for ingest mode under any combination of `model` and `effort` values (set or unset)
- **THEN** argv contains none of `--add-dir`, `--allow-dangerously-skip-permissions`, `--dangerously-skip-permissions`
