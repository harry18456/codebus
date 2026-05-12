## MODIFIED Requirements

### Requirement: Subcommand Registration

The `codebus` binary SHALL register exactly six subcommands at the top level: `init`, `goal`, `query`, `lint`, `fix`, `config`. No other subcommand SHALL be registered. The `--help` and `--version` flags SHALL be available at both the binary level and per subcommand. The `config` subcommand SHALL itself expose three sub-actions (`set-key`, `get-key`, `delete-key`); the sub-action contract is defined normatively in the `claude-code-config` capability.

#### Scenario: Help output lists exactly the six subcommands

- **WHEN** `codebus --help` is invoked
- **THEN** the help output SHALL list `init`, `goal`, `query`, `lint`, `fix`, `config` as the only subcommands AND SHALL exit with status zero

#### Scenario: Version flag prints cargo package version

- **WHEN** `codebus --version` is invoked
- **THEN** the binary SHALL print a single line containing the cargo package version of the `codebus-cli` crate AND SHALL exit with status zero

#### Scenario: Unknown subcommand is rejected by clap

- **WHEN** `codebus mcp` or `codebus randomverb` is invoked
- **THEN** the binary SHALL print a clap error message to stderr identifying the unknown subcommand AND SHALL exit with non-zero status

#### Scenario: Config subcommand help lists its three actions

- **WHEN** `codebus config --help` is invoked
- **THEN** the help output SHALL list `set-key`, `get-key`, `delete-key` as the only sub-actions AND SHALL exit with status zero

## REMOVED Requirements

### Requirement: Claude Code Configuration Schema

**Reason**: Superseded by the `claude-code-config` capability's `Endpoint Profile Schema` requirement. The flat `claude_code.{goal,query,fix}.{model,effort}` shape is replaced by the profile-based schema with `active: system | azure` selector and per-profile blocks. `model` is no longer a free-string pass-through under the system profile (it is a closed `SystemModel` enum) and the defaults are versioned (`opus-4-6` / `haiku-4-5` / `sonnet-4-6`).
**Migration**: User config files containing the legacy top-level verb keys SHALL trigger the migration warning defined in `claude-code-config / Legacy Config Schema Warning Without Rewrite`. Code references to `ClaudeCodeConfig.goal.model` etc. SHALL be replaced by `ClaudeCodeConfig::resolve(Verb)` returning a `ResolvedVerb { model, effort }`.

### Requirement: Agent Spawn Model and Effort Forwarding

**Reason**: Superseded by `claude-code-config / System Profile Model Aliases` (system mode: enum → `to_cli_flag` translation), `claude-code-config / Azure Profile Model String Passthrough` (azure mode: verbatim deployment name), and `claude-code-config / Scoped Environment Injection At Spawn` (azure profile injects three additional env vars on the child process). The free-string forwarding contract is no longer accurate — system mode performs alias-to-flag translation and azure mode additionally injects env vars.
**Migration**: Verb command modules SHALL compose an `EnvOverrides` via `config::build_env_overrides(&ClaudeCodeConfig)` and pass both the resolved `model` / `effort` AND the `env` map into `InvokeAgentOptions`. Spawn-side `Command::env` injection is non-negotiable for the azure profile; system profile MUST pass an empty `EnvOverrides`.
