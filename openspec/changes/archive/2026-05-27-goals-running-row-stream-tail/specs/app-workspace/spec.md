## ADDED Requirements

### Requirement: Goals List Running Row Stream Tail

The system SHALL render a single-line "stream tail" inside each `RunListItem` row whose `run.outcome` equals `"running"`. The tail SHALL display a one-line summary of the most recent non-thought `VerbEvent` received for that run on the `goal-stream` Tauri channel, derived via the shared `summarizeVerbEvent` helper. The tail SHALL render to the left of the existing relative-timestamp column AND to the right of the truncated goal-text column. The tail element SHALL carry `data-testid="run-row-tail"` AND the CSS class set `font-mono text-meta text-fg-secondary tabular-nums truncate`. When no non-thought event has yet been received for the run (i.e., the `tailByRunId` slot is undefined for that run id), the tail SHALL render the i18n string for key `workspace.goals.runningTailPending`. When `run.outcome` is NOT `"running"`, the row SHALL NOT render the tail element at all (the `data-testid="run-row-tail"` element SHALL be absent from the DOM for non-running rows).

#### Scenario: Running row shows tail derived from latest tool_use event

- **WHEN** the Goals list contains a row with `run.outcome` equal to `"running"` AND `useGoalsStore.tailByRunId[run.run_id]` holds a `VerbEvent` with shape `{ kind: "stream", data: { kind: "tool_use", name: "Read", input: { file_path: "raw/code/auth.rs" } } }`
- **THEN** the row's `data-testid="run-row-tail"` element SHALL be present AND its text content SHALL contain the string `Read` AND the string `auth.rs`

#### Scenario: Running row shows tail derived from Write event

- **WHEN** the Goals list contains a row with `run.outcome` equal to `"running"` AND `tailByRunId[run.run_id]` holds a `VerbEvent` with shape `{ kind: "stream", data: { kind: "tool_use", name: "Write", input: { file_path: "wiki/modules/auth.md" } } }`
- **THEN** the row's tail element text content SHALL contain the string `wiki/modules/auth.md`

#### Scenario: Running row shows tail derived from banner event

- **WHEN** the Goals list contains a row with `run.outcome` equal to `"running"` AND `tailByRunId[run.run_id]` holds a `VerbEvent` with shape `{ kind: "banner", data: { kind: "sync_start" } }`
- **THEN** the row's tail element text content SHALL equal the i18n value for key `workspace.activity.banner.syncStart`

#### Scenario: Running row shows placeholder when tail slot is empty

- **WHEN** the Goals list contains a row with `run.outcome` equal to `"running"` AND `useGoalsStore.tailByRunId[run.run_id]` is undefined
- **THEN** the row's `data-testid="run-row-tail"` element SHALL be present AND its text content SHALL equal the i18n value for key `workspace.goals.runningTailPending`

#### Scenario: Non-running row omits tail element entirely

- **WHEN** the Goals list contains a row with `run.outcome` equal to `"succeeded"` AND `useGoalsStore.tailByRunId[run.run_id]` holds a `VerbEvent` value (tail slot retained after the run terminated)
- **THEN** the row SHALL NOT contain a `data-testid="run-row-tail"` element

### Requirement: useGoalsStore Tracks Latest Stream Event Per Run

The system SHALL maintain `useGoalsStore.tailByRunId` typed as `Record<string, VerbEvent>` recording the most recently received non-thought `VerbEvent` per `run_id`. The store's existing `_onStreamEvent` handler SHALL, in addition to its existing `activeRun.events` append behavior, write the incoming `VerbEvent` to `tailByRunId[payload.run_id]` whenever the event is NOT a thought event (i.e., NOT `{ kind: "stream", data: { kind: "thought" } }`). The store SHALL accept stream events for any `run_id`, including run ids not present in `activeRun` (terminal-spawned goals whose `_onStreamEvent` arrives without a matching `activeRun.runId`). The store's `_onTerminal` handler SHALL NOT clear `tailByRunId[payload.run_id]` — the entry SHALL remain in the map after the run terminates. The store's `reset()` method SHALL clear `tailByRunId` to an empty object alongside its existing `activeRun` and `runs` clearing.

