## ADDED Requirements

### Requirement: Goal Content Verify GUI Wiring

The GUI goal-spawn Tauri IPC command SHALL participate in the optional content verification stage defined by the `verb-library` capability's `Goal Content Verification and Repair` requirement, with behavior parity to the CLI and without adding a new IPC command or a content-review UI element.

The command SHALL resolve `goal.content_verify` from the shared `goal.*` configuration using the same core loader the CLI uses (default `false`; a config load error SHALL fall back to `false` rather than silently enabling extra spawns). It SHALL pass the originating goal text into `run_goal` so the off-goal defect check can run. When `goal.content_verify` is `false`, the GUI goal flow SHALL be unchanged and no content-review status SHALL be produced. When `true`, the GUI-driven `run_goal` SHALL run the same verify→repair stage the CLI does (events stream over the existing goal channel); `auto_commit` and the run outcome SHALL be unaffected by content verification beyond the content-review status.

#### Scenario: GUI resolves config and threads goal text

- **WHEN** the GUI goal-spawn IPC runs with `goal.content_verify` set to `true`
- **THEN** `run_goal` SHALL receive `content_verify = true` and the originating goal text AND the verify→repair stage SHALL run with events on the goal stream channel

#### Scenario: GUI default-off leaves the flow unchanged

- **WHEN** the GUI goal-spawn IPC runs and `goal.content_verify` is absent or `false`
- **THEN** no verify spawn SHALL run AND no content-review status SHALL be produced AND no new IPC command or content-review UI element SHALL be introduced

#### Scenario: GUI config load error is conservative

- **WHEN** the shared goal config cannot be loaded
- **THEN** the GUI SHALL treat `content_verify` as `false` (do not silently enable extra spawns)
