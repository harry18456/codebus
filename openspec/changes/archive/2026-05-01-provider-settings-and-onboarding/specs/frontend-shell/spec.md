## ADDED Requirements

### Requirement: TopBar exposes a settings entry routed to /settings

The frontend SHALL render a settings entry button (gear icon) in `<TopBar>`. Clicking the button MUST route to `/settings` via the Vue Router. The existing `open-settings` emit on `<TopBar>` MUST be wired by the layout host to a router push handler so the existing event signature stays intact while the actual navigation happens at the layout level.

The settings entry MUST appear on every layout-level page (`/tutorial/...` / `/explorer/...` / `/audit/...` / `/settings`) but MUST NOT appear on `/onboarding/*` routes — the onboarding wizard explicitly does not allow escape into other UI surfaces (see `provider-onboarding` Requirement "Onboarding wizard exposes three sequential routes").

#### Scenario: Settings button visible on tutorial page

- **WHEN** the user is on `/tutorial/ws_xxx/index`
- **THEN** `<TopBar>` MUST render a button with `data-testid="topbar-settings"`
- **AND** clicking it MUST route to `/settings`

#### Scenario: Settings button hidden on onboarding routes

- **WHEN** the user is on `/onboarding/welcome`, `/onboarding/providers`, or `/onboarding/done`
- **THEN** `<TopBar>` MUST NOT render any button with `data-testid="topbar-settings"`

### Requirement: useProviderConfig composable exposes provider pool state

The frontend SHALL ship `web/app/composables/useProviderConfig.ts` as a module-level singleton (matching the `useQaSession` / `useIntervention` convention). The composable MUST expose:

- `providers: Ref<ProviderEntry[]>` — read-only snapshot of the provider pool
- `bindings: Ref<{ reasoning: string; judge: string; chat: string; embed: string }>` — current role bindings
- `piiMode: Ref<{ mode: 'rule' | 'llm'; provider_id: string | null }>`
- `loadConfig(): Promise<void>` — fetches `/settings/providers` and updates state
- `upsertProvider(entry: ProviderEntry): Promise<void>` — POSTs to `/settings/providers`
- `deleteProvider(id: string): Promise<void>` — DELETEs `/settings/providers/{id}`
- `setBinding(role: string, provider_id: string): Promise<void>` — PUTs `/settings/bindings`
- `setPiiMode(mode: 'rule' | 'llm', provider_id?: string): Promise<void>` — PUTs `/settings/pii-mode`

The composable MUST subscribe to the app-level `provider_config_changed` SSE event and re-fetch state automatically on receipt. The composable MUST NOT cache API keys; all `api_key` flows go through Tauri keyring IPC directly without crossing this composable.

#### Scenario: Two callers receive same singleton state

- **WHEN** two components both call `useProviderConfig()`
- **THEN** the returned `providers` / `bindings` / `piiMode` refs MUST satisfy `Object.is(a.providers, b.providers) === true` for all three

#### Scenario: SSE event triggers re-fetch

- **WHEN** the composable is mounted and the app-level SSE channel emits `provider_config_changed`
- **THEN** the composable MUST issue a `GET /settings/providers` request within 100 ms
- **AND** the local refs MUST update once the response arrives

#### Scenario: useProviderConfig source has no api_key field

- **WHEN** the test suite greps `web/app/composables/useProviderConfig.ts` for the literal string `api_key`
- **THEN** zero matches MUST be found in non-comment lines

### Requirement: Index page redirects to onboarding when LLM dependencies are not configured

The route entry `/` (rendered by `web/app/pages/index.vue`) SHALL call `GET /healthz` on mount and redirect via `router.replace('/onboarding/welcome')` when any of `dependency.llm_chat` / `dependency.llm_embed` reports `not-configured`. The page MUST NOT render its existing entry-point UI (workspace picker etc.) until the dependency snapshot confirms readiness.

This redirect logic is duplicated from the global Nuxt route middleware (`provider-onboarding` Requirement "Startup detection redirects to onboarding when any LLM dependency is not configured") because direct landing on `/` is a common path that benefits from inline gating without relying on middleware ordering.

#### Scenario: Empty keyring routes to onboarding

- **WHEN** the user opens the app for the first time and `pages/index.vue` mounts
- **THEN** the page MUST issue `GET /healthz`
- **AND** when the response has `dependency.llm_chat: "not-configured"`, the page MUST redirect to `/onboarding/welcome`
- **AND** the workspace picker UI MUST NOT render

#### Scenario: Configured keyring renders entry UI

- **WHEN** the user has completed onboarding and revisits `/`
- **THEN** the page MUST issue `GET /healthz`
- **AND** when the response has `dependency.llm_chat: "ready"` and `dependency.llm_embed: "ready"`, the page MUST render the existing workspace picker UI without redirecting
