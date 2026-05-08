## MODIFIED Requirements

### Requirement: Spawn agent in query mode with Write/Edit excluded from toolset

The system SHALL spawn the LLM provider with cwd = `.codebus/` (same isolation as ingest) and SHALL omit `Write` and `Edit` from the `--tools` toolset whitelist so the agent cannot write files even within the vault. (See §3.2.4 of the design spec for why `--tools` is the toolset gate, not `--allowedTools`.)

When the resolved `ProviderConfig::ClaudeCli` carries a non-empty `model` value, the system SHALL append `--model <value>` to the spawned argv. When the resolved `ProviderConfig::ClaudeCli` carries a non-empty `effort` value, the system SHALL append `--effort <value>` to the spawned argv. When either field is `None`, the corresponding flag SHALL NOT appear in argv.

The system SHALL NOT pass any of the following sandbox-breaking flags under any combination of mode, model, or effort: `--add-dir`, `--allow-dangerously-skip-permissions`, `--dangerously-skip-permissions`.

#### Scenario: Required argv flags are present in query mode

- **WHEN** the system builds argv for query mode
- **THEN** argv contains `-p`, `--output-format stream-json`, `--input-format stream-json`, `--verbose`, `--permission-mode acceptEdits`, `--tools Read,Glob,Grep`, and `--allowedTools Read,Glob,Grep` (auto-approval safety net mirroring `--tools`)

#### Scenario: Provider spawn cwd matches vault root

- **WHEN** the system invokes the LLM provider for query mode against repo X
- **THEN** the spawn cwd equals `<X>/.codebus/`

#### Scenario: Model flag is injected in query mode when ClaudeCli config sets model

- **WHEN** the system builds argv for query mode with `ProviderConfig::ClaudeCli { model: Some("haiku"), ... }`
- **THEN** argv contains `--model haiku`

#### Scenario: Effort flag is injected in query mode when ClaudeCli config sets effort

- **WHEN** the system builds argv for query mode with `ProviderConfig::ClaudeCli { effort: Some("low"), ... }`
- **THEN** argv contains `--effort low`

#### Scenario: Model and effort flags are absent in query mode when config leaves them None

- **WHEN** the system builds argv for query mode with `ProviderConfig::ClaudeCli { model: None, effort: None, ... }`
- **THEN** argv contains neither `--model` nor `--effort`

#### Scenario: Forbidden sandbox-breaking flags never appear in query argv

- **WHEN** the system builds argv for query mode under any combination of `model` and `effort` values (set or unset)
- **THEN** argv contains none of `--add-dir`, `--allow-dangerously-skip-permissions`, `--dangerously-skip-permissions`
