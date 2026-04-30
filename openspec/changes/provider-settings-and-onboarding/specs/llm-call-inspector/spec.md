## ADDED Requirements

### Requirement: LlmCallInspector renders provider id and filters PII detection role

The `<LlmCallInspector>` component SHALL render the `provider_id` field of the active `LlmCallEntry` in the status strip alongside the existing `role` / `module` / `model` badges. The provider id MUST appear as a chip with `data-testid="llm-inspector-provider-id"` and MUST display the literal id (e.g., `openai-default`) — not the resolved `base_url` or any derived nickname.

The component SHALL accept a new prop `hidePiiDetection: boolean` (default `true`) which, when true, causes the inspector to skip rows whose `role === "pii_detection"` from the prev/next navigation chain. When `hidePiiDetection` is `false`, all rows in the input array participate in navigation regardless of role.

The component SHALL display, in the header region below the prev/next buttons, a small banner of the form `"+ N PII detection call(s) hidden"` whenever `hidePiiDetection === true` and at least one `pii_detection` row exists in the input. The banner MUST be a `<button>` element with `data-testid="llm-inspector-toggle-pii"`; clicking it MUST emit a new event `(e: 'toggle-pii-visible'): void` so the parent page can flip the prop.

#### Scenario: Provider id chip rendered

- **WHEN** the inspector is open with an entry whose `provider_id == "openai-default"`
- **THEN** the rendered DOM MUST contain an element matching `[data-testid="llm-inspector-provider-id"]`
- **AND** that element's text content MUST equal `openai-default`

#### Scenario: PII rows excluded from navigation by default

- **WHEN** the inspector is mounted with rows containing 3 chat entries and 2 pii_detection entries (default `hidePiiDetection: true`)
- **THEN** clicking next from the last chat entry MUST clamp at the last chat entry, not advance into a pii_detection entry
- **AND** the rendered count display MUST read `3 / 3` (chat rows only), not `5 / 5`

#### Scenario: Toggle button surfaces hidden count

- **WHEN** rows include 2 pii_detection entries and `hidePiiDetection === true`
- **THEN** the inspector MUST render a button with `data-testid="llm-inspector-toggle-pii"`
- **AND** the button text content MUST contain the literal `"2"` (the hidden count)

#### Scenario: Toggle emits event

- **WHEN** the user clicks the toggle button
- **THEN** the inspector MUST emit `toggle-pii-visible` with no payload

### Requirement: AuditPanel filters llm tab rows by role for PII separation

The `<AuditPanel>` component SHALL accept a new optional prop `hidePiiDetection: boolean` (default `true`). When the active tab is `llm` and `hidePiiDetection === true`, the rows passed to the panel MUST be filtered to exclude `role === "pii_detection"` entries before count display, row rendering, and selection events. The panel MUST display the same toggle banner (mirroring the inspector) at the top of the body region when at least one pii_detection row exists; clicking the banner MUST emit a new event `(e: 'toggle-pii-visible'): void`.

The other six audit tabs (`sanitize`, `tool`, `reasoning`, `token`, `kb_growth`, `generator`) MUST NOT be affected by this prop — they do not carry a `role: "pii_detection"` concept.

#### Scenario: llm tab count excludes pii rows by default

- **WHEN** `<AuditPanel :active-tab="'llm'" :rows="[...]" :counts="{ ...counts, llm: 5 }" />` is rendered with 3 chat rows and 2 pii_detection rows
- **THEN** the displayed row count for the `llm` tab MUST read `3` (not `5`)
- **AND** the rendered list MUST contain exactly 3 row elements

#### Scenario: Sanitize tab unaffected

- **WHEN** `<AuditPanel :active-tab="'sanitize'" :hide-pii-detection="true" />` is rendered
- **THEN** the panel MUST behave identically to the case where `hide-pii-detection` is omitted
- **AND** no PII-related toggle banner MUST appear on the sanitize tab
