## MODIFIED Requirements

### Requirement: Subcommand Registration

The `codebus` binary SHALL register nine subcommands at the top level: `init`, `goal`, `query`, `lint`, `fix`, `config`, `chat`, `quiz`, `mcp`. The `mcp` subcommand SHALL be gated behind the default-on `mcp` cargo feature; when that feature is disabled at build time the binary SHALL register only the other eight. No other subcommand SHALL be registered. The `--help` and `--version` flags SHALL be available at both the binary level and per subcommand. The `config` subcommand SHALL itself expose three sub-actions (`set-key`, `get-key`, `delete-key`); the sub-action contract is defined normatively in the `claude-code-config` capability. The `quiz` subcommand SHALL itself expose a `validate` sub-action (`codebus quiz validate <quiz-md-file | -> [--count <N>] [--json]`); the sub-action contract is defined normatively in the `Quiz Validate Sub-Action Behavior` requirement of this capability and the `Quiz Output Validation and Repair` requirement of the `quiz` capability. The `chat` subcommand contract is defined normatively in the `Chat Subcommand Behavior` requirement of this capability and the `Chat CLI Subcommand Behavior` requirement of the `chat-verb` capability. The `quiz` subcommand contract is defined normatively in the `Quiz Subcommand Behavior` requirement of this capability and the `quiz` capability. The `mcp` subcommand contract is defined normatively in the `mcp-server` capability.

#### Scenario: Help output lists the eight always-on subcommands plus feature-gated mcp

- **WHEN** `codebus --help` is invoked in a default build
- **THEN** the help output SHALL list `init`, `goal`, `query`, `lint`, `fix`, `config`, `chat`, `quiz`, `mcp` as the only subcommands AND SHALL exit with status zero

#### Scenario: Version flag prints cargo package version

- **WHEN** `codebus --version` is invoked
- **THEN** the output SHALL be the `codebus` cargo package version AND SHALL exit with status zero

#### Scenario: Quiz validate sub-action is registered under quiz

- **WHEN** `codebus quiz --help` is invoked
- **THEN** the help output SHALL document a `validate` sub-action AND the top-level `codebus --help` SHALL still list exactly the registered subcommands (the eight always-on plus `mcp` when its default-on feature is enabled) with no further undocumented top-level subcommand
