## MODIFIED Requirements

### Requirement: Settings UI Endpoint Section

The Settings modal SHALL render an Endpoint section that lets the user configure the Claude Code endpoint profile schema and manage the Azure API key entirely from the GUI. The section heading SHALL read `Claude Code endpoint settings` (or the locale-specific translation) and SHALL NOT include a provider-selector control — single-implementation selectors are out of scope until a second provider is integrated.

The section SHALL contain three controls plus two sub-sections:

1. An `active` radio group with exactly two options, `system` and `azure`. Selecting an option SHALL mutate the in-memory `claude_code.active` field; it SHALL NOT clear any field values in the non-selected profile sub-section.
2. A System Profile sub-section containing four verb rows (`goal` / `query` / `fix` / `verify`). Each row SHALL contain a `model` `<select>` with exactly four `<option>` values (`opus-4-7`, `opus-4-6`, `haiku-4-5`, `sonnet-4-6`) AND an `effort` `<select>` with exactly six `<option>` values (`low`, `medium`, `high`, `xhigh`, `max`, `auto`). The `verify` row SHALL be visually positioned after the `fix` row to convey the "verification follows main action" sequence.
3. An Azure Profile sub-section containing: a `base_url` text input, a `keyring_service` text input (pre-filled with the default `codebus-azure` when the underlying config field is empty or absent), an API-key status indicator (`Set` / `Unset`) with `Set new...` and `Delete` action buttons, AND four verb rows each containing a free-text `model` (deployment name) input AND an `effort` `<select>` with exactly six `<option>` values (`low`, `medium`, `high`, `xhigh`, `max`, `auto`).

Both profile sub-sections SHALL be present in the DOM regardless of the `active` value, organised as an **accordion**: the sub-section whose name matches `active` SHALL be expanded; the other sub-section SHALL be collapsed showing only its header (which SHALL include an `(inactive)` label). The user SHALL be able to click the collapsed header to expand the inactive sub-section and edit cold-storage configuration. When the user toggles the `active` radio, the newly-active sub-section SHALL auto-expand AND the previously-active sub-section SHALL auto-collapse; this auto-folding SHALL NOT delete or reset any form input values (collapsed inputs remain in the DOM, hidden via CSS, so values persist).

The `Save` button at the Settings modal level SHALL persist only yaml content via `save_global_config`; it SHALL NOT carry the Azure API key. The API key SHALL flow exclusively through the three `*_endpoint_key` IPC commands triggered by the `Set new...` / `Delete` action buttons.

The Settings UI SHALL NOT include a Test Connection / endpoint reachability button — verification SHALL require running `codebus query "ping"` from the terminal.

The Settings modal SHALL perform client-side validation of the `claude_code` block before allowing the user to save. Specifically: when `active === "azure"`, all of `base_url`, `keyring_service`, AND each verb's `model` (deployment name) SHALL be non-empty strings (trimmed), where "each verb" means all four of `goal`, `query`, `fix`, `verify`. Additionally, every verb's `effort` field (in BOTH `system` and `azure` profiles, regardless of which is active, for all four verbs) SHALL be one of the six values `low`, `medium`, `high`, `xhigh`, `max`, `auto` — values outside this set (including the empty string and any legacy value loaded from yaml) SHALL be treated as invalid. When any validation rule fails, the modal SHALL disable the Save button AND SHALL render an inline validation summary listing each failing field AND SHALL apply `aria-invalid="true"` to the offending inputs. The validation rules SHALL match the codebus-core `Endpoint Profile Schema` validation so the frontend and `save_global_config` backend gate produce the same reject/accept decision for fields covered by the backend (note: the backend keeps `effort` as a freeform `String` for yaml backward compatibility, so the effort enum constraint is enforced only at the UI layer).

The Settings UI SHALL preserve a loaded `effort` value verbatim in in-memory state even when that value is not in the enum, so that legacy yaml content (e.g. `effort: super-high` written by an earlier version or hand-edit) is not silently coerced or discarded; the value SHALL surface through the validation summary so the user re-selects a valid value before Save becomes enabled. The `<select>` trigger for an invalid effort value SHALL render with no option visually selected (empty trigger label).

#### Scenario: Save button is disabled when active=azure has empty required fields