#### Scenario: Stream event for active run writes to tailByRunId

- **GIVEN** `tailByRunId` is initially empty AND `activeRun.runId` equals `"run-A"`
- **WHEN** `_onStreamEvent` receives payload with `run_id` equal to `"run-A"` AND event `{ kind: "stream", data: { kind: "tool_use", name: "Read", input: { file_path: "x" } } }`
- **THEN** after the call `tailByRunId["run-A"]` SHALL equal that `VerbEvent` AND `activeRun.events` SHALL also contain the event (unchanged from prior behavior)

#### Scenario: Stream event for terminal-spawned goal writes to tailByRunId even when activeRun is null

- **GIVEN** `activeRun` is `null` AND `tailByRunId` is empty
- **WHEN** `_onStreamEvent` receives payload with `run_id` equal to `"run-B"` AND event `{ kind: "banner", data: { kind: "start", repo_path: "/v" } }`
- **THEN** `tailByRunId["run-B"]` SHALL equal that `VerbEvent` AND `activeRun` SHALL remain `null`

#### Scenario: Thought event does not write to tailByRunId

- **GIVEN** `tailByRunId["run-A"]` holds a prior tool_use event `e_prev`
- **WHEN** `_onStreamEvent` receives payload with `run_id` equal to `"run-A"` AND event `{ kind: "stream", data: { kind: "thought", text: "..." } }`
- **THEN** `tailByRunId["run-A"]` SHALL remain `e_prev` (unchanged)

#### Scenario: Terminal event preserves tail slot

- **GIVEN** `tailByRunId["run-A"]` holds a `VerbEvent` `e` AND `activeRun.runId` equals `"run-A"`
- **WHEN** `_onTerminal` receives payload with `run_id` equal to `"run-A"`
- **THEN** `activeRun` SHALL be set to `null` (unchanged from prior behavior) AND `tailByRunId["run-A"]` SHALL still equal `e` (NOT cleared)

#### Scenario: reset clears tail map

- **GIVEN** `tailByRunId` contains entries for `"run-A"` AND `"run-B"`
- **WHEN** `reset()` is called
- **THEN** `tailByRunId` SHALL equal the empty object AND `activeRun` SHALL be `null` AND `runs` SHALL be the empty array

### Requirement: useLatestStreamEvent Hook Provides Per-Run Tail Access

The system SHALL expose a React hook `useLatestStreamEvent(runId: string): VerbEvent | null` in `codebus-app/src/hooks/useLatestStreamEvent.ts`. The hook SHALL subscribe to `useGoalsStore` AND return `tailByRunId[runId]` if present, else `null`. The hook SHALL use a Zustand selector that depends only on the slot for the supplied `runId` (NOT the entire `tailByRunId` record), so that stream events for OTHER run ids do NOT trigger a re-render of components consuming this hook for an unrelated `runId`.

#### Scenario: Hook returns tail value for known run id

- **GIVEN** `useGoalsStore.tailByRunId["run-A"]` holds a `VerbEvent` value `e`
- **WHEN** a component calls `useLatestStreamEvent("run-A")`
- **THEN** the hook SHALL return `e`

#### Scenario: Hook returns null for unknown run id

- **GIVEN** `useGoalsStore.tailByRunId` does NOT contain key `"run-Z"`
- **WHEN** a component calls `useLatestStreamEvent("run-Z")`
- **THEN** the hook SHALL return `null`

#### Scenario: Unrelated stream event does not re-render hook consumer

