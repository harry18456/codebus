# mcp-client-install Specification

## Purpose

One-click registration of codebus as a **user-scope** MCP server in the user's agent clients (claude / codex), driven from app Settings — shelling out to each client's own native CLI (never editing its config files), with per-client detection and independent controls.

## Requirements

### Requirement: One-click MCP client registration

The codebus-app SHALL provide, per supported client (claude and codex independently), a one-click action that registers codebus as a **user-scope** MCP server in that client and a corresponding action that removes it. Registration SHALL be performed by shelling out to the client's own native CLI with an argument vector (argv array), NOT by parsing, merging, or rewriting the client's configuration files. The command SHALL pass the absolute path of the codebus CLI binary bundled with the app (resolved from the app's bundled resources), NOT the bare name `codebus`, so registration does not depend on `codebus` being on `PATH`. The argv array form SHALL be used (never a single concatenated shell string) so a CLI path containing spaces is passed as one argument and no shell metacharacter is interpreted.

For claude the registration command SHALL include `--scope user` (the claude default scope is `local`, which only registers for the current project; user scope is required for a global registration). For codex no scope flag SHALL be passed (codex MCP config is a single global store with no scope concept).

#### Scenario: Registering codebus into claude uses user scope and the bundled CLI path

- **WHEN** the user enables the MCP integration for the claude client
- **THEN** the app SHALL invoke the claude CLI with an argv array equivalent to `claude mcp add --scope user codebus -- <absolute-bundled-codebus-path> mcp` AND SHALL NOT modify any claude configuration file directly

#### Scenario: Registering codebus into codex omits scope

- **WHEN** the user enables the MCP integration for the codex client
- **THEN** the app SHALL invoke the codex CLI with an argv array equivalent to `codex mcp add codebus -- <absolute-bundled-codebus-path> mcp` AND SHALL NOT pass any `--scope` flag

#### Scenario: Disabling removes the registration symmetrically

- **WHEN** the user disables the MCP integration for a client that currently has codebus registered
- **THEN** the app SHALL invoke the client's native remove command (`claude mcp remove --scope user codebus` for claude / `codex mcp remove codebus` for codex)

##### Example: per-client command construction

| client | action  | argv (after the resolved client binary) |
| ------ | ------- | --------------------------------------- |
| claude | install | `mcp add --scope user codebus -- <abs-codebus> mcp` |
| claude | remove  | `mcp remove --scope user codebus` |
| codex  | install | `mcp add codebus -- <abs-codebus> mcp` |
| codex  | remove  | `mcp remove codebus` |

<!-- @trace
source: mcp-multi-vault-and-client-install
updated: 2026-06-28
code:
  - codebus-app/src-tauri/src/ipc/mcp_install.rs
tests:
  - codebus-app/src-tauri/src/ipc/mcp_install.rs
-->

---
### Requirement: Client detection and absent-client handling

The app SHALL present an independent MCP-integration control for EACH supported client (claude and codex) as its own Settings row — NOT a single control that follows the currently active provider — and SHALL detect, per client, whether that client's CLI is installed before offering registration for it, reusing the existing CLI-presence probe (spawning `<client-binary> --version`). When a client's CLI is not detected, that client's control SHALL be disabled and a friendly hint SHALL be shown; a missing client SHALL NOT surface as an error AND SHALL NOT affect the other client's control. The client binary the app invokes for registration SHALL be resolved by the same rule the agent backend uses (the `CODEBUS_CLAUDE_BIN` / `CODEBUS_CODEX_BIN` override when set, otherwise the platform default — `claude`, and `codex.cmd` on Windows / `codex` elsewhere), so detection and invocation agree on which binary is used.

#### Scenario: Absent client disables the control without erroring

- **WHEN** the user opens the MCP integration control for a client whose CLI is not installed
- **THEN** the control SHALL be disabled with a hint that the client is not installed AND no error SHALL be surfaced to the user

#### Scenario: Detected client enables the control

- **WHEN** a supported client's CLI responds to a `--version` probe
- **THEN** that client's MCP integration control SHALL be enabled and reflect whether codebus is currently registered in that client

#### Scenario: Each supported client has an independent control

- **WHEN** the Settings MCP-integration section renders AND claude is installed but codex is not
- **THEN** the claude control SHALL be enabled and reflect its own registration state AND the codex control SHALL be shown disabled with a not-installed hint, independently of the currently active provider selection

<!-- @trace
source: mcp-multi-vault-and-client-install
updated: 2026-06-28
code:
  - codebus-app/src-tauri/src/ipc/mcp_install.rs
  - codebus-app/src-tauri/src/ipc/cli_status.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-core/src/agent/codex_backend.rs
tests:
  - codebus-app/src/components/settings/McpIntegrationSection.test.tsx
-->

---
### Requirement: MCP-integration IPC commands

The app SHALL expose three Tauri IPC commands to drive the integration: `mcp_client_status(provider: String)`, `mcp_client_install(provider: String)`, and `mcp_client_remove(provider: String)`. The `provider` argument SHALL accept the literals `"claude_code"` and `"codex"`. For `mcp_client_status`, any other value SHALL resolve to `client_missing` (never a frontend error). For `mcp_client_install` / `mcp_client_remove`, an unknown `provider` SHALL return an `AppError` (fail-loud — the frontend only ever offers the two literals, and a mutating action SHALL NOT silently no-op). `mcp_client_status` SHALL return an `McpClientStatus` enum serialised as `serde(tag = "kind", rename_all = "snake_case")` with variants `installed` (codebus is registered in the client), `not_registered` (client present but codebus not registered), and `client_missing` (client CLI not detected); status SHALL be derived by querying the client's own listing command (`claude mcp list` / `codex mcp list`) for a codebus entry. `mcp_client_install` and `mcp_client_remove` SHALL return `Result<(), AppError>`; a non-zero exit from the shelled-out client command SHALL surface as `AppError::Io` carrying the captured stderr tail. In a development build where the bundled CLI resource is absent, the commands SHALL fall back to a development-resolved codebus binary so the integration is exercisable; this fallback SHALL apply only to development builds and SHALL NOT change packaged-build behavior.

#### Scenario: Status reflects registration state

- **WHEN** the frontend invokes `mcp_client_status("claude_code")` AND the claude CLI lists a `codebus` MCP entry
- **THEN** the command SHALL return `installed`

#### Scenario: Status reports client_missing when CLI absent

- **WHEN** the frontend invokes `mcp_client_status("codex")` AND the codex CLI is not detected
- **THEN** the command SHALL return `client_missing` without surfacing an error

#### Scenario: Install failure surfaces the client error

- **WHEN** the frontend invokes `mcp_client_install("claude_code")` AND the shelled-out `claude mcp add` command exits non-zero
- **THEN** the command SHALL return `AppError::Io` carrying the captured stderr tail AND SHALL NOT report success

#### Scenario: Unknown provider resolves to client_missing

- **WHEN** the frontend invokes `mcp_client_status("gemini_cli")`
- **THEN** the command SHALL return `client_missing` without surfacing an error

<!-- @trace
source: mcp-multi-vault-and-client-install
updated: 2026-06-28
code:
  - codebus-app/src-tauri/src/ipc/mcp_install.rs
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/lib/ipc.ts
tests:
  - codebus-app/src-tauri/src/ipc/mcp_install.rs
-->

---
### Requirement: Global instruction guidance block on enable

When the user enables the MCP integration for a client, the app SHALL additionally write a codebus wiki-usage guidance block into that client's global instruction file; when the user disables it, the app SHALL remove that block. This makes the registered MCP tools discoverable to the client's agent by stating, in its always-loaded global instructions, that a cross-project wiki library is available and when to reach for it. The same guidance content SHALL be used for both clients.

The guidance SHALL be written as a marked managed block delimited by the literal markers `<!-- codebus:mcp:start -->` and `<!-- codebus:mcp:end -->`. Enabling SHALL perform an idempotent upsert: when both markers are already present and well-ordered, the app SHALL replace the existing block in place; otherwise it SHALL append the block at the end of the file separated by a blank line. Disabling SHALL remove exactly the marked block, collapsing the blank lines its removal leaves, and SHALL be a no-op when the block or the file is absent. The app SHALL preserve every byte outside the markers unchanged and SHALL write the file atomically (temporary file then rename) so a failure never leaves a half-written instruction file. When the global instruction file does not exist, enabling SHALL create it containing only the block.

The global instruction file SHALL be resolved per client: for claude, `CLAUDE.md` under `CLAUDE_CONFIG_DIR` when that environment variable is set, otherwise `CLAUDE.md` under `~/.claude`; for codex, `AGENTS.md` under `CODEX_HOME` when set, otherwise `AGENTS.md` under `~/.codex`.

The guidance-block write SHALL be subordinate to the MCP registration: the client CLI `mcp add` / `mcp remove` is authoritative, and a failure to write or remove the guidance block SHALL be surfaced as a warning but SHALL NOT roll back the registration nor fail the IPC (the command SHALL still return success when the registration itself succeeded). Each client's Settings control SHALL disclose, in visible copy, that enabling also writes — and disabling removes — a codebus guidance block in that client's global instructions.

#### Scenario: Enabling twice upserts a single block

- **WHEN** the user enables the MCP integration for a client twice in a row
- **THEN** the client's global instruction file SHALL contain exactly one codebus marked block, its content replaced in place rather than duplicated

##### Example: idempotent upsert into an empty file

- **GIVEN** the claude global instruction file does not yet exist (or is empty)
- **WHEN** the MCP integration is enabled, then enabled a second time
- **THEN** the file contains exactly one `<!-- codebus:mcp:start -->` … `<!-- codebus:mcp:end -->` block — two enables produce one block, not two

#### Scenario: Disabling removes only the marked block

- **GIVEN** a client's global instruction file contains hand-written content plus a codebus marked block
- **WHEN** the user disables the MCP integration for that client
- **THEN** the codebus marked block SHALL be removed AND every byte of hand-written content outside the markers SHALL remain unchanged

#### Scenario: Per-client file resolution honors environment overrides

- **WHEN** the guidance block is written for the codex client AND `CODEX_HOME` is set
- **THEN** the block SHALL be written to `AGENTS.md` under `CODEX_HOME`, and for the claude client to `CLAUDE.md` under `CLAUDE_CONFIG_DIR` when set, each falling back to `~/.codex` / `~/.claude` when the override is unset

#### Scenario: Guidance write failure does not fail registration

- **WHEN** the client CLI registration succeeds but writing the guidance block fails
- **THEN** the command SHALL surface a warning AND SHALL return success AND SHALL NOT undo the registration

#### Scenario: Settings discloses the global-instruction write

- **WHEN** the Settings MCP-integration control for a client renders
- **THEN** it SHALL display copy stating that enabling also writes, and disabling removes, a codebus guidance block in that client's global instructions

<!-- @trace
source: mcp-usage-guidance
updated: 2026-06-28
code:
  - codebus-app/src-tauri/src/ipc/global_md.rs
  - codebus-app/src-tauri/src/ipc/mcp_install.rs
  - codebus-app/src/components/settings/McpIntegrationSection.tsx
  - codebus-app/src/i18n/messages.ts
tests:
  - codebus-app/src-tauri/src/ipc/global_md.rs
  - codebus-app/src-tauri/src/ipc/mcp_install.rs
  - codebus-app/src/components/settings/McpIntegrationSection.test.tsx
-->
