## ADDED Requirements

### Requirement: Vault Init Progress Event

The system SHALL emit a Tauri event named `vault-init-progress` from the Rust side of `codebus-app` while `add_vault` runs an init-heavy branch (`AddVaultMode::Detect` against a folder without `.codebus/` OR `AddVaultMode::ReInit`). The event payload SHALL be a struct serialised with `serde(rename_all = "snake_case")` carrying three fields: `phase` (integer 1..=6), `init_event_kind` (string, the InitEvent variant debug label such as `"Start"` or `"LayoutCreated"`), and `elapsed_ms` (unsigned integer milliseconds since `add_vault_at` started). The `phase` value SHALL be derived from the InitEvent variant by a Tauri-layer function (not by `codebus-core`) using the authoritative mapping below; the frontend SHALL NOT interpret `init_event_kind` for layout decisions.

The system SHALL change `add_vault_at` from a synchronous function into an asynchronous function that accepts a `tauri::AppHandle`; the existing `add_vault` Tauri command and any internal test callers SHALL be updated accordingly. The `codebus-core` `InitEvent` enum, `run_init` signature semantics, and existing variants SHALL NOT be modified by this change.

The InitEvent variant to phase mapping SHALL be:

- Phase 1: `Start`, `LayoutCreated`, `SourceGitignore`.
- Phase 2: `PiiConfigLoadWarn`, `PiiPatternsExtraWarn`, `RawSyncDone`.
- Phase 3: `InternalGitignoreDone`, `NestedRepoDone`.
- Phase 4: `SchemaDone`, `ManifestSignal`, `ManifestDone`, `SkillBundlesDone`, `NavStubsDone`, `SettingsDone`.
- Phase 5: `ObsidianResult`, `ObsidianSkipped`.
- Phase 6: `StarterConfigUnavailable`, `StarterConfigDone`, `StarterConfigError`, `CommitDone`, `Finished`.

The mapping function SHALL use an exhaustive `match` over all `InitEvent` variants (no catch-all arm) so that adding a new variant to `codebus-core::vault::init::InitEvent` produces a compile-time error in `codebus-app` until the mapping is updated.

#### Scenario: Detect-mode add emits one event per InitEvent

- **WHEN** the user invokes the New Vault flow on a folder with no `.codebus/` directory
- **THEN** the Rust side SHALL emit one `vault-init-progress` event per InitEvent that `run_init` produces AND each payload's `phase` field SHALL match the mapping above for the corresponding InitEvent variant AND `elapsed_ms` SHALL be monotonic non-decreasing across events of a single add operation

#### Scenario: Re-init mode emits events from the second run_init call

- **GIVEN** a folder that already contains `.codebus/` and the user picks the Re-initialize destructive option with the typed `delete` confirmation
- **WHEN** the system removes the existing `.codebus/` and calls `run_init` for the fresh init
- **THEN** the `vault-init-progress` event stream SHALL cover the InitEvents emitted by the fresh `run_init` call with the same phase mapping AND no events from the removed directory's history SHALL be emitted

#### Scenario: Just-bind mode emits no progress events

- **WHEN** the user picks the Just-Bind option on a folder that already contains `.codebus/`
- **THEN** the system SHALL NOT call `run_init` AND SHALL NOT emit any `vault-init-progress` event AND `add_vault` SHALL still return the new `VaultEntry`

#### Scenario: Unknown InitEvent variant is a compile-time error

- **GIVEN** a future change adds a new variant `NewlyAdded` to the `InitEvent` enum in `codebus-core::vault::init`
- **WHEN** `codebus-app` is rebuilt without updating the Tauri-layer mapping function
- **THEN** the Rust compiler SHALL emit a `non-exhaustive patterns` error in the mapping function rather than silently routing the new variant to a default phase

##### Example: phase mapping table

| InitEvent variant         | Emitted phase |
| ------------------------- | ------------- |
| `Start`                   | 1             |
| `LayoutCreated`           | 1             |
| `SourceGitignore`         | 1             |
| `PiiConfigLoadWarn`       | 2             |
| `PiiPatternsExtraWarn`    | 2             |
| `RawSyncDone`             | 2             |
| `InternalGitignoreDone`   | 3             |
| `NestedRepoDone`          | 3             |
| `SchemaDone`              | 4             |
| `ManifestSignal`          | 4             |
| `ManifestDone`            | 4             |
| `SkillBundlesDone`        | 4             |
| `NavStubsDone`            | 4             |
| `SettingsDone`            | 4             |
| `ObsidianResult`          | 5             |
| `ObsidianSkipped`         | 5             |
| `StarterConfigUnavailable`| 6             |
| `StarterConfigDone`       | 6             |
| `StarterConfigError`      | 6             |
| `CommitDone`              | 6             |
| `Finished`                | 6             |

---

### Requirement: LoadingOverlay Live Progress

The `LoadingOverlay` component SHALL render while `useVaultsStore.initInProgress` is `true` and SHALL listen for the `vault-init-progress` Tauri event. The overlay SHALL maintain a frontend state machine with a `phase` value (0..6, default 0) and a `failed` boolean (default false). The bus emoji animated by `@keyframes codebus-bus-roll` SHALL remain mounted across all phase transitions; the component SHALL NOT remount the bus element when the phase changes.

When `phase === 0` (no `vault-init-progress` event received yet) the overlay SHALL render the v1 fallback content: the existing `loading.title` and `loading.subtitle` i18n strings together with the bus animation, and SHALL NOT render the phase-dots indicator. When `phase >= 1` the overlay SHALL render the existing `loading.title`, the phase-specific subtitle from `loading.phase.{phase}.title`, and a 6-dot indicator using the shared `PhaseDots` component (extracted from `QuizTab.StepDots`) with `total={6}` and `current={phase}`. The `loading.title` and `loading.subtitle` existing i18n keys SHALL NOT be renamed nor have their values modified by this change.

