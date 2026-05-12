## ADDED Requirements

### Requirement: Config Parse Failure Aborts Invocation

Every `codebus` subcommand that reads `~/.codebus/config.yaml` SHALL distinguish three load outcomes and behave deterministically for each:

1. **File missing** (`io::ErrorKind::NotFound`) — the system SHALL apply the corresponding section's `Default::default()` and proceed with the invocation. This preserves first-time-setup ergonomics and is unchanged from the prior behavior.
2. **Load succeeds** — the system SHALL use the parsed configuration and proceed. Unknown keys SHALL remain silently ignored (forward-compat); fields absent from the document SHALL be filled with section defaults. Both behaviors are unchanged from the prior behavior.
3. **File exists but a config-section loader returns `Err`** (yaml syntax error, schema validation failure such as an invalid `SystemModel` enum value, missing required field under an active profile, or any other `ConfigLoadError`) — the system SHALL emit a stderr message naming the failing section AND the parse-error detail returned by the loader, AND SHALL exit with non-zero status, AND SHALL NOT perform any side effect that depends on the broken section (no `claude` child spawn, no keyring read/write/delete, no wiki file write, no run-log append, no auto-commit).

The third outcome applies to every codebus subcommand whose execution depends on the broken section: `goal`, `query`, `fix`, and `config` (including all three `config` sub-actions `set-key`, `get-key`, `delete-key`). When the broken section is unused by a given subcommand the system MAY proceed; however the helper implementations defined by this change apply fail-loud uniformly across every helper (`load_pii_config_with_warning`, `load_claude_code_config_with_warning`, `load_log_config_with_warning`, the inline `load_lint_fix_config` handlers in `goal.rs` / `fix.rs`, AND `read_azure_keyring_service_from_config` in `commands/config.rs`) so callers cannot accidentally skip the gate.

#### Scenario: Yaml syntax error aborts goal verb before agent spawn

- **WHEN** `~/.codebus/config.yaml` contains a yaml syntax error (e.g. missing colon on the `pii` key) AND the user runs `codebus goal "ingest X"`
- **THEN** the binary SHALL exit with non-zero status AND stderr SHALL contain a parse-error message naming the failing section AND no `claude` child process SHALL be spawned AND no wiki file under `<vault>/wiki/` SHALL be created or modified

#### Scenario: Invalid SystemModel value aborts query verb

- **WHEN** `~/.codebus/config.yaml` contains `claude_code.system.goal.model: gpt-4` (a value rejected by the `SystemModel` enum) AND the user runs `codebus query "what is X"`
- **THEN** the binary SHALL exit with non-zero status AND stderr SHALL contain a message naming `claude_code.system.goal.model` AND the invalid variant text AND no `claude` child process SHALL be spawned

#### Scenario: Yaml syntax error aborts config delete-key before keyring access

- **WHEN** `~/.codebus/config.yaml` contains a yaml syntax error AND the user runs `codebus config delete-key azure`
- **THEN** the binary SHALL exit with non-zero status AND stderr SHALL contain a parse-error message AND the keyring entry for any service name SHALL remain unchanged (no `Entry::delete_credential` call)

#### Scenario: Missing config file preserves default behavior

- **WHEN** `~/.codebus/config.yaml` does not exist AND the user runs `codebus goal "X"`
- **THEN** the binary SHALL proceed with each section's `Default::default()` AND SHALL NOT emit any parse-error message on stderr

#### Scenario: Unknown key under valid yaml does not trigger fail-loud

- **WHEN** `~/.codebus/config.yaml` parses cleanly under every section loader but contains a `future_field: hello` key the binary does not recognise AND the user runs `codebus query "X"`
- **THEN** the binary SHALL proceed normally AND SHALL NOT emit any parse-error message on stderr AND the unknown key SHALL have no observable effect on the invocation

##### Example: behavior matrix

| Config file state                        | All verb commands  | `config set-key` | `config get-key` | `config delete-key` |
| ---------------------------------------- | ------------------ | ---------------- | ---------------- | ------------------- |
| File absent                              | Proceed (defaults) | Proceed          | Proceed          | Proceed             |
| Parses cleanly, unknown keys present     | Proceed            | Proceed          | Proceed          | Proceed             |
| Parses cleanly, recognised values        | Proceed            | Proceed          | Proceed          | Proceed             |
| Yaml syntax error                        | Abort, exit ≠ 0    | Abort            | Abort            | Abort               |
| Schema validation failure in any section | Abort, exit ≠ 0    | Abort            | Abort            | Abort               |
