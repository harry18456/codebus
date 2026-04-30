## MODIFIED Requirements

### Requirement: Health endpoint

The sidecar SHALL expose `GET /healthz` returning a JSON payload reflecting its readiness state. The response body SHALL include a top-level `status` field (`"ok"` or `"degraded"`) and a top-level `dependency` field carrying per-lane readiness (`llm_chat` / `llm_embed` / `pii` plus the existing infrastructure dependencies such as `qdrant`). Each lane MUST report one of `"ready"` / `"not-configured"` / `"unreachable"`.

A lane reports `not-configured` when the corresponding `llm.bindings.<role>.provider_id` resolves to a provider that has no API key in `app.state.provider_keys` (the in-memory dict populated by `POST /internal/startup-config`). A lane reports `unreachable` when the API key is present but a smoke check (e.g., embedding model availability for `llm_embed`, model list for `llm_chat`) fails. A lane reports `ready` only when both checks pass.

#### Scenario: Healthy state with all lanes ready

- **WHEN** `GET /healthz` is called with a valid bearer and every lane's smoke check succeeds
- **THEN** the response status MUST be 200
- **AND** the body MUST contain `{"status": "ok", "dependency": { "llm_chat": "ready", "llm_embed": "ready", "pii": "ready", ... }}`

#### Scenario: Degraded state with unreachable infrastructure dependency

- **WHEN** `GET /healthz` is called with a valid bearer and Qdrant is unreachable
- **THEN** the response status MUST be 200
- **AND** the body MUST contain `"status": "degraded"`
- **AND** the body MUST contain `dependency.qdrant: "unreachable"` and the rest of the lane keys per their actual state

#### Scenario: not-configured lane after fresh install

- **WHEN** `GET /healthz` is called with a valid bearer immediately after a sidecar boot where no `POST /internal/startup-config` was made
- **THEN** the response status MUST be 200
- **AND** the body MUST contain `dependency.llm_chat: "not-configured"`
- **AND** the body MUST contain `dependency.llm_embed: "not-configured"`

## ADDED Requirements

### Requirement: Sidecar accepts provider config mutation endpoints

The sidecar SHALL register an internal mutation router that exposes the following bearer-authenticated endpoints used by the settings page:

- `GET /settings/providers` returns `{ providers: [...], bindings: {...}, pii_mode: "rule" | "llm" }` reflecting the in-memory snapshot. API keys MUST NOT appear in the response.
- `POST /settings/providers` upserts an entry in `llm.providers[]` (body schema `{ id, type, model, base_url }` — no `api_key`). On success the sidecar MUST emit SSE event `provider_config_changed` and persist the config (without API keys) to disk.
- `DELETE /settings/providers/{id}` removes an entry. Returns HTTP 409 with `code: PROVIDER_BOUND_TO_ROLE` and the bound role names if the provider id is referenced in `llm.bindings.<role>.provider_id` for any role.
- `PUT /settings/bindings` updates `llm.bindings` (body schema `{ reasoning, judge, chat, embed: <provider_id> }`). On success the sidecar MUST swap the active `RegistryHolder` reference atomically and emit `provider_config_changed`.
- `PUT /settings/pii-mode` updates `llm.pii.mode` (body schema `{ mode: "rule" | "llm", provider_id?: string }`). When `mode == "llm"` the `provider_id` MUST be present and MUST resolve to a provider type in `TrackedProvider.PII_ALLOWED_INNER_TYPES`; otherwise HTTP 400 with `code: INVALID_PII_PROVIDER`.

These endpoints MUST be guarded by the existing bearer middleware. They MUST NOT appear in the public OpenAPI document path entry list (set `include_in_schema=False`).

#### Scenario: Settings provider GET excludes API keys

- **WHEN** `GET /settings/providers` is called with a valid bearer
- **THEN** the response body MUST contain `providers[*]` entries
- **AND** no entry MUST contain an `api_key` field

#### Scenario: Settings binding PUT triggers RegistryHolder swap

- **WHEN** `PUT /settings/bindings` is called with a valid body
- **THEN** the in-memory `RegistryHolder` MUST be swapped to a new immutable `ProviderRegistry` instance reflecting the new bindings
- **AND** the sidecar MUST emit SSE event `provider_config_changed` on the app-level channel

#### Scenario: PII mode llm without provider_id rejected

- **WHEN** `PUT /settings/pii-mode` is called with body `{ mode: "llm" }` and no `provider_id`
- **THEN** the response status MUST be 400
- **AND** the body MUST contain `{ "detail": { "code": "INVALID_PII_PROVIDER" } }`

### Requirement: RegistryHolder enables atomic registry hot-swap

The sidecar SHALL provide a `RegistryHolder` class wrapping a single immutable `ProviderRegistry` reference. Code paths that retrieve providers MUST go through `holder.current()` which returns the registry reference under an `asyncio.Lock`. The holder MUST expose `swap(new_registry: ProviderRegistry)` which atomically replaces the current reference; in-flight callers that already hold a registry reference MUST continue using their captured reference until they finish, while subsequent `holder.current()` calls receive the new registry.

