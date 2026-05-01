# keyring-integration Specification

## Purpose

TBD - created by archiving change 'provider-settings-and-onboarding'. Update Purpose after archive.

## Requirements

### Requirement: Tauri keyring plugin commands

The Tauri host SHALL expose three IPC commands `keyring_set` / `keyring_get` / `keyring_delete` that wrap an OS-native keychain (macOS Keychain Services, Windows Credential Manager, Linux Secret Service) for storing LLM API keys. The implementation MUST use either `tauri-plugin-keyring` or the underlying `keyring-rs` crate; the choice MUST be confirmed by a cross-platform proof of concept on macOS, Windows, and at least one Linux desktop environment (GNOME Keyring or KWallet) before code is merged.

The three commands MUST share a single key namespace `codebus.<provider_id>.api_key` where `<provider_id>` matches the regex `^[a-z][a-z0-9-]{2,40}$`. Commands MUST NOT accept a free-form key; the namespace prefix is appended host-side so the renderer cannot escape into other applications' keychain entries.

#### Scenario: keyring_set persists to OS keychain

- **WHEN** the renderer invokes `keyring_set({ provider_id: "openai-default", api_key: "sk-proj-..." })`
- **THEN** the Tauri host MUST resolve the canonical key name `codebus.openai-default.api_key` and call the OS keychain set primitive
- **AND** on success the response MUST be `{ ok: true }` and the value MUST persist across app restarts

#### Scenario: keyring_get returns stored value

- **WHEN** `keyring_set` has been called for a `provider_id` and the renderer later invokes `keyring_get({ provider_id: "openai-default" })`
- **THEN** the response MUST be `{ ok: true, api_key: "<the stored value>" }`

#### Scenario: keyring_get for unknown provider returns missing

- **WHEN** the renderer invokes `keyring_get({ provider_id: "never-set" })` and no entry exists
- **THEN** the response MUST be `{ ok: false, code: "KEYRING_ENTRY_MISSING" }`
- **AND** the host MUST NOT raise an unhandled exception

#### Scenario: keyring_delete removes entry

- **WHEN** `keyring_set` has been called and the renderer later invokes `keyring_delete({ provider_id: "openai-default" })`
- **THEN** the response MUST be `{ ok: true }`
- **AND** a subsequent `keyring_get` for the same `provider_id` MUST return `{ ok: false, code: "KEYRING_ENTRY_MISSING" }`

#### Scenario: provider_id rejects characters outside the allowed regex

- **WHEN** the renderer invokes `keyring_set({ provider_id: "../escape", api_key: "x" })`
- **THEN** the host MUST reject with `{ ok: false, code: "KEYRING_INVALID_PROVIDER_ID" }`
- **AND** no value MUST be written to the OS keychain

---
### Requirement: Tauri-to-sidecar startup key injection

When Tauri spawns the sidecar binary, the host SHALL collect every API key currently stored in the keychain (one per `llm.providers[]` entry in the active config), then after the sidecar emits its handshake line, push the keys via a single bearer-authenticated `POST /internal/startup-config` call to the sidecar loopback URL. The keys MUST flow through the loopback HTTP boundary; the host MUST NOT pass keys via environment variables, command-line arguments, or stdin lines after the handshake.

The sidecar SHALL accept `POST /internal/startup-config` exactly once per process lifetime, within five seconds of handshake emission. The endpoint body schema is `{ provider_keys: { <provider_id>: <api_key>, ... } }`. On success the sidecar SHALL store the keys in `app.state.provider_keys` (in-memory dict only) and return HTTP 204. The endpoint MUST be excluded from the OpenAPI document (`include_in_schema=False`).

#### Scenario: Sidecar accepts startup-config exactly once

- **WHEN** the sidecar receives `POST /internal/startup-config` with a valid bearer and well-formed body
- **THEN** the response status MUST be 204
- **AND** `app.state.provider_keys` MUST contain the supplied entries

#### Scenario: Second startup-config call rejected

- **WHEN** the sidecar receives a second `POST /internal/startup-config` call within the same process lifetime
- **THEN** the response status MUST be 409
- **AND** the response body MUST be `{ "detail": { "code": "STARTUP_ALREADY_CONFIGURED" } }`
- **AND** `app.state.provider_keys` MUST remain unchanged from the first call

#### Scenario: startup-config without bearer rejected

- **WHEN** the endpoint receives a request without the bearer header
- **THEN** the response status MUST be 401 (matching the existing bearer middleware behavior)

#### Scenario: startup-config endpoint hidden from OpenAPI

- **WHEN** any client requests `GET /openapi.json`
- **THEN** the returned document MUST NOT contain a path entry for `/internal/startup-config`

---
### Requirement: API keys never written to disk or audit logs

API keys flowing through the keyring → Tauri → sidecar pipeline SHALL exist only in: (1) the OS keychain database managed by the operating system, (2) Tauri host process memory between read and IPC dispatch, (3) sidecar `app.state.provider_keys` for the duration of the sidecar process. They MUST NOT appear in: any file under `<workspace>/.codebus/`, the `~/.codebus/` directory, sidecar stdout/stderr, FastAPI access logs, error response bodies, SSE event payloads, or any user-visible message.

The seven workspace audit JSONL files (`sanitize_audit.jsonl` / `tool_audit.jsonl` / `kb_growth.jsonl` / `reasoning_log.jsonl` / `token_usage.jsonl` / `llm_calls.jsonl` / `generator_log.jsonl`) and the App-level `~/.codebus/authorization_audit.jsonl` MUST NOT be read or modified by the keyring code path.

#### Scenario: API key never appears in any audit JSONL

- **WHEN** the test suite runs an end-to-end flow that calls `keyring_set` with a sentinel API key value, runs the sidecar through several LLM calls, and then greps every `*.jsonl` under `<workspace>/.codebus/` and `~/.codebus/`
- **THEN** zero matches MUST be found for the sentinel value

#### Scenario: API key never appears in sidecar stdout

- **WHEN** the test captures sidecar stdout and stderr during the same flow
- **THEN** zero matches MUST be found for the sentinel API key value

#### Scenario: API key never appears in FastAPI error responses

- **WHEN** the test forces a sidecar exception during an LLM call (using a sentinel API key as the configured key)
- **THEN** the SSE error event payload MUST NOT contain the sentinel value
- **AND** any HTTP error response body MUST NOT contain the sentinel value