- **GIVEN** a component mounted with `useLatestStreamEvent("run-A")` AND `tailByRunId["run-A"]` holding a stable value
- **WHEN** `_onStreamEvent` writes a NEW event to `tailByRunId["run-B"]` AND `tailByRunId["run-A"]` is unchanged
- **THEN** the component's render count SHALL NOT increase

### Requirement: Stream Event Summary Helper Module

The system SHALL expose a shared helper module at `codebus-app/src/lib/streamEventSummary.ts` exporting the following pure functions: `summarizeVerbEvent(event: VerbEvent, t: TFunction): string | null`, `bannerLabel(banner: VerbBanner, t: TFunction): string`, `summarizeToolInput(input: unknown): string`, `writeEditPath(input: unknown): string`, AND `extractInnerCommand(raw: string): string`. The `summarizeVerbEvent` facade SHALL return a single-line string for `VerbEvent` whose `kind` is `"banner"` AND for events whose `kind` is `"stream"` AND inner `data.kind` is `"tool_use"`; it SHALL return `null` for events whose `kind` is `"stream"` AND inner `data.kind` is `"thought"`, AND for any other event shape with no one-line summary. The `ActivityStreamItem` component SHALL consume these helpers from the new module rather than from local file-scope definitions; its rendered output SHALL be functionally equivalent to its pre-extraction behavior, verified by the existing `ActivityStreamItem.test.tsx` suite passing without modification.

#### Scenario: summarizeVerbEvent renders Write event

- **WHEN** `summarizeVerbEvent` is called with event `{ kind: "stream", data: { kind: "tool_use", name: "Write", input: { file_path: "wiki/x.md" } } }`
- **THEN** the result SHALL be a non-null string containing the substring `wiki/x.md`

#### Scenario: summarizeVerbEvent renders generic tool_use event

- **WHEN** `summarizeVerbEvent` is called with event `{ kind: "stream", data: { kind: "tool_use", name: "Read", input: { file_path: "raw/code/auth.rs" } } }`
- **THEN** the result SHALL be a non-null string containing the substring `Read` AND the substring `auth.rs`

#### Scenario: summarizeVerbEvent returns null for thought event

- **WHEN** `summarizeVerbEvent` is called with event `{ kind: "stream", data: { kind: "thought", text: "..." } }`
- **THEN** the result SHALL be `null`

#### Scenario: summarizeToolInput truncates long shell command to 80 chars

- **GIVEN** an 800-character shell command string `cmd`
- **WHEN** `summarizeToolInput` is called with `{ command: cmd }`
- **THEN** the result SHALL have length 80 AND SHALL end with the character `…` (U+2026)

##### Example: shell command extraction and truncation

| Input.command                              | Expected output | Notes                          |
| ------------------------------------------ | --------------- | ------------------------------ |
| `bash -c "echo hi"`                        | `echo hi`       | sh -c wrapper stripped         |
| `powershell.exe -NoProfile -Command "ls"`  | `ls`            | PowerShell wrapper stripped    |
| 800-char raw command                       | 79 chars + `…`  | length 80 with trailing ellipsis |
| `git status`                               | `git status`    | no wrapper, passthrough        |

### Requirement: i18n Key for Running Tail Pending Placeholder

The system SHALL define a new i18n key `workspace.goals.runningTailPending` in BOTH the `messages.en` AND `messages.zh` bundles in `codebus-app/src/i18n/messages.ts`. The value SHALL be the single Unicode horizontal ellipsis character `…` (U+2026) in BOTH bundles. The key SHALL be enumerated in the workspace i18n bundle test (`codebus-app/src/i18n/workspace.test.ts`) so the existing bundle-parity check catches any missing translation.

#### Scenario: Both bundles define the key with the ellipsis value

- **WHEN** the test inspects `messages.en["workspace.goals.runningTailPending"]` AND `messages.zh["workspace.goals.runningTailPending"]`
- **THEN** both values SHALL equal the string `"…"` (single character, U+2026)
