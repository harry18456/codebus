## MODIFIED Requirements

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
