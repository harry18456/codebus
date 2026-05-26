## ADDED Requirements

### Requirement: Settings Language Override

The codebus-app SHALL persist a user-selected locale override at `app.locale_override` in `~/.codebus/config.yaml` with three valid values: `"zh"`, `"en"`, or `null`. A `null` value (or an absent key, including in configs written by earlier versions) SHALL mean "auto-detect from the system locale".

The `useLocale` hook (`codebus-app/src/hooks/useLocale.ts`) SHALL resolve the active locale by this precedence, evaluated top-down on every render:

1. The `override` argument passed to `useLocale(override?: Locale)`, when non-nullish — this path SHALL remain available for tests to inject a deterministic locale
2. The `app.locale_override` value read reactively from the settings store, when non-`null`
3. Otherwise, `navigator.language`: a value beginning with `zh` (case-insensitive) SHALL resolve to `"zh"`; any other value (including when `navigator` is undefined) SHALL resolve to `"en"`

The settings store SHALL expose `app.locale_override` such that React components subscribing via the store hook re-render when the value changes, so changing the language selection in the Settings modal SHALL take effect immediately without restarting the application or remounting the React tree.

Changes to `app.locale_override` SHALL persist through the existing `save_global_config` / `load_global_config` IPC round-trip, so the selected locale SHALL be sticky across application restarts. Backend errors surfaced through `LocalizedError` (`codebus-app/src/i18n/errors.ts`) SHALL render in the active locale because the toast layer resolves them through `useT` / `useLocale` at display time; this requirement SHALL NOT require any imperative locale lookup in `errors.ts`.

A standalone synchronous helper `tStatic` that resolves locale outside the React tree is out of scope for this requirement and MAY continue to read `navigator.language` directly until a follow-up change wires it to the store.

#### Scenario: Language dropdown switches the UI reactively

- **GIVEN** the user has the Settings modal open and the active locale is `"zh"`
- **WHEN** the user selects "English" in the Language dropdown
- **THEN** the Settings modal contents, the Workspace background, and the Lobby background re-render in English without any restart, remount, or page reload

#### Scenario: Locale override survives application restart

- **GIVEN** the user has set the Language dropdown to "English" and clicked Save
- **WHEN** the user closes and relaunches the codebus-app
- **THEN** `~/.codebus/config.yaml` contains `app.locale_override: "en"` AND the relaunched app renders in English regardless of the system locale

#### Scenario: Auto option follows the system locale

- **GIVEN** `navigator.language` resolves to `zh-TW`
- **WHEN** the user sets the Language dropdown to "Auto" and clicks Save
- **THEN** `~/.codebus/config.yaml` contains `app.locale_override: null` AND the active locale resolves to `"zh"`

#### Scenario: Backend error toast follows the active locale

- **GIVEN** the user has the Language dropdown set to "English" and has saved
- **WHEN** the user triggers a backend error from the Settings modal (for example by submitting an invalid endpoint base URL)
- **THEN** the resulting toast renders the error message in English

#### Scenario: Hook override argument outranks the store

- **GIVEN** `app.locale_override` in the settings store is `"en"`
- **WHEN** a component calls `useLocale("zh")` directly (typically a test injecting a deterministic locale)
- **THEN** the call SHALL return `"zh"`

##### Example: Precedence resolution table

| Hook arg `override` | Store `locale_override` | `navigator.language`  | Resolved locale |
| ------------------- | ---------------------- | --------------------- | --------------- |
| `"zh"`              | `"en"`                 | `en-US`               | `"zh"`          |
| `undefined`         | `"en"`                 | `zh-TW`               | `"en"`          |
| `undefined`         | `null`                 | `zh-TW`               | `"zh"`          |
| `undefined`         | `null`                 | `en-US`               | `"en"`          |
| `undefined`         | `null`                 | `fr-FR`               | `"en"`          |
| `undefined`         | `null`                 | (navigator undefined) | `"en"`          |

#### Scenario: Legacy config without locale_override round-trips safely

- **GIVEN** `~/.codebus/config.yaml` was written by a version before this change and contains no `app.locale_override` key
- **WHEN** the codebus-app loads the config and the user later opens Settings, makes no language change, and clicks Save
- **THEN** the load SHALL succeed AND the active locale SHALL be derived from `navigator.language` AND the saved config SHALL preserve all other existing keys unchanged

