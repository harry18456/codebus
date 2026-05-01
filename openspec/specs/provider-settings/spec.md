# provider-settings Specification

## Purpose

TBD - created by archiving change 'provider-settings-and-onboarding'. Update Purpose after archive.

## Requirements

### Requirement: Settings page renders three sections

The frontend SHALL ship `web/app/pages/settings.vue` as a standalone route at `/settings`. The page MUST render exactly three sections in this top-to-bottom order, each rendered by a dedicated component:

1. **Provider pool** — `<ProviderPoolList>` listing every entry in `llm.providers[]` with edit / delete affordances and an "Add provider" button that opens `<ProviderEditModal>`
2. **Role bindings** — `<RoleBindingTable>` rendering rows for the four roles `reasoning` / `judge` / `chat` / `embed` and a dropdown per row letting the user pick from compatible providers (chat-shaped for the first three roles, embedding-shaped for `embed`)
3. **PII mode** — `<PiiModeToggle>` with two radio options `rule` (default, no extra config) and `llm` (requires picking a PII-allowlisted provider — disabled in P0 because no LLM PII provider is registered yet, but the radio MUST render so the future option is discoverable)

The page MUST NOT include any additional configuration sections (temperature, max_tokens, system prompt, retry policy, etc.) in P0; those are explicitly out of scope per the Non-Goals of `provider-settings-and-onboarding`.

#### Scenario: All three sections render in canonical order

- **WHEN** the user navigates to `/settings` after completing onboarding
- **THEN** the rendered DOM MUST contain exactly three `<section>` regions
- **AND** their `data-section` attribute values MUST equal `provider-pool`, `role-bindings`, `pii-mode` in that order

#### Scenario: PII LLM mode disabled in P0

- **WHEN** `<PiiModeToggle>` mounts
- **THEN** the radio button labeled `llm` MUST be visually disabled (CSS pointer-events:none + visual indicator)
- **AND** clicking it MUST NOT change the persisted PII mode

---
### Requirement: Provider pool CRUD touches keyring and config

The `<ProviderEditModal>` component SHALL accept fields `id` (regex `^[a-z][a-z0-9-]{2,40}$`), `type` (one of `openai_chat` / `openai_embedding` — extensible enum), `model` (string), `base_url` (https URL), `api_key` (input with reveal toggle). On Confirm the component MUST:

1. Call `keyring_set({ provider_id: <id>, api_key: <api_key> })` first; on failure, display the error and abort the save (do not write to config)
2. On keyring success, call `useProviderConfig().upsertProvider({ id, type, model, base_url })` which proxies to a sidecar mutation endpoint that updates the in-memory `llm.providers[]` and persists the config (without `api_key`) to disk
3. Trigger SSE event `provider_config_changed` from the sidecar so other open frontend instances see the new entry

Delete flow SHALL: (1) refuse if the provider is currently bound to any role (display "remove role binding first"), (2) call `keyring_delete` first, then `useProviderConfig().deleteProvider(id)`, (3) emit `provider_config_changed`.

#### Scenario: Save without keyring write does not update config

- **WHEN** the user submits `<ProviderEditModal>` and the keyring_set IPC fails with `KEYRING_ENTRY_MISSING` or any other error code
- **THEN** the modal MUST display the error message
- **AND** `useProviderConfig().upsertProvider` MUST NOT be called
- **AND** the in-memory provider pool snapshot MUST remain unchanged

#### Scenario: Delete bound provider blocked

- **WHEN** the user clicks delete on a provider whose `id` appears in `llm.bindings.<role>.provider_id` for any role
- **THEN** the UI MUST display a blocking message naming the bound role
- **AND** neither `keyring_delete` nor `deleteProvider` MUST be called

---
### Requirement: Role binding change propagates via hot-swap

The `<RoleBindingTable>` component SHALL render a dropdown per role; selecting a different provider id MUST trigger `useProviderConfig().setBinding(role, provider_id)` which calls the sidecar mutation endpoint, the sidecar swaps the active `RegistryHolder` reference, and the sidecar broadcasts `provider_config_changed` so all clients re-fetch the binding snapshot.

For the `embed` role the change MUST go through `<EmbeddingChangeConfirmModal>` first (see separate Requirement); for the other three roles the change is non-destructive and applies immediately.

#### Scenario: Non-embed role change applies without confirm modal

- **WHEN** the user changes the `reasoning` role binding from one provider to another in `<RoleBindingTable>`
- **THEN** the sidecar MUST receive the mutation request
- **AND** no confirm modal MUST be rendered
- **AND** the next LLM call dispatched on the `reasoning` role MUST use the new provider

#### Scenario: In-flight task continues with old binding

- **WHEN** a sidecar task is running (e.g., explorer or generator) and the user changes a non-embed role binding
- **THEN** the in-flight task MUST complete its remaining LLM calls using the registry reference it captured at task start
- **AND** the next task started after the swap MUST use the new binding

---
### Requirement: Embedding switch goes through destructive confirm modal

When the user changes the `embed` role binding, the frontend SHALL render `<EmbeddingChangeConfirmModal>` before applying the change. The modal MUST display:

1. A warning that switching embedding will rebuild the entire knowledge base
2. The current KB chunk count (read from a sidecar `GET /kb/stats` endpoint or equivalent state)
3. An estimated rebuild duration (computed from chunk count)
4. Cancel and Confirm buttons

Cancel MUST close the modal without applying the change. Confirm MUST: (a) call `useProviderConfig().setBinding('embed', new_provider_id)`, (b) trigger the sidecar KB rebuild SSE task, (c) display a banner on `/settings` page indicating the rebuild is in progress.

While KB rebuild is in progress the sidecar SHALL respond with HTTP 503 and `code: KB_REBUILD_IN_PROGRESS` to any of `/qa`, `/explore`, `/scan?stream=true`, `/kb/build` requests. The frontend components consuming those endpoints MUST display a "rebuilding KB" message instead of their normal error UI.

#### Scenario: Embedding change requires confirmation

- **WHEN** the user opens the `embed` role dropdown in `<RoleBindingTable>` and selects a different provider
- **THEN** `<EmbeddingChangeConfirmModal>` MUST render
- **AND** the binding MUST NOT be changed until the user clicks Confirm

#### Scenario: KB rebuild blocks dependent endpoints

- **WHEN** an embedding switch has been confirmed and the KB rebuild SSE task is running
- **THEN** any request to `/qa` MUST receive HTTP 503 with `code: KB_REBUILD_IN_PROGRESS`
- **AND** any request to `/explore` MUST receive the same response
- **AND** the response body MUST NOT contain the new or old `provider_id` value (preserving the no-leak invariant)
