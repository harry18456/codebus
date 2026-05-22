## ADDED Requirements

### Requirement: Settings Provider Registry

The Settings layer SHALL drive its provider-specific behavior from a single provider registry rather than hard-coding the claude provider. Each registry entry SHALL declare: a stable `id` (the `agent.active_provider` value, e.g. `claude` or `codex`), a `displayName`, a `cliBinaryId` (the argument passed to `check_cli_installed`), the set of endpoint `profiles` the provider supports, a `validate(block)` function returning the provider's client-side validation errors, and an endpoint-editor component. The registry SHALL contain exactly two entries in this change: `claude` and `codex`.

The set of profiles SHALL be declared per provider, NOT assumed universal. `claude` and `codex` each declare `["system", "azure"]`; a future provider MAY declare a different set (e.g. only `["system"]`) and SHALL NOT be forced to expose an azure profile. Each provider's endpoint editor is a concrete component (no generic schema-to-form engine); the registry abstracts only the cross-provider glue (provider selection, config read/write keyed by provider id, CLI status probe by `cliBinaryId`, validation dispatch, and backend validation dispatch).

The in-memory config store SHALL expose provider-keyed accessors `getProviderBlock(id)` and `updateProviderBlock(id, block)` that read and write `agent.providers.<id>` and set `agent.active_provider` to the currently selected provider. The store SHALL NOT hard-code `active_provider` to `claude`. Adding a future provider SHALL require adding a registry entry plus its editor component, with no change to the cross-provider glue.

#### Scenario: Registry exposes claude and codex entries

- **WHEN** the Settings modal initializes its provider registry
- **THEN** the registry SHALL contain entries for `claude` and `codex`, each with a `cliBinaryId`, declared `profiles`, a `validate` function, and an editor component

#### Scenario: Provider declares its own profiles without an assumed azure slot

- **WHEN** a provider registry entry declares its supported `profiles`
- **THEN** the declared set SHALL be used verbatim (codex declares `["system", "azure"]`) AND no azure profile SHALL be synthesized for a provider that did not declare one

#### Scenario: Store writes the selected provider block and active_provider

- **WHEN** `updateProviderBlock("codex", block)` is called
- **THEN** the in-memory config SHALL set `agent.providers.codex` to `block` AND `agent.active_provider` to `codex`, preserving sibling `agent.providers.*` entries

## MODIFIED Requirements

### Requirement: IPC Command Registry

The system SHALL expose exactly fifteen Tauri commands invokable from the frontend: `list_vaults`, `add_vault`, `remove_vault`, `load_global_config`, `save_global_config`, `set_endpoint_key`, `get_endpoint_key`, `delete_endpoint_key`, `check_cli_installed`, `spawn_goal`, `cancel_goal`, `list_runs`, `get_run_detail`, `list_wiki_pages`, `read_wiki_page`. No other Tauri commands SHALL be registered by this change. Each command SHALL have a stable name (snake_case), a typed argument shape, and a typed return shape mirroring the design contract.

The `check_cli_installed` command SHALL accept a `provider: String` argument whose legal values are the literals `"claude_code"` and `"codex"`. The command SHALL probe whether the agentic CLI binary for that provider is reachable by spawning `<binary> --version`. It SHALL return a `CliStatus` enum (serialised as `serde(tag = "kind", rename_all = "snake_case")` with variants `installed { version }` and `not_installed`). Any spawn failure — binary missing, non-zero exit, empty stdout — SHALL collapse to `not_installed`; the underlying error SHALL NOT surface to the frontend. A `provider` value outside the legal set SHALL collapse to `not_installed` (never an error to the frontend). Further provider values (`gemini_cli`, etc.) extend this match arm in a separate change.

The three keyring-management commands (`set_endpoint_key` / `get_endpoint_key` / `delete_endpoint_key`) SHALL accept a `service: String` argument naming the OS keyring service to act on. The Settings editor supplies the active provider's `azure.keyring_service` (defaulting to `codebus-claude-azure` for claude / `codebus-codex-azure` for codex), so claude and codex keys occupy DISTINCT keyring entries and the GUI writes to exactly the service the user sees — without a stale on-disk config lookup. An empty / whitespace-only `service` SHALL reject the call with `AppError::Invalid { field: "service", message: ... }`. The commands SHALL delegate to the codebus-core keyring helpers (`store_azure_key` / `probe_keyring_only` / `delete_azure_key`) — there is no separate keyring backend implementation in the app crate.