## MODIFIED Requirements

### Requirement: Global Settings Modal Field Set

The Settings modal SHALL be invoked by the bottom-left gear in either Lobby or Workspace state. The modal SHALL display, in addition to the CLI Status row (see "Settings UI CLI Status Field") and the Endpoint Section (see "Settings UI Endpoint Section"), the following editable configuration fields:

1. AI Provider (read-only label: "Claude CLI (only option for now)")
2. PII scanner (dropdown showing scanner name and dynamic pattern count, e.g. `regex_basic · 14 patterns`)
3. PII on-hit policy (dropdown: `warn` / `skip` / `mask`) mapping to `pii.on_hit`
4. PII extra patterns (`pii.patterns_extra`): an editable list of raw regex strings with add and remove controls, no display label per entry
5. Lint fix enabled (toggle) mapping to `lint.fix.enabled`
6. Quiz content verify (toggle) mapping to `quiz.content_verify`
7. Goal content verify (toggle) mapping to `goal.content_verify`
8. Log sink (path display + Change folder link) with an additional control that disables logging entirely by writing `log.sink: none`
9. Quiz pass threshold (slider 50–100%, displayed value with `%` unit suffix)
10. Default quiz length (slider 3–10, displayed value with `questions` unit suffix)
11. Block image / binary reads (toggle) mapping to `hooks.read_image_block`. The toggle SHALL display the current resolved boolean value (default `true` when the config key is absent), and changing it SHALL set `hooks.read_image_block` to the new value on the next Save. The toggle SHALL be accompanied by visible copy stating that disabling it allows the agent to read image / PDF / binary files into its context AND that doing so bypasses the regex_basic PII filter (which only scans text). This copy SHALL be a security-conscious warning, not a neutral description, because the default is `true` (block) and disabling it weakens the PII safety floor.
12. Language (dropdown with exactly three options: "Auto", "中文", "English") mapping to `app.locale_override`. The "Auto" option SHALL write `null`, "中文" SHALL write `"zh"`, and "English" SHALL write `"en"`. The dropdown SHALL be positioned below the Endpoint Section and above the PII scanner field. The two non-Auto option labels ("中文" and "English") SHALL appear identically in both locales because they identify the language they select; only the "Auto" label and the field label itself SHALL be localized.

The Endpoint Section SHALL render a read-only `chat` row that displays the model and effort the `chat` verb inherits from the `query` verb, in the form "沿用 query（<model> / <effort>）", kept in sync with the editable `query` row. The `chat` row SHALL NOT be editable and SHALL NOT introduce any `chat`-specific configuration key.

No theme toggle and no per-vault override section SHALL be present. Sub-labels under fields SHALL NOT promise features absent from v1. The PII on-hit field SHALL display copy stating that Critical-severity matches are always masked regardless of this setting (the security floor cannot be disabled from the UI). The Quiz content verify and Goal content verify toggles SHALL each display copy stating that enabling them incurs additional verify/repair agent spawns.

The `save_global_config` IPC SHALL preserve every known and unknown subkey under any namespace it does not exclusively own. In particular, when enriching the `quiz` namespace with the resolved `default_length`, the IPC SHALL merge into the existing `quiz` object rather than replace it, so sibling keys (e.g. `quiz.content_verify`) set by the Settings UI survive a save→load round-trip. Unknown top-level YAML sections SHALL likewise continue to round-trip unchanged. The `hooks` namespace SHALL likewise round-trip through Save without losing unknown subkeys (forward-compat for future hook toggles). The `app` namespace SHALL likewise preserve unknown sibling subkeys when the Settings UI writes `app.locale_override`.

#### Scenario: Modal opens from Lobby gear

- **WHEN** the user clicks the bottom-left gear in the Lobby
- **THEN** the Settings modal opens centered over a dimmed Lobby background

#### Scenario: PII pattern count is dynamic

- **WHEN** the Settings modal renders the PII scanner field
- **THEN** the displayed pattern count is read at runtime from the active scanner registry (not hard-coded in the UI source)

#### Scenario: PII on-hit field states the Critical security floor

- **WHEN** the Settings modal renders the PII on-hit policy field
- **THEN** the field displays selectable values `warn`, `skip`, `mask` AND visible copy stating that Critical-severity matches are always masked regardless of the selected value

#### Scenario: Content verify toggles state their cost