The state machine SHALL enforce a minimum 300 ms residence time per phase: when an incoming `vault-init-progress` event would advance the phase, the component SHALL delay the visible transition until at least 300 ms have elapsed since the previous phase became visible, queueing the pending phase value in the meantime. Backend events that arrive faster than 300 ms SHALL NOT be dropped — only the visible subtitle / dots update is debounced. When the `add_vault` IPC call resolves successfully (regardless of which phase the last event reported), the overlay SHALL render at phase 6 for at least 300 ms and then fade out over 200 ms before unmounting.

If `add_vault` IPC rejects with an error, the overlay SHALL enter failure mode: the `codebus-bus-roll` animation SHALL be paused, the title SHALL switch to `loading.error.title`, the subtitle SHALL display the rejected `LocalizedError`'s message string, the `PhaseDots` SHALL keep `total={6}` and `current` at the last reached phase with `state="error"` (the current dot SHALL render in `--color-warn` to match the 02c Interrupted banner), and a retry button labeled `loading.error.retry` SHALL appear. The retry button SHALL re-dispatch the same `addVault` call (mode and path unchanged from the failed attempt). The failure styling SHALL use the amber-warm `--color-warn` token and SHALL NOT use a hard-fail red color.

When the visible phase has not advanced for more than 20 000 ms (single-phase stall), the overlay SHALL render the `loading.slow.hint` text in a dim style directly below the phase subtitle. The hint SHALL disappear when the next phase becomes visible.

The component SHALL define and use the following new i18n keys in both `zh` and `en` of `codebus-app/src/i18n/messages.ts`, in addition to the existing `loading.title` and `loading.subtitle`:

- `loading.phase.1.title`, `loading.phase.2.title`, `loading.phase.3.title`, `loading.phase.4.title`, `loading.phase.5.title`, `loading.phase.6.title`
- `loading.error.title`, `loading.error.retry`
- `loading.slow.hint`

#### Scenario: Initial mount shows fallback before any event

- **WHEN** `useVaultsStore.initInProgress` flips to `true` and no `vault-init-progress` event has been received
- **THEN** the overlay renders the existing `loading.title` and `loading.subtitle` strings with the bus animation running AND no element with `data-testid="loading-overlay-phase-dots"` is mounted

#### Scenario: Phase advances on event

- **GIVEN** the overlay is rendering at phase 1 with subtitle `loading.phase.1.title`
- **WHEN** a `vault-init-progress` event with `phase: 2` arrives more than 300 ms after the overlay entered phase 1
- **THEN** the overlay updates the subtitle to `loading.phase.2.title` AND the second dot is marked active AND the bus emoji element is not remounted (same DOM node identity)

#### Scenario: Backend skips phase 5 but UI still pauses

- **GIVEN** the overlay is rendering at phase 4
- **WHEN** the backend emits `ObsidianSkipped` (phase 5) and `CommitDone` (phase 6) within 50 ms of each other
- **THEN** the overlay renders phase 5 subtitle `loading.phase.5.title` for at least 300 ms before transitioning to phase 6 subtitle `loading.phase.6.title`

#### Scenario: Successful finish fades out

- **GIVEN** the overlay is rendering at phase 6
- **WHEN** the `add_vault` IPC resolves with `Ok(VaultEntry)`
- **THEN** the overlay opacity transitions from 1 to 0 over 200 ms AND after the transition completes the overlay is removed from the DOM AND the Workspace for the new vault is now visible

#### Scenario: Backend error enters failure mode

- **GIVEN** the overlay is rendering at phase 3
- **WHEN** the `add_vault` IPC rejects with an `AppError` whose `LocalizedError.message` resolves to "Permission denied writing to .codebus/"
- **THEN** the bus animation pauses AND the title is `loading.error.title` AND the subtitle is "Permission denied writing to .codebus/" AND the third dot renders in the `--color-warn` token AND a retry button with label `loading.error.retry` is visible

#### Scenario: Retry re-dispatches the same add_vault call

- **GIVEN** the overlay is in failure mode after an `AddVaultMode::Detect` failure on path `/Users/alice/repo`
- **WHEN** the user clicks the retry button
- **THEN** the overlay re-invokes `useVaultsStore.addVault("/Users/alice/repo", "detect")` AND the state machine resets to phase 0 and `failed=false` while the new IPC is in flight

#### Scenario: Slow phase shows dim hint

- **GIVEN** the overlay has been rendering at phase 4 for 19 500 ms with no new `vault-init-progress` event
- **WHEN** another 500 ms elapses without an event
- **THEN** the overlay renders a dim hint element with text `loading.slow.hint` directly below the phase subtitle AND the hint disappears when the next `vault-init-progress` event causes the visible phase to advance

#### Scenario: Backend never emits events but IPC succeeds

- **GIVEN** the `vault-init-progress` event listener is never invoked during an `add_vault` call (regression or rollout gap)
- **WHEN** `add_vault` resolves with `Ok(VaultEntry)`
- **THEN** the overlay fades out over 200 ms from its phase-0 fallback render AND no error UI is shown AND the Workspace becomes visible

#### Scenario: Quiz wizard step dots continue to work

- **GIVEN** the `QuizTab` previously rendered four step dots through a local `StepDots` function
- **WHEN** the local function is replaced by the shared `PhaseDots` component with `total={4}` and `current={wizardStep}`
- **THEN** the rendered element continues to expose `data-testid="quiz-wizard-step-dots"` AND the `data-current-step` attribute reflects the active step value as before
