## MODIFIED Requirements

### Requirement: Config Subcommand For Keyring Management

The `codebus` binary SHALL provide a `config` subcommand that exposes four actions: `set-key <profile>`, `get-key <profile>`, `delete-key <profile>`, and `purge-keys`. The `<profile>` argument SHALL accept the literal value `azure` and SHALL reject all other values with a non-zero exit code. The `get-key` action SHALL accept an optional `--show` flag. The `purge-keys` action SHALL take no profile argument.

`codebus config set-key azure` SHALL read a key from stdin without echoing, write the value to the keyring entry `(azure.keyring_service, "default")`, and exit zero on success. If the keyring backend is unavailable, the command SHALL exit non-zero with a stderr message instructing the user to set `CODEBUS_AZURE_KEY` instead.

`codebus config get-key azure` SHALL print `set` if the keyring entry exists AND `unset` otherwise. When `--show` is passed AND the entry exists, the command SHALL print the key value verbatim.

`codebus config delete-key azure` SHALL remove the keyring entry if present AND SHALL exit zero whether or not the entry existed (idempotent).

`codebus config purge-keys` SHALL remove the Azure keyring entries for BOTH the claude and codex providers in a single invocation, addressed as `(service, "default")`. For each provider it SHALL resolve the keyring service name from config — claude from `agent.providers.claude.azure.keyring_service` and codex from `agent.providers.codex.azure.keyring_service` — and SHALL fall back to the well-known default service name for that provider (`codebus-claude-azure` for claude, `codebus-codex-azure` for codex) when the config file is absent, fails to parse, or omits that provider's `keyring_service`. The action SHALL be best-effort and idempotent: a missing entry, an unavailable keyring backend, or an absent/unparseable config SHALL NOT cause a non-zero exit. `codebus config purge-keys` SHALL exit zero in all of these cases.

#### Scenario: set-key stores the key

- **WHEN** the user runs `codebus config set-key azure` and enters `sk-test` on stdin
- **THEN** the keyring entry `(codebus-claude-azure, default)` SHALL contain `sk-test` AND stdout SHALL contain `key stored` AND the command SHALL exit zero

#### Scenario: get-key reports unset without revealing absence detail

- **WHEN** the user runs `codebus config get-key azure` AND no keyring entry exists
- **THEN** stdout SHALL print `unset` AND the command SHALL exit zero

#### Scenario: get-key with --show prints the value

- **WHEN** the user runs `codebus config get-key azure --show` AND the keyring entry holds `sk-test`
- **THEN** stdout SHALL print `sk-test` AND the command SHALL exit zero

#### Scenario: delete-key is idempotent

- **WHEN** the user runs `codebus config delete-key azure` AND no keyring entry exists
- **THEN** the command SHALL exit zero

#### Scenario: Unknown profile argument rejected

- **WHEN** the user runs `codebus config set-key bedrock`
- **THEN** the command SHALL exit non-zero AND stderr SHALL contain a clap error message identifying `bedrock` as an invalid profile value

#### Scenario: purge-keys removes both providers' azure keyring entries

- **WHEN** the user runs `codebus config purge-keys` AND both the claude and codex azure keyring entries exist
- **THEN** both entries SHALL be removed AND a subsequent `get-key azure` SHALL print `unset` AND the command SHALL exit zero

#### Scenario: purge-keys is idempotent on absent entries

- **WHEN** the user runs `codebus config purge-keys` AND neither provider's azure keyring entry exists
- **THEN** the command SHALL exit zero and SHALL NOT emit an error

#### Scenario: purge-keys falls back to default service names when config is absent

- **WHEN** the user runs `codebus config purge-keys` AND no `~/.codebus/config.yaml` exists
- **THEN** the command SHALL target the default services `codebus-claude-azure` and `codebus-codex-azure`, remove those entries if present, AND exit zero
