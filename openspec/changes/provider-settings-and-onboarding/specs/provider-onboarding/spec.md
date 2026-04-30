## ADDED Requirements

### Requirement: Onboarding wizard exposes three sequential routes

The frontend SHALL ship three pages under `web/app/pages/onboarding/`:

1. `welcome.vue` rendered at `/onboarding/welcome` — pure copy: codebus introduction + "needs an LLM to function" + a link out to the OpenAI Terms of Service
2. `providers.vue` rendered at `/onboarding/providers` — two side-by-side forms (chat provider + embedding provider) each with type / model / base_url / api_key fields
3. `done.vue` rendered at `/onboarding/done` — confirmation copy + a single CTA button "Start" that routes to `/`

Each page MUST contain a single "Next" button (or "Start" on the last page) that is disabled until the page's required fields are filled. The pages MUST NOT contain a "Skip" button or any escape hatch other than browser navigation.

#### Scenario: Welcome page next button always enabled

- **WHEN** the user lands on `/onboarding/welcome`
- **THEN** the "Next" button MUST be enabled (no fields to fill)
- **AND** clicking it MUST route to `/onboarding/providers`

#### Scenario: Providers page next disabled until both forms valid

- **WHEN** the user lands on `/onboarding/providers` without prior input
- **THEN** the "Next" button MUST be disabled
- **AND** entering valid `id` / `type` / `model` / `base_url` / `api_key` for the chat form alone MUST keep the button disabled
- **AND** entering valid values for both chat and embedding forms MUST enable the button

#### Scenario: Done page Start button routes to entry page

- **WHEN** the user reaches `/onboarding/done` and clicks "Start"
- **THEN** the page MUST route to `/`
- **AND** the route MUST NOT redirect back to `/onboarding/welcome` because `/healthz.dependency` MUST now report all required lanes ready

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
