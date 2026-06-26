## MODIFIED Requirements

### Requirement: Global Settings Modal Field Set

The Settings modal SHALL be invoked by the bottom-left gear in either Lobby or Workspace state. The modal SHALL display, in addition to the CLI Status row (see "Settings UI CLI Status Field") and the Endpoint Section (see "Settings UI Endpoint Section"), the following editable configuration fields:

1. AI Provider (read-only label: "Claude CLI (only option for now)")
2. PII scanner (dropdown showing scanner name and dynamic pattern count, e.g. `regex_basic · 13 patterns`)
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