`set_endpoint_key` SHALL accept a `key: String` argument and store the value via the codebus-core helper. On success it SHALL return `Ok(())`. The key value SHALL NOT be cached anywhere in the app process beyond the Tauri command call boundary.

`get_endpoint_key` SHALL return a `KeyStatus` enum (serialised as `serde(tag = "kind", rename_all = "snake_case")` with variants `set` and `unset`) reflecting only whether the keyring entry exists. The command SHALL NOT return the key value under any circumstance, including with any optional flag — verifying the key value SHALL require running the CLI verb (`codebus query "ping"`) instead.

`delete_endpoint_key` SHALL be idempotent: removing a non-existent entry SHALL return `Ok(())` rather than an error.

The six new commands `spawn_goal`, `cancel_goal`, `list_runs`, `get_run_detail`, `list_wiki_pages`, and `read_wiki_page` are defined normatively in the `app-workspace` capability (Tauri IPC Commands for Goal Lifecycle and Wiki Read requirement). Their argument shapes, return types, and error behavior live in that capability; this registry requirement only pins their existence and total count.

#### Scenario: check_cli_installed probes claude binary

- **WHEN** the frontend invokes `check_cli_installed("claude_code")`
- **THEN** the command SHALL probe the claude binary and return `installed { version }` or `not_installed`

#### Scenario: check_cli_installed probes codex binary

- **WHEN** the frontend invokes `check_cli_installed("codex")`
- **THEN** the command SHALL probe the codex binary and return `installed { version }` or `not_installed` (a missing codex binary collapses to `not_installed`, never a frontend error)

#### Scenario: Unknown provider collapses to not_installed

- **WHEN** the frontend invokes `check_cli_installed("gemini_cli")`
- **THEN** the command SHALL return `not_installed` without surfacing an error

### Requirement: Settings UI CLI Status Field

The Settings modal SHALL render a CLI Status row that probes whether the agentic CLI binary for the currently selected provider is installed. The row SHALL invoke `check_cli_installed` with the selected provider's `cliBinaryId` (from the provider registry) on modal open AND whenever the selected provider changes. It SHALL display one of three states: `Checking…` (probe in flight), `Installed · <version>` (success), or `Not installed` (any failure). When the state is `Not installed`, the row SHALL render an inline hint instructing the user to install that provider's CLI before configuring the endpoint. The row label SHALL reflect the selected provider's `displayName`.

#### Scenario: CLI status row probes claude when claude is selected

- **WHEN** the selected provider is `claude` AND `check_cli_installed("claude_code")` returns `{ kind: "installed", version: "2.1.139 (Claude Code)" }`
- **THEN** the Settings UI SHALL display a status badge containing `Installed` AND the version string `2.1.139 (Claude Code)`

#### Scenario: CLI status row probes codex when codex is selected

- **WHEN** the selected provider is `codex` AND `check_cli_installed("codex")` returns `{ kind: "not_installed" }`
- **THEN** the Settings UI SHALL display a status badge containing `Not installed` AND an inline hint instructing the user to install the codex CLI

### Requirement: Settings UI Endpoint Section

The Settings modal SHALL render a provider selector AND an Endpoint section that lets the user configure the currently selected provider's endpoint profiles entirely from the GUI. The provider selector SHALL list the providers in the registry (`claude`, `codex`); selecting one SHALL set `agent.active_provider` and SHALL switch the Endpoint section to that provider's editor component and the CLI Status row to that provider's binary probe. The Endpoint section heading SHALL reflect the selected provider's `displayName`.

The Endpoint editor rendered SHALL be the registry entry's editor component for the selected provider:

- The claude editor SHALL contain an `active` radio group (`system` / `azure`); a System Profile sub-section with four verb rows (`goal` / `query` / `fix` / `verify`), each row a free-text `model` combobox input wired to a suggestion `<datalist>` of the known aliases (`opus-4-7`, `opus-4-6`, `haiku-4-5`, `sonnet-4-6`) — a newly-released Claude model MAY be typed directly (codebus-core relaxed `SystemModel` to a free string translated to the CLI `--model` flag via the `claude-` prefix) — AND an `effort` `<select>` with six options (`low`, `medium`, `high`, `xhigh`, `max`, `auto`); and an Azure Profile sub-section with a `base_url` text input, a `keyring_service` text input (default `codebus-claude-azure`), an API-key status indicator with `Set new...` / `Delete` buttons, AND four verb rows each with a free-text `model` (deployment name) input AND an `effort` `<select>`.
- The codex editor SHALL contain an `active` radio group over the codex provider's declared profiles (`system` / `azure`); a System Profile sub-section with four verb rows each containing a free-text `model` input (codex model names are arbitrary strings, NOT a closed enum) AND an `effort` input; and an Azure Profile sub-section with a `base_url` text input, an `api_version` text input, a `keyring_service` text input (default `codebus-azure`), the API-key status indicator with `Set new...` / `Delete` buttons, AND four verb rows each with a free-text deployment-name `model` input AND an `effort` input.

Both profile sub-sections of the active provider SHALL be present in the DOM organised as an accordion: the sub-section whose name matches the provider's `active` value SHALL be expanded; the other SHALL be collapsed showing only its header with an `(inactive)` label. Toggling the `active` radio SHALL auto-expand the newly-active sub-section and auto-collapse the previously-active one WITHOUT clearing any form input values.

The `Save` button SHALL persist only yaml content via `save_global_config`; it SHALL NOT carry any API key. API keys SHALL flow exclusively through the three `*_endpoint_key` IPC commands. The Settings UI SHALL NOT include a Test Connection button.

The Settings modal SHALL perform client-side validation of the selected provider's endpoint block before allowing Save, using the registry entry's `validate(block)` function. The validation rules SHALL match the corresponding codebus-core parser (`parse_claude_code_yaml` for claude, `parse_codex_yaml` for codex) so the frontend and the `save_global_config` backend gate produce the same reject/accept decision. On failure the modal SHALL disable Save, render an inline validation summary listing each failing field, AND apply `aria-invalid="true"` to the offending inputs. The backend `save_global_config` SHALL validate the active provider's block via the matching core parser and reject an invalid block before writing the file.

The codex provider's validation SHALL require, when codex `active === "azure"`, non-empty `base_url`, `api_version`, `keyring_service`, AND each verb's `model`; and when `active === "system"`, non-empty `model` for each verb. Unknown codex `model` strings SHALL NOT be rejected (codex models are not a closed enum).

#### Scenario: Selecting codex switches editor and CLI probe

- **WHEN** the user selects `codex` in the provider selector
- **THEN** `agent.active_provider` SHALL become `codex` AND the Endpoint section SHALL render the codex editor (free-text model inputs plus an `api_version` field in the azure sub-section) AND the CLI Status row SHALL probe the codex binary

#### Scenario: Claude editor uses a free-text model combobox

- **WHEN** the selected provider is `claude`
- **THEN** the System Profile model control SHALL be a free-text input wired to a suggestion `<datalist>` of the four known aliases, AND typing a not-yet-known model (e.g. `opus-4-8`) SHALL be accepted (no closed-enum rejection)

#### Scenario: Codex azure requires base_url, api_version, keyring_service, and verb models

- **WHEN** the selected provider is `codex`, codex `active === "azure"`, AND `api_version` (or any required azure field) is the empty string AND the user has edited a setting
- **THEN** the Save button SHALL be disabled AND the validation summary SHALL list the failing field(s) including `api_version`

#### Scenario: Codex system accepts arbitrary model strings

- **WHEN** the selected provider is `codex`, codex `active === "system"`, AND a verb `model` is set to `gpt-5.5`
- **THEN** validation SHALL accept it (no closed-enum rejection) AND, if all verbs' models are non-empty and the user has edited a setting, the Save button SHALL be enabled

#### Scenario: Backend rejects an invalid codex block on save

- **WHEN** `save_global_config` receives a config with `agent.active_provider: codex` and a codex block whose active profile is missing a required verb
- **THEN** the command SHALL reject with an `AppError` (validated via `parse_codex_yaml`) AND SHALL NOT write the file