- **WHEN** the Settings modal renders the Quiz content verify and Goal content verify toggles
- **THEN** each toggle displays copy stating that enabling it incurs additional verify/repair agent spawns

#### Scenario: Invalid extra PII pattern blocks save

- **WHEN** the user enters a string that is not a valid regular expression into the PII extra patterns list
- **THEN** the field shows an inline error AND the Save button is disabled until the invalid pattern is corrected or removed

#### Scenario: Disabling logging writes sink none

- **GIVEN** `~/.codebus/config.yaml` has no `log` section
- **WHEN** the user activates the disable-logging control in the Log sink field and clicks Save
- **THEN** `~/.codebus/config.yaml` contains `log:` with `sink: none` after save

#### Scenario: Chat row is read-only and mirrors query

- **GIVEN** the `query` verb resolves to model `haiku-4-5` and effort `low`
- **WHEN** the user opens the Settings modal Endpoint Section
- **THEN** a non-editable `chat` row displays "沿用 query（haiku-4-5 / low）" AND no `chat` key is written to `~/.codebus/config.yaml` on save

#### Scenario: Save persists atomically

- **WHEN** the user changes any field and clicks Save
- **THEN** the system writes `~/.codebus/config.yaml` atomically (temporary file then rename), closes the modal, and shows a "Saved" toast

##### Example: Quiz pass threshold round-trip

- **GIVEN** `~/.codebus/config.yaml` has `app.quiz.pass_threshold: 80`
- **WHEN** the user opens Settings, changes the threshold slider to 70, and clicks Save
- **THEN** `~/.codebus/config.yaml` contains `app.quiz.pass_threshold: 70` after save, and reopening Settings shows the slider at 70

#### Scenario: quiz sibling subkeys survive save

- **GIVEN** the in-memory config payload has `quiz.default_length: 7` AND `quiz.content_verify: true`
- **WHEN** `save_global_config` writes the payload to disk and a subsequent `load_global_config` reads it back
- **THEN** the reloaded payload still contains `quiz.default_length: 7` AND `quiz.content_verify: true`

#### Scenario: Language dropdown is positioned and labeled correctly

- **WHEN** the user opens the Settings modal
- **THEN** a Language dropdown SHALL be present below the Endpoint Section AND above the PII scanner field, AND the dropdown SHALL offer exactly three options whose displayed strings are "Auto" (or its localized equivalent), "中文", and "English"

#### Scenario: Identifier-style language labels are not translated

- **GIVEN** the active locale is `"en"`
- **WHEN** the Settings modal renders the Language dropdown
- **THEN** the option labels for the two non-Auto values SHALL appear as "中文" and "English" verbatim, identical to how they appear when the active locale is `"zh"`

### Requirement: Forbidden Behaviors in v1

The v1 codebus-app SHALL NOT include any of the following:

- Theme toggle or light-mode support (dark mode is hard-coded)
- Vault-specific settings override UI in the Settings modal
- Multi-AI-provider selection UI
- Quest banner, progress bar, or any "graduated" / "mastered" / "learned" page-level state in the Lobby or Workspace
- Tutorial slideshow UI, embedded checkpoints, or tutorial md generation triggers
- Telemetry, analytics, crash reporting, or auto-update channels
- A "Recent Pages" panel inside any sidebar
- Graph view entry in any sidebar
- Chat-mode Cmd+K with conversation memory (the overlay itself is out of scope for this change; no precursor UI element SHALL be added)
- Direct LLM API calls from the frontend (all agent interaction goes through `codebus-core`)
- Multiple concurrently-active goal runs within a single vault session (per the One Active Goal Run At A Time requirement in `app-workspace`)

A user-facing language override SHALL be permitted in the Settings modal as defined by "Settings Language Override" and "Global Settings Modal Field Set"; it is explicitly NOT a forbidden behavior.

#### Scenario: Settings modal has no theme controls

- **WHEN** the user opens the Settings modal in any state
- **THEN** the rendered modal contains exactly the fields defined in "Global Settings Modal Field Set" (including the Language dropdown) plus the CLI Status row and Endpoint Section defined by their own requirements, AND no theme controls are present

#### Scenario: No telemetry network calls

- **WHEN** the codebus-app launches and runs through any Lobby or Settings flow
- **THEN** no outbound network requests are made by the app shell itself (LLM/agent invocations remain the responsibility of `codebus-core` and are out of scope for this change)
