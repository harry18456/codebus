## ADDED Requirements

### Requirement: Wiki Tab Subscribes To Watcher Events

The Wiki tab SHALL subscribe to the `wiki-list-changed` and `wiki-page-changed` Tauri events defined by the `fs-watcher` capability. On `wiki-list-changed` the Wiki tab SHALL invoke `useWikiStore.listPages()` to refresh the tree. On `wiki-page-changed` the WikiPreview component SHALL compare the event payload `path` against its currently rendered page; if they match, the preview SHALL re-fetch and re-render that page's content. If they do not match, the preview SHALL ignore the event.

#### Scenario: External edit refreshes the wiki tree

- **GIVEN** the Wiki tab is mounted AND a vault watcher is active
- **WHEN** an external editor saves a new file `<V>/.codebus/wiki/concepts/new.md`
- **THEN** the Wiki tree SHALL show `new.md` within 400 ms without a manual tab switch

#### Scenario: External edit of the open page refreshes the preview

- **GIVEN** WikiPreview is rendering `<V>/.codebus/wiki/concepts/foo.md`
- **WHEN** an external editor modifies that same file
- **THEN** WikiPreview SHALL re-fetch and re-render the file's new content within 400 ms

#### Scenario: External edit of a non-open page does not refresh the preview

- **GIVEN** WikiPreview is rendering `<V>/.codebus/wiki/concepts/foo.md`
- **WHEN** an external editor modifies `<V>/.codebus/wiki/concepts/other.md`
- **THEN** WikiPreview SHALL NOT re-fetch foo.md AND its rendered content SHALL remain unchanged

### Requirement: Goals Tab Subscribes To Watcher Events

The Goals tab SHALL subscribe to the `goals-changed` and `goal-run-changed` Tauri events. On `goals-changed` the tab SHALL invoke `useGoalsStore.refreshRuns()`. On `goal-run-changed` any currently mounted RunDetailRunning or RunDetailDone component SHALL compare the event payload `run_id` against its currently displayed run; if they match, the component SHALL re-fetch the run's events and RunLog summary.

#### Scenario: Terminal-spawned goal becomes visible in Goals list

- **GIVEN** the Goals tab is mounted AND no GUI goal run is in flight
- **WHEN** a terminal session writes a new `events-*.jsonl` and `runs-*.jsonl` for a goal run
- **THEN** the Goals list SHALL include the new run within 400 ms

#### Scenario: Live append to currently viewed run is reflected

- **GIVEN** RunDetailRunning is displaying run `R` that was spawned externally
- **WHEN** the corresponding `events-<R>.jsonl` receives appended lines
- **THEN** RunDetailRunning SHALL re-fetch the events and render the new lines within 400 ms

#### Scenario: Append to a different run does not refetch the open run

- **GIVEN** RunDetailDone is displaying run `R1`
- **WHEN** the `events-<R2>.jsonl` file for a different run is appended
- **THEN** RunDetailDone SHALL NOT re-fetch `R1`'s events

### Requirement: Quiz Tab Subscribes To Watcher Events

The Quiz tab SHALL subscribe to the `quiz-changed` and `quiz-attempt-changed` Tauri events. On `quiz-changed` the tab SHALL rescan `<vault>/.codebus/quiz/` and update its history view. On `quiz-attempt-changed` any currently mounted QuizAnswering or QuizReview component SHALL compare the event payload `{ slug, id }` against its currently displayed attempt; if they match, the component SHALL re-fetch the attempt's markdown and progress sidecar.

#### Scenario: Terminal-spawned quiz becomes visible in history

- **GIVEN** the Quiz tab is mounted
- **WHEN** a terminal session writes a new `<V>/.codebus/quiz/<slug>/<id>.md`
- **THEN** the Quiz history view SHALL include the new attempt within 400 ms

#### Scenario: External progress edit refreshes open attempt

- **GIVEN** QuizAnswering is displaying attempt `(slug=jwt-basics, id=2026-05-20T08-30-00Z)`
- **WHEN** an external process modifies that attempt's `.progress.json` sidecar
- **THEN** QuizAnswering SHALL re-fetch the sidecar and update its rendered progress within 400 ms

#### Scenario: Edit of a different attempt does not refetch

- **GIVEN** QuizReview is displaying attempt `A1`
- **WHEN** a different attempt `A2` is modified externally
- **THEN** QuizReview SHALL NOT re-fetch `A1`'s files

### Requirement: Watcher Error Surfaces Auto-Refresh-Disabled State

The Workspace SHALL subscribe to the `vault-watcher-error` Tauri event and SHALL display a persistent inline indicator on every affected tab (Wiki, Goals, Quiz) when the event fires for the open vault. The indicator SHALL state that auto-refresh is disabled and SHALL include the failure reason. The indicator SHALL remain visible for the rest of the Workspace session for that vault; the frontend SHALL NOT attempt to restart the watcher automatically.

#### Scenario: Auto-refresh-disabled indicator appears on all tabs after watcher failure

- **GIVEN** the Workspace is mounted for vault V
- **WHEN** the backend emits `vault-watcher-error { vault_path: V, reason: "..." }`
- **THEN** each of the Wiki, Goals, and Quiz tabs SHALL render an indicator stating "auto-refresh disabled" together with the failure reason

#### Scenario: No automatic retry

- **GIVEN** the auto-refresh-disabled indicator is showing for vault V
- **WHEN** any time passes while V's Workspace remains mounted
- **THEN** the frontend SHALL NOT invoke `start_vault_watcher(V)` again until the user manually leaves and re-enters the Workspace
