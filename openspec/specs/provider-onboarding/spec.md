# provider-onboarding Specification

## Purpose

TBD - created by archiving change 'provider-settings-and-onboarding'. Update Purpose after archive.

## Requirements

### Requirement: Onboarding wizard exposes three sequential routes

The frontend SHALL ship three pages under `web/app/pages/onboarding/`:

1. `welcome.vue` rendered at `/onboarding/welcome` — pure copy: codebus introduction + a generic statement that the app needs an LLM provider to function. The page MUST NOT include any provider-specific terms-of-service link; provider-specific ToS belongs on `/onboarding/providers` per the contextual rule below.
2. `providers.vue` rendered at `/onboarding/providers` — two side-by-side forms (chat provider + embedding provider) each with type / model / base_url / api_key fields. Each form MUST display a provider-specific terms-of-service link adjacent to the form, derived from the form's selected `type` value. The mapping from `type` to ToS URL MUST be sourced from a single per-app constant (e.g., `PROVIDER_TYPE_TOS_URL`) so future provider types can be added without touching `welcome.vue` or `done.vue`. For the P0 types `openai_chat` and `openai_embedding`, both MUST resolve to OpenAI's terms-of-use URL.
3. `done.vue` rendered at `/onboarding/done` — confirmation copy + a single CTA button "Start" that routes to `/`.

Each page MUST contain a single "Next" button (or "Start" on the last page) that is disabled until the page's required fields are filled. The pages MUST NOT contain a "Skip" button or any escape hatch other than browser navigation.

#### Scenario: Welcome page next button always enabled

- **WHEN** the user lands on `/onboarding/welcome`
- **THEN** the "Next" button MUST be enabled (no fields to fill)
- **AND** clicking it MUST route to `/onboarding/providers`

#### Scenario: Welcome page contains no provider-specific ToS link

- **WHEN** the user lands on `/onboarding/welcome`
- **THEN** the rendered DOM MUST NOT contain any anchor whose `href` resolves to a provider's terms-of-service URL (e.g., `openai.com/policies/terms-of-use`, `anthropic.com/legal/...`)
- **AND** any legal-acknowledgement copy MUST be phrased provider-agnostically (e.g., "review your provider's terms of service before continuing" without naming a specific provider)

#### Scenario: Providers page next disabled until both forms valid

- **WHEN** the user lands on `/onboarding/providers` without prior input
- **THEN** the "Next" button MUST be disabled
- **AND** entering valid `id` / `type` / `model` / `base_url` / `api_key` for the chat form alone MUST keep the button disabled
- **AND** entering valid values for both chat and embedding forms MUST enable the button

#### Scenario: Providers page renders contextual ToS link per type

- **WHEN** the user is on `/onboarding/providers` and the chat form's `type` is `openai_chat`
- **THEN** the chat form MUST render a visible anchor element whose `href` resolves to the URL mapped from `openai_chat` in the per-app `PROVIDER_TYPE_TOS_URL` constant
- **AND** the same MUST hold for the embedding form when its `type` is `openai_embedding`
- **AND** if a future `type` is added to the app without a corresponding entry in `PROVIDER_TYPE_TOS_URL`, the form MUST either omit the ToS link entirely or render a generic "review your provider's terms" placeholder — it MUST NOT render a broken or default-OpenAI link

#### Scenario: Done page Start button routes to entry page

- **WHEN** the user reaches `/onboarding/done` and clicks "Start"
- **THEN** the page MUST route to `/`
- **AND** the route MUST NOT redirect back to `/onboarding/welcome` because `/healthz.dependency` MUST now report all required lanes ready


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
### Requirement: Onboarding writes through keyring and provider config in correct order

When the user clicks "Next" on `/onboarding/providers`, the frontend SHALL execute these steps in order:

1. Call `keyring_set` for the chat provider; abort with displayed error if it fails
2. Call `keyring_set` for the embedding provider; abort with displayed error if it fails
3. Call `useProviderConfig().upsertProvider` for the chat provider entry (no api_key in payload)
4. Call `useProviderConfig().upsertProvider` for the embedding provider entry
5. Call `useProviderConfig().setBinding('reasoning', chat_provider_id)`
6. Call `useProviderConfig().setBinding('judge', chat_provider_id)`
7. Call `useProviderConfig().setBinding('chat', chat_provider_id)`
8. Call `useProviderConfig().setBinding('embed', embed_provider_id)`
9. Route to `/onboarding/done`

If any step in 1–8 fails, the wizard MUST stop, display the error, and remain on `/onboarding/providers`. Steps 1 and 2 are atomic-ish (keyring writes), and steps 3–8 are idempotent on the sidecar mutation endpoints, so partial completion does not corrupt state — a retry simply re-applies the same writes.

#### Scenario: Chat keyring failure aborts before embedding

- **WHEN** the user submits valid forms but step 1 (chat `keyring_set`) fails
- **THEN** the wizard MUST display the error message
- **AND** step 2 (embedding `keyring_set`) MUST NOT be attempted
- **AND** the user MUST remain on `/onboarding/providers`

#### Scenario: Successful submission routes to done

- **WHEN** all steps 1–8 succeed
- **THEN** the user MUST be routed to `/onboarding/done`
- **AND** subsequent calls to `/healthz` MUST report `dependency.llm_chat: ready`, `dependency.llm_embed: ready`

---
### Requirement: Startup detection redirects to onboarding when any LLM dependency is not configured

The frontend SHALL register a Nuxt route middleware that runs on every navigation into a non-onboarding route. The middleware MUST call `GET /healthz` and inspect the `dependency` field. If any of the keys `llm_chat` / `llm_embed` reports `not-configured`, the middleware MUST redirect the navigation to `/onboarding/welcome`.

The middleware MUST NOT run on routes matching `/onboarding/*` (the wizard itself must be reachable while the dependency is `not-configured`).

The middleware MUST NOT run on the bare `/healthz` test page (if any exists) — that path is reserved for diagnostic surfaces.

#### Scenario: Cold app start with empty keyring redirects to onboarding

- **WHEN** the user opens the app for the first time and the keyring contains no API keys, then navigates to `/`
- **THEN** the middleware MUST receive `dependency.llm_chat: "not-configured"` from `/healthz`
- **AND** the navigation MUST redirect to `/onboarding/welcome`

#### Scenario: Manual URL paste into tutorial route redirects when not configured

- **WHEN** the user pastes `/tutorial/ws_xxx/s02-mqtt-client` while the keyring is empty
- **THEN** the middleware MUST redirect to `/onboarding/welcome` before the page mounts

#### Scenario: Onboarding routes themselves never redirect

- **WHEN** the user is on `/onboarding/welcome` (with empty keyring) and the middleware fires
- **THEN** no redirect MUST occur
- **AND** the wizard page MUST render normally

#### Scenario: Browser back from onboarding to entry redirects again

- **WHEN** the user is on `/onboarding/providers` and presses the browser back button to navigate to `/`
- **THEN** the middleware MUST detect the still-`not-configured` state and redirect back to `/onboarding/welcome`
