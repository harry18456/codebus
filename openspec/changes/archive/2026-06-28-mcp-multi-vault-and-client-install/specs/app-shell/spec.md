## MODIFIED Requirements

### Requirement: App-State Persistence

The system SHALL persist app-level state to `~/.codebus/app-state.json`. The file SHALL include a top-level `schema_version: 1` field and a top-level `vault_list` array. Each `vault_list` entry SHALL contain an absolute `path` string, a `display_name` string, and a `last_opened` ISO 8601 UTC timestamp string. The codebus-app SHALL be the sole writer of `app-state.json`; no CLI subcommand SHALL write `app-state.json`. The CLI `mcp` subcommand, when running in registry mode, SHALL read `app-state.json` READ-ONLY as the multi-vault registry (per the `mcp-server` capability); no other CLI subcommand SHALL read or write it.

On startup, if the file does not exist, the system SHALL create it with `{ "schema_version": 1, "vault_list": [] }`. On startup, if the file fails to parse OR if `schema_version` exceeds the current supported value, the system SHALL log a warning and proceed with an empty in-memory `vault_list` without overwriting the file.

#### Scenario: First launch creates empty state file

- **WHEN** the codebus-app launches and `~/.codebus/app-state.json` does not exist
- **THEN** the system creates the file containing `{ "schema_version": 1, "vault_list": [] }`

#### Scenario: Corrupt state falls back to empty without overwrite

- **WHEN** the codebus-app launches and `~/.codebus/app-state.json` contains invalid JSON
- **THEN** the system logs a parse warning, the in-memory vault list is empty, and the on-disk file is not modified

##### Example: schema version mismatch

- **GIVEN** `~/.codebus/app-state.json` contains `{ "schema_version": 99, "vault_list": [...] }`
- **WHEN** the codebus-app launches
- **THEN** the system logs a `schema_version unsupported` warning, the in-memory vault list is empty, and the on-disk file is preserved

#### Scenario: The mcp subcommand reads the registry read-only

- **WHEN** `codebus mcp` runs in registry mode and resolves vaults from `~/.codebus/app-state.json`
- **THEN** the CLI SHALL read the file AND SHALL NOT create, modify, or delete it; writes to the registry remain exclusively the codebus-app's responsibility

### Requirement: IPC Command Registry

The system SHALL register a closed set of Tauri commands invokable from the frontend, totaling thirty-two. The foundation commands are: `list_vaults`, `add_vault`, `remove_vault`, `load_global_config`, `save_global_config`, `set_endpoint_key`, `get_endpoint_key`, `delete_endpoint_key`, `check_cli_installed`, `spawn_goal`, `cancel_goal`, `list_runs`, `get_run_detail`, `list_wiki_pages`, `read_wiki_page`. The remaining commands are defined normatively by their owning capabilities — the goal-lifecycle and wiki-read commands by the `app-workspace` capability, the vault-watcher commands by the `fs-watcher` capability, the MCP-integration commands by the `mcp-client-install` capability, and the chat-turn and quiz-lifecycle commands by their respective app capabilities — each of which pins its own commands' existence, while this requirement pins the closed-set total. No other Tauri command SHALL be registered. Each command SHALL have a stable name (snake_case), a typed argument shape, and a typed return shape mirroring the design contract.

The `check_cli_installed` command SHALL accept a `provider: String` argument whose legal values are the literals `"claude_code"` and `"codex"`. The command SHALL probe whether the agentic CLI binary for that provider is reachable by spawning `<binary> --version`. It SHALL return a `CliStatus` enum (serialised as `serde(tag = "kind", rename_all = "snake_case")` with variants `installed { version }` and `not_installed`). Any spawn failure — binary missing, non-zero exit, empty stdout — SHALL collapse to `not_installed`; the underlying error SHALL NOT surface to the frontend. A `provider` value outside the legal set SHALL collapse to `not_installed` (never an error to the frontend). Further provider values (`gemini_cli`, etc.) extend this match arm in a separate change.

The three keyring-management commands (`set_endpoint_key` / `get_endpoint_key` / `delete_endpoint_key`) SHALL accept a `service: String` argument naming the OS keyring service to act on. The Settings editor supplies the active provider's `azure.keyring_service` (defaulting to `codebus-claude-azure` for claude / `codebus-codex-azure` for codex), so claude and codex keys occupy DISTINCT keyring entries and the GUI writes to exactly the service the user sees — without a stale on-disk config lookup. An empty / whitespace-only `service` SHALL reject the call with `AppError::Invalid { field: "service", message: ... }`. The commands SHALL delegate to the codebus-core keyring helpers (`store_azure_key` / `probe_keyring_only` / `delete_azure_key`) — there is no separate keyring backend implementation in the app crate.

`set_endpoint_key` SHALL accept a `key: String` argument and store the value via the codebus-core helper. On success it SHALL return `Ok(())`. The key value SHALL NOT be cached anywhere in the app process beyond the Tauri command call boundary.

`get_endpoint_key` SHALL return a `KeyStatus` enum (serialised as `serde(tag = "kind", rename_all = "snake_case")` with variants `set` and `unset`) reflecting only whether the keyring entry exists. The command SHALL NOT return the key value under any circumstance, including with any optional flag — verifying the key value SHALL require running the CLI verb (`codebus query "ping"`) instead.

`delete_endpoint_key` SHALL be idempotent: removing a non-existent entry SHALL return `Ok(())` rather than an error.

The six new commands `spawn_goal`, `cancel_goal`, `list_runs`, `get_run_detail`, `list_wiki_pages`, and `read_wiki_page` are defined normatively in the `app-workspace` capability (Tauri IPC Commands for Goal Lifecycle and Wiki Read requirement). Their argument shapes, return types, and error behavior live in that capability; this registry requirement only pins their existence and total count.

The three MCP-integration commands `mcp_client_status`, `mcp_client_install`, and `mcp_client_remove` are defined normatively in the `mcp-client-install` capability. Their argument shapes, return types, shell-out behavior, and error handling live in that capability; this registry requirement only pins their existence and the resulting updated total command count.

#### Scenario: check_cli_installed probes claude binary

- **WHEN** the frontend invokes `check_cli_installed("claude_code")`
- **THEN** the command SHALL probe the claude binary and return `installed { version }` or `not_installed`

#### Scenario: check_cli_installed probes codex binary

- **WHEN** the frontend invokes `check_cli_installed("codex")`
- **THEN** the command SHALL probe the codex binary and return `installed { version }` or `not_installed` (a missing codex binary collapses to `not_installed`, never a frontend error)

#### Scenario: Unknown provider collapses to not_installed

- **WHEN** the frontend invokes `check_cli_installed("gemini_cli")`
- **THEN** the command SHALL return `not_installed` without surfacing an error

#### Scenario: MCP-integration commands are registered

- **WHEN** the frontend invokes `mcp_client_status`, `mcp_client_install`, or `mcp_client_remove`
- **THEN** each SHALL be a registered Tauri command with the contract defined by the `mcp-client-install` capability, and the registry's closed-set count SHALL include these three commands