The holder MUST NOT mutate the inner `ProviderRegistry`; each swap creates a new immutable registry from the updated config snapshot.

#### Scenario: holder.current returns same instance until swap

- **WHEN** code calls `await holder.current()` twice in succession with no intervening `swap()`
- **THEN** the two calls MUST return the same `ProviderRegistry` instance (identity comparison)

#### Scenario: In-flight task continues with captured reference after swap

- **WHEN** an explorer task captures `holder.current()` and then a separate caller invokes `holder.swap(new_registry)`
- **THEN** the in-flight task's subsequent calls on its captured reference MUST continue using the old registry's providers
- **AND** a new task started after the swap MUST receive the new registry from `holder.current()`

#### Scenario: swap is atomic across concurrent reads

- **WHEN** N concurrent `await holder.current()` calls and one `holder.swap(new_registry)` call interleave
- **THEN** every concurrent caller MUST receive either the old or new registry — never a partially-constructed state — and the swap MUST complete in finite time

### Requirement: provider_config_changed SSE event surface

The sidecar SHALL broadcast SSE event `provider_config_changed` with payload `{ changed_roles: string[], embed_changed: boolean, providers_pool_changed: boolean }` whenever the provider pool, role bindings, or PII mode is mutated through the settings endpoints. The event MUST be emitted on an app-level SSE channel (not bound to a specific `task_id`) — sidecar SHALL register `GET /events?channel=app` for clients to subscribe; this channel MUST require a valid bearer.

The event MUST NOT carry any API key value, sensitive provider metadata (e.g., raw `base_url` is acceptable since it is not a secret), or any audit lane content. Multiple events MUST be coalesced when several mutations happen within a 50 ms window (single event with the union of changes).

#### Scenario: Binding change emits event with role list

- **WHEN** `PUT /settings/bindings` changes the `reasoning` and `chat` roles
- **THEN** the SSE channel `app` MUST receive exactly one event with `type: "provider_config_changed"` and `data.changed_roles == ["reasoning", "chat"]` (order-insensitive)

#### Scenario: Embed change sets embed_changed flag

- **WHEN** `PUT /settings/bindings` changes the `embed` role
- **THEN** the SSE event MUST include `data.embed_changed == true`

#### Scenario: Event carries no secrets

- **WHEN** the test inspects every emitted `provider_config_changed` event payload during a flow that mutates provider pool + bindings
- **THEN** the payload MUST NOT contain any `api_key` value
- **AND** the payload MUST NOT contain any `~/.codebus/` filesystem path

### Requirement: Config schema supports provider pool with role bindings

The sidecar config loader SHALL accept the new schema shape with `[[llm.providers]]` array entries (each carrying `id` / `type` / `model` / `base_url`) and a separate `[llm.bindings]` table mapping `reasoning` / `judge` / `chat` / `embed` roles to provider ids. The loader MUST also accept the legacy `[llm.roles]` shape and convert it into the new in-memory representation for backward compatibility, emitting a single deprecation warning per process start.

The loader MUST validate: (1) every binding's `provider_id` exists in the providers array, (2) the `embed` binding's referenced provider has `type` matching the embedding-shaped allowlist (`openai_embedding` and any future embedding type added to `TrackedProvider.ALLOWED_INNER_TYPES`), (3) `pii.mode == "llm"` requires `pii.provider_id` to reference a provider in the PII allowlist. Validation failures MUST raise `INVALID_PROVIDER_BINDING` / `INVALID_PROVIDER_TYPE` / `INVALID_PII_PROVIDER` at startup with the offending field name.

#### Scenario: New schema accepted

- **WHEN** the config file contains `[[llm.providers]]` entries plus `[llm.bindings]` table and the loader runs
- **THEN** the in-memory provider pool MUST contain entries matching every `[[llm.providers]]` block
- **AND** the in-memory bindings MUST match the `[llm.bindings]` table

#### Scenario: Legacy schema converted with deprecation warning

- **WHEN** the config file contains the legacy `[llm.roles.reasoning]` / `[llm.roles.embed]` shape
- **THEN** the loader MUST convert it into the new in-memory representation
- **AND** exactly one deprecation warning MUST be emitted to logs

#### Scenario: Binding referencing unknown provider rejected

- **WHEN** the config file contains `llm.bindings.reasoning = "does-not-exist"` with no matching provider id
- **THEN** the loader MUST raise an error with code `INVALID_PROVIDER_BINDING` and the offending role name in the message

#### Scenario: Embed binding to chat-typed provider rejected

- **WHEN** the config file contains `llm.bindings.embed = "openai-default"` and that provider's `type == "openai_chat"`
- **THEN** the loader MUST raise an error with code `INVALID_PROVIDER_TYPE`
