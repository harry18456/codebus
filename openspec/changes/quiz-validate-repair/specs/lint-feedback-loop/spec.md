## MODIFIED Requirements

### Requirement: Fix Bash Hook Installation

The `codebus init` subcommand SHALL write a `<vault_root>/.claude/settings.json` file containing a Claude Code `PreToolUse` hook configuration that intercepts every `Bash` tool invocation and routes it through the `codebus hook check-bash` subcommand. The settings file SHALL use the standard Claude Code settings schema with `hooks.PreToolUse` configured to match `Bash` and invoke `codebus hook check-bash` as a `command`-type hook.

The system SHALL apply write-if-missing semantics for this file: if `<vault_root>/.claude/settings.json` already exists, init SHALL NOT modify it (preserving any user-customized hook chain or other settings). The file SHALL NOT be written to `<repo>/.claude/settings.json` (source repository root) — the settings are vault-internal so the hook only applies to agent processes spawned with cwd at the vault root.

The `codebus hook check-bash` subcommand SHALL implement the following stdin/stdout contract:

- **Input** (stdin): a single JSON object matching Claude Code's PreToolUse hook input schema. The relevant fields are `tool_name` (expected `"Bash"`) and `tool_input.command` (the shell command string the agent intends to run).
- **Allow**: when the command's first argv token resolves to a `codebus` binary (file basename `codebus` or `codebus.exe`, case-insensitive match) AND EITHER (a) the second argv token is exactly `lint`, OR (b) the second argv token is exactly `quiz` AND the third argv token is exactly `validate`, the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON to stdout. The two allowed forms correspond to the codebus-fix agent self-checking via `codebus lint` and the codebus-quiz generate agent self-validating via `codebus quiz validate` respectively; no other `codebus` subcommand and no other binary is permitted.
- **Block**: in all other cases (different binary, neither the `lint` nor the `quiz validate` form, malformed input, parse error), the subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` describes why the command was blocked.
- **Cross-platform**: the binary basename match SHALL be case-insensitive on Windows (`codebus.EXE` and `codebus.exe` both allowed) and case-sensitive on Unix.

The `<vault_root>/.gitignore` (vault internal) SHALL include the line `.claude/settings.local.json` so user-added local override settings are not committed to the vault git repository.

#### Scenario: Init writes settings.json on fresh vault

- **WHEN** `codebus init` runs against a repository with no existing `<vault_root>/.claude/settings.json`
- **THEN** the system SHALL create `<vault_root>/.claude/settings.json` AND the file content SHALL parse as JSON AND SHALL contain a `hooks.PreToolUse` array with a Bash matcher entry whose hook command invokes `codebus hook check-bash`

#### Scenario: Init does not overwrite existing settings.json

- **WHEN** `codebus init` runs against a vault where `<vault_root>/.claude/settings.json` already exists with custom content
- **THEN** the system SHALL NOT modify the existing file AND its byte-content SHALL be identical before and after init

#### Scenario: Init does not write settings.json to repo root

- **WHEN** `codebus init` runs against `<repo>`
- **THEN** the system SHALL NOT create or modify `<repo>/.claude/settings.json`

#### Scenario: hook check-bash allows bare codebus lint invocation

- **WHEN** `codebus hook check-bash` receives stdin JSON `{"tool_name":"Bash","tool_input":{"command":"codebus lint --format json"}}`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash allows codebus lint via absolute path

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` value is `/usr/local/bin/codebus lint --repo /path` OR (on Windows) `D:/dev/codebus.exe lint --format json`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash allows codebus quiz validate invocation

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus quiz validate -` OR `/usr/local/bin/codebus quiz validate draft.md --json`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash blocks non-codebus binaries

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `echo MARKER`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field is a non-empty string

#### Scenario: hook check-bash blocks codebus subcommands other than the two allowed forms

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus fix --no-fix` OR `codebus quiz "some topic"` (the generate form, not `quiz validate`)
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-bash fails closed on malformed input

- **WHEN** `codebus hook check-bash` receives stdin that does not parse as JSON
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (fail-closed default — never silently allow on parse failure)

#### Scenario: Vault internal gitignore excludes settings.local.json

- **WHEN** `codebus init` runs against `<repo>` and reaches the vault internal `.gitignore` mutation step
- **THEN** the file `<vault_root>/.gitignore` SHALL contain a line equal to `.claude/settings.local.json`
