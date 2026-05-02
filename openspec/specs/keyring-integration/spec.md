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

When Tauri spawns the sidecar binary, the host SHALL collect every API key currently stored in the keychain (one per `llm.providers[]` entry in the active config), then after the sidecar emits its handshake line, push the keys via a bearer-authenticated `POST /internal/startup-config` call to the sidecar loopback URL. The keys MUST flow through the loopback HTTP boundary; the host MUST NOT pass keys via environment variables, command-line arguments, or stdin lines after the handshake.

The sidecar SHALL accept `POST /internal/startup-config` any number of times during a process lifetime; the latest body REPLACES `app.state.provider_keys` wholesale (not merged). This relaxation (from D-033 B's original "exactly once + 409 lock") supports two real flows the original spec missed:

1. **Onboarding submit** — at boot the keyring is empty, so the first injection delivers an empty dict; after the user finishes the wizard the renderer pushes the just-stored keys via the same endpoint so `/healthz.dependency.llm_chat` flips to `ready` without a sidecar restart.
2. **Settings page edits** — adding / editing a provider's API key likewise requires a fresh push so the new key is visible to subsequent LLM calls.

The endpoint body schema is `{ provider_keys: { <provider_id>: <api_key>, ... } }`. On any call the sidecar SHALL store the keys in `app.state.provider_keys` (in-memory dict only) and return HTTP 204. The endpoint MUST remain excluded from the OpenAPI document (`include_in_schema=False`). Trust boundary is unchanged: bearer + 127.0.0.1 loopback + Tauri-only caller.

#### Scenario: Sidecar accepts initial startup-config

- **WHEN** the sidecar receives `POST /internal/startup-config` with a valid bearer and well-formed body for the first time during its process lifetime
- **THEN** the response status MUST be 204
- **AND** `app.state.provider_keys` MUST contain exactly the supplied entries

#### Scenario: Second startup-config call overwrites

- **WHEN** the sidecar receives a second `POST /internal/startup-config` call during the same process lifetime
- **THEN** the response status MUST be 204
- **AND** `app.state.provider_keys` MUST be replaced wholesale by the new body (existing entries not present in the new body MUST be removed; entries with the same key MUST be overwritten with the new value)
- **AND** no 409 / `STARTUP_ALREADY_CONFIGURED` response MUST be issued

#### Scenario: startup-config without bearer rejected

- **WHEN** the endpoint receives a request without the bearer header
- **THEN** the response status MUST be 401 (matching the existing bearer middleware behavior)

#### Scenario: startup-config endpoint hidden from OpenAPI

- **WHEN** any client requests `GET /openapi.json`
- **THEN** the returned document MUST NOT contain a path entry for `/internal/startup-config`

#### Scenario: Onboarding submit pushes keys after wizard completion

- **WHEN** the renderer completes the onboarding wizard (keyring_set × 2 → upsertProvider × 2 → setBinding × 4 succeed)
- **THEN** the renderer MUST invoke the Tauri `push_startup_config_cmd` IPC with the just-written provider ids
- **AND** the resulting `POST /internal/startup-config` call MUST update `app.state.provider_keys` so the next `/healthz` reports `dependency.llm_chat: ready` and `dependency.llm_embed: ready`
- **AND** if `push_startup_config_cmd` fails the renderer MUST still route to `/onboarding/done` (the failure is non-fatal because a sidecar restart will re-collect keys from the keychain on next boot)


<!-- @trace
source: phase7-onboarding-polish
updated: 2026-05-02
code:
  - sidecar/src/codebus_agent/api/__init__.py
  - web/dist
  - sidecar/src/codebus_agent/api/settings.py
  - web/app/utils/provider-tos.ts
  - tauri/src-tauri/src/lib.rs
  - tauri/src-tauri/Cargo.toml
  - web/app/components/settings/ProviderEditModal.vue
  - web/app/pages/onboarding/providers.vue
  - tauri/src-tauri/capabilities/default.json
  - web/app/components/settings/EmbeddingChangeConfirmModal.vue
  - web/app/pages/onboarding/welcome.vue
  - sidecar/src/codebus_agent/api/startup_config.py
  - web/app/plugins/sidecar-startup-config.client.ts
  - web/app/pages/onboarding/done.vue
  - web/app/utils/external-link.ts
  - web/app/components/settings/RoleBindingTable.vue
  - web/app/components/settings/ProviderPoolList.vue
  - web/nuxt.config.ts
  - web/package.json
  - sidecar/src/codebus_agent/config/llm_config_store.py
  - web/app/pages/settings.vue
  - sidecar/src/codebus_agent/auth/paths.py
  - web/app/components/settings/PiiModeToggle.vue
tests:
  - sidecar/tests/api/test_startup_config.py
  - web/tests/utils/provider-tos.spec.ts
  - web/tests/settings/EmbeddingChangeConfirmModal.spec.ts
  - sidecar/tests/test_cors_preflight_smoke.py
  - sidecar/tests/config/test_llm_config_store.py
  - sidecar/tests/api/test_healthz_dependency.py
  - sidecar/tests/api/test_settings_persistence.py
  - web/tests/onboarding/welcome.spec.ts
  - web/tests/onboarding/providers.spec.ts
-->

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

---
### Requirement: Provider pool persists to disk across sidecar restarts

The sidecar SHALL mirror the in-memory `app.state.provider_pool_snapshot` (provider list, role bindings, PII mode, PII provider id) to a single App-level file at `~/.codebus/llm-config.json`. The file's parent directory SHALL be created on demand. The file SHALL contain metadata only (`id` / `type` / `model` / `base_url` / `bindings` / `pii_mode` / `pii_provider_id` plus a top-level `version` integer for forward compatibility); API keys SHALL NOT appear in this file (they remain exclusively in the OS keyring per the trust boundary defined above).

Write semantics: each mutation endpoint (`POST /settings/providers`, `DELETE /settings/providers/{id}`, `PUT /settings/bindings`, `PUT /settings/pii-mode`) SHALL persist the post-mutation snapshot via an atomic write — serialize to `<path>.tmp`, then `os.replace()` into place — so a crash mid-write never leaves a partial JSON behind. Disk-write failure (e.g. `OSError`) SHALL be logged at error level but MUST NOT cause the mutation to return 5xx; the in-memory state already reflects the user's intent and the next successful mutation will retry the disk write.

Read semantics: `create_app` SHALL call `load_llm_config_or_default()` at boot to populate the initial snapshot. Three failure modes SHALL all fall back to an empty default (`providers=()`, `bindings={}`, `pii_mode='rule'`, `pii_provider_id=None`) plus a warning log: file missing (first install), JSON parse error (corrupt / truncated), and schema validation error (manually edited to invalid shape). A corrupt file MUST NOT brick sidecar boot.

#### Scenario: Mutation persists snapshot to disk

- **WHEN** the renderer calls `POST /settings/providers` with a valid body
- **THEN** the response status MUST be 204
- **AND** `~/.codebus/llm-config.json` MUST exist after the call returns
- **AND** the file's `providers[]` array MUST contain the upserted entry with the same `id` / `type` / `model` / `base_url`

#### Scenario: Boot rehydrates snapshot from existing file

- **WHEN** `~/.codebus/llm-config.json` exists with a valid schema before sidecar boot
- **THEN** `create_app` MUST initialize `app.state.provider_pool_snapshot` from the file's contents
- **AND** the next `GET /settings/providers` MUST return the rehydrated providers and bindings

#### Scenario: Persisted file never contains api_key

- **WHEN** any number of mutations have run against the settings endpoints
- **THEN** the contents of `~/.codebus/llm-config.json` MUST NOT contain any field named `api_key` or `apiKey`
- **AND** API keys MUST remain exclusively in the OS keyring (verified by reading the file as text and matching the substring `api_key` case-insensitively)

#### Scenario: Corrupt file falls back to empty default at boot

- **WHEN** `~/.codebus/llm-config.json` exists but contains malformed JSON
- **THEN** `create_app` MUST NOT raise
- **AND** `app.state.provider_pool_snapshot` MUST be initialized to the empty default (`providers=()`, `bindings={}`, `pii_mode='rule'`)
- **AND** a warning MUST be emitted to the sidecar log (operator visibility)

#### Scenario: Disk-write failure does not 5xx the mutation

- **WHEN** `POST /settings/providers` succeeds at the in-memory layer but the subsequent disk write raises `OSError` (e.g. `ENOSPC`)
- **THEN** the response status MUST still be 204
- **AND** an error-level message MUST be logged
- **AND** the in-memory snapshot MUST still reflect the successful mutation so the next mutation can retry the disk write

<!-- @trace
source: phase7-onboarding-polish
updated: 2026-05-02
code:
  - sidecar/src/codebus_agent/api/__init__.py
  - web/dist
  - sidecar/src/codebus_agent/api/settings.py
  - web/app/utils/provider-tos.ts
  - tauri/src-tauri/src/lib.rs
  - tauri/src-tauri/Cargo.toml
  - web/app/components/settings/ProviderEditModal.vue
  - web/app/pages/onboarding/providers.vue
  - tauri/src-tauri/capabilities/default.json
  - web/app/components/settings/EmbeddingChangeConfirmModal.vue
  - web/app/pages/onboarding/welcome.vue
  - sidecar/src/codebus_agent/api/startup_config.py
  - web/app/plugins/sidecar-startup-config.client.ts
  - web/app/pages/onboarding/done.vue
  - web/app/utils/external-link.ts
  - web/app/components/settings/RoleBindingTable.vue
  - web/app/components/settings/ProviderPoolList.vue
  - web/nuxt.config.ts
  - web/package.json
  - sidecar/src/codebus_agent/config/llm_config_store.py
  - web/app/pages/settings.vue
  - sidecar/src/codebus_agent/auth/paths.py
  - web/app/components/settings/PiiModeToggle.vue
tests:
  - sidecar/tests/api/test_startup_config.py
  - web/tests/utils/provider-tos.spec.ts
  - web/tests/settings/EmbeddingChangeConfirmModal.spec.ts
  - sidecar/tests/test_cors_preflight_smoke.py
  - sidecar/tests/config/test_llm_config_store.py
  - sidecar/tests/api/test_healthz_dependency.py
  - sidecar/tests/api/test_settings_persistence.py
  - web/tests/onboarding/welcome.spec.ts
  - web/tests/onboarding/providers.spec.ts
-->