- **WHEN** `claude_code.active === "azure"` AND `claude_code.azure.base_url` is the empty string (or any required azure field is empty, including `claude_code.azure.verify.model`) AND the user has edited any setting (dirty)
- **THEN** the Save button SHALL be disabled AND the Endpoint section SHALL render an inline validation summary listing the failing fields

#### Scenario: Empty azure field gets aria-invalid

- **WHEN** `claude_code.active === "azure"` AND `claude_code.azure.goal.model` is the empty string
- **THEN** the `azure-deployment-goal` input SHALL have `aria-invalid="true"` AND the validation summary SHALL list `claude_code.azure.goal.model`

#### Scenario: Empty azure verify field gets aria-invalid

- **WHEN** `claude_code.active === "azure"` AND `claude_code.azure.verify.model` is the empty string
- **THEN** the `azure-deployment-verify` input SHALL have `aria-invalid="true"` AND the validation summary SHALL list `claude_code.azure.verify.model`

#### Scenario: Save button enables when active=azure becomes fully populated

- **WHEN** `claude_code.active === "azure"` AND all azure required fields are non-empty (all four verbs' models, base_url, keyring_service) AND every verb's effort in both profiles (all four verbs) is one of `low` / `medium` / `high` / `xhigh` / `max` / `auto` AND the user has edited any setting (dirty)
- **THEN** the Save button SHALL be enabled AND the Endpoint section SHALL NOT render a validation summary

#### Scenario: Active radio switch preserves non-active profile inputs

- **WHEN** the user has typed `https://example.com/anthropic` into the azure `base_url` input AND `active` is currently `system` AND the user toggles `active` to `azure` then back to `system`
- **THEN** the azure `base_url` input SHALL still contain `https://example.com/anthropic` (the value SHALL NOT be cleared by the toggle) — even though auto-fold collapses the azure sub-section back to a header when `active` returns to `system`

#### Scenario: Initial render collapses the non-active sub-section

- **WHEN** the Endpoint section first renders AND `claude_code.active` is `system`
- **THEN** the System Profile sub-section SHALL be expanded (its verb rows and inputs visible) AND the Azure Profile sub-section SHALL be collapsed (only its header with `(inactive)` label visible)

#### Scenario: User can expand inactive sub-section to edit cold storage

- **WHEN** `active` is `system` AND the user clicks the Azure Profile collapsed header
- **THEN** the Azure Profile sub-section SHALL expand revealing its inputs AND the System Profile sub-section SHALL remain expanded (the user-driven expansion of the inactive sub-section SHALL NOT collapse the active one)

#### Scenario: Toggling active auto-collapses the previously-active sub-section

- **WHEN** the System Profile is expanded (active) AND the user toggles `active` to `azure`
- **THEN** the Azure Profile SHALL expand AND the System Profile SHALL collapse to its header — but the System Profile verb model dropdowns and effort dropdowns SHALL remain in the DOM (hidden via CSS) so their values persist across toggles

#### Scenario: System model dropdown lists exactly four versioned options

- **WHEN** the System Profile sub-section is rendered AND the user opens any of the four verb `model` dropdowns
- **THEN** the dropdown SHALL list exactly four options whose `value` attributes are `opus-4-7`, `opus-4-6`, `haiku-4-5`, `sonnet-4-6` in that order

#### Scenario: System effort dropdown lists exactly six options

- **WHEN** the System Profile sub-section is rendered AND the user opens any of the four verb `effort` dropdowns
- **THEN** the dropdown SHALL list exactly six options whose `value` attributes are `low`, `medium`, `high`, `xhigh`, `max`, `auto` in that order

#### Scenario: Azure effort dropdown lists exactly six options

- **WHEN** the Azure Profile sub-section is rendered AND the user opens any of the four verb `effort` dropdowns
- **THEN** the dropdown SHALL list exactly six options whose `value` attributes are `low`, `medium`, `high`, `xhigh`, `max`, `auto` in that order AND the option set SHALL be identical to the System Profile effort dropdown

#### Scenario: Legacy invalid effort value renders empty select trigger and flags validation

- **WHEN** `~/.codebus/config.yaml` loads with `claude_code.system.goal.effort` set to `super-high` (a value outside the enum) AND the user opens the Settings modal
- **THEN** the `system-effort-goal` `<select>` trigger SHALL render with no option visually selected (empty trigger label) AND the in-memory state SHALL retain the value `super-high` verbatim AND the validation summary SHALL list `claude_code.system.goal.effort` AND the `<select>` SHALL have `aria-invalid="true"` AND the Save button SHALL be disabled

#### Scenario: Selecting a valid effort clears the invalid flag and enables Save

- **WHEN** the Settings modal is open with `claude_code.system.goal.effort` equal to the invalid value `super-high` AND the Save button is disabled AND the user selects `medium` from the `system-effort-goal` dropdown AND no other fields are invalid
- **THEN** the in-memory state SHALL update `claude_code.system.goal.effort` to `medium` AND the validation summary SHALL no longer list `claude_code.system.goal.effort` AND the `<select>` SHALL NOT have `aria-invalid` AND the Save button SHALL be enabled

#### Scenario: Inactive profile invalid effort still blocks Save

- **WHEN** `claude_code.active === "system"` AND every system verb effort is a valid enum value AND every azure required field is populated AND `claude_code.azure.fix.effort` equals `extreme` (a value outside the enum)
- **THEN** the Save button SHALL be disabled AND the validation summary SHALL list `claude_code.azure.fix.effort` AND the `azure-effort-fix` `<select>` SHALL have `aria-invalid="true"`

#### Scenario: Verify row renders and behaves identically to other verb rows

- **WHEN** the System Profile sub-section is rendered
- **THEN** a `verify` verb row SHALL be present AND its `model` dropdown SHALL list the same four versioned options as the other verb rows AND its `effort` dropdown SHALL list the same six options AND user interaction (selecting model or effort) SHALL mutate `claude_code.system.verify` in the same shape as the other verb rows

#### Scenario: Azure verify deployment-name input renders and validates

- **WHEN** the Azure Profile sub-section is rendered AND `claude_code.active === "azure"` AND `claude_code.azure.verify.model` is the empty string
- **THEN** a `verify` verb row SHALL be present containing a free-text deployment-name input identified as `azure-deployment-verify` AND the input SHALL have `aria-invalid="true"` AND the validation summary SHALL list `claude_code.azure.verify.model` AND the Save button SHALL be disabled

#### Scenario: Azure keyring_service input is pre-filled when config field is empty

- **WHEN** `~/.codebus/config.yaml` either does not exist OR exists with `claude_code.azure.keyring_service` empty / absent AND the user opens the Settings modal
- **THEN** the Azure `keyring_service` input SHALL display the value `codebus-azure`

#### Scenario: Set new... button opens key entry modal

- **WHEN** the user clicks the `Set new...` button in the Azure Profile sub-section
- **THEN** a modal SHALL open containing a password-masked `<input type="password">` AND a `Confirm` button AND a `Cancel` button

#### Scenario: Confirming the key entry modal stores the key without persisting it client-side

- **WHEN** the user enters `sk-modal-test` into the password input AND clicks `Confirm`
- **THEN** the modal SHALL invoke `set_endpoint_key("azure", "sk-modal-test")` AND on success the modal SHALL close AND the API-key status indicator SHALL update to `Set` AND no DOM element OR app state SHALL retain the entered key value

#### Scenario: Delete button removes the keyring entry and updates status

- **WHEN** the API-key status indicator currently shows `Set` AND the user clicks the `Delete` button
- **THEN** the UI SHALL invoke `delete_endpoint_key("azure")` AND on success the status indicator SHALL update to `Unset`

#### Scenario: Save button does not transmit the API key

- **WHEN** the user has made any edits to the System / Azure profile fields AND clicks the `Save` button
- **THEN** the resulting `save_global_config` payload SHALL contain the edited `claude_code` block (with all four verb sub-blocks including `verify`) AND SHALL NOT contain any key, field, or string value matching the Azure API key value

<!-- @trace
source: endpoint-effort-dropdown, verify-stage-independent-model
updated: 2026-05-20
code:
  - codebus-app/src/components/settings/EndpointSection.tsx
  - codebus-app/src/lib/ipc.ts
tests:
  - codebus-app/src/lib/ipc.effort.test.ts
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-app/src/components/settings/EndpointSection.test.tsx
-->
