## MODIFIED Requirements

### Requirement: Run Detail Views — Cancelled and Interrupted

The system SHALL render a single `RunDetailInterrupted` component for any terminal run whose `RunLog` outcome is `"cancelled"`, `"failed"`, or `"interrupted"`. The component file SHALL be `codebus-app/src/components/workspace/RunDetailInterrupted.tsx` (renamed from `RunDetailCancelled.tsx`); the previous `RunDetailCancelled` component SHALL NOT exist as a separate export.

The component SHALL implement an explicit two-stage state machine driven by two inputs:

1. **Banner tier** (color and visual language), derived from `RunLog.outcome`:
   - `outcome === "failed"` → banner tier `"red"` (Failed visual language).
   - `outcome === "cancelled"` OR `outcome === "interrupted"` → banner tier `"amber"` (Interrupted visual language).
2. **Reason sub-variant** (banner subtitle copy), derived from `RunLog.interrupt_reason`:
   - When `interrupt_reason === "app-close"` → subtitle key `workspace.runDetail.banner.reason.appClose`.
   - When `interrupt_reason === "user-cancel"` → subtitle key `workspace.runDetail.banner.reason.userCancel`.
   - When `interrupt_reason === "network-drop"` → subtitle key `workspace.runDetail.banner.reason.networkDrop`.
   - When `interrupt_reason` is the `{ other: string }` variant → subtitle key `workspace.runDetail.banner.reason.other`. The free-form `other` string SHALL NOT be rendered into the UI text.
   - When `interrupt_reason` is `undefined` (legacy RunLog) AND `outcome === "cancelled" | "interrupted"` → subtitle key `workspace.runDetail.banner.interruptedSubtitle` (generic amber fallback).
   - When banner tier is `"red"` (`outcome === "failed"`) → title key `workspace.runDetail.banner.failedTitle` AND subtitle key `workspace.runDetail.banner.failedSubtitle` (the failed branch ignores `interrupt_reason`).

The component SHALL include: a header with `← back`, the goal text, and a status badge whose icon and tier follow the banner tier above; a banner block with title + subtitle keyed per the state machine above; a `Partial timeline` section summarizing tool_use events grouped by category (reading / writing / other); and a `[Retry with same goal]` button.

The `[Retry with same goal]` button SHALL extract the goal text from the run's RunLog row (when present) or the events.jsonl first user-prompt event (for virtual interrupted entries with no RunLog row), pre-fill the New Goal modal with that text, and open the modal. The user SHALL still confirm the run by clicking `Run` in the modal — Retry SHALL NOT spawn a new goal directly.

Routing in `codebus-app/src/components/workspace/Workspace.tsx` SHALL dispatch every non-running terminal outcome that is not `"succeeded"` to `RunDetailInterrupted`. The previous outcome switch case for `RunDetailCancelled` SHALL be removed.

The i18n keys `workspace.runDetail.cancelledBadge`, `workspace.runDetail.cancelledWarning`, `workspace.runDetail.interruptedBadge`, `workspace.runDetail.interruptedWarning`, and `workspace.runDetail.retryButton` SHALL remain registered with their existing key names; the system SHALL NOT rename them. New banner keys SHALL be added without removing the legacy keys.

#### Scenario: Cancelled run renders amber banner with userCancel subtitle

- **WHEN** the user navigates to a run with `RunLog.outcome === "cancelled"` AND `RunLog.interrupt_reason === "user-cancel"`
- **THEN** the detail view renders `RunDetailInterrupted` AND the banner uses the amber tier AND the banner subtitle is sourced from the i18n key `workspace.runDetail.banner.reason.userCancel`

#### Scenario: Failed run renders red banner

- **WHEN** the user navigates to a run with `RunLog.outcome === "failed"`
- **THEN** the detail view renders `RunDetailInterrupted` AND the banner uses the red tier AND the banner title is sourced from `workspace.runDetail.banner.failedTitle` AND the banner subtitle is sourced from `workspace.runDetail.banner.failedSubtitle` AND the banner SHALL NOT use any `reason.*` subtitle key

#### Scenario: Interrupted virtual entry with app-close reason renders appClose subtitle

- **WHEN** the vault contains `events-2026-05-13T03-00-00Z.jsonl` AND no corresponding RunLog row exists for `started_at === "2026-05-13T03:00:00Z"` AND the synthesized virtual entry carries `interrupt_reason === "app-close"`
- **THEN** the Goals overview list contains a virtual entry with `⚠` icon AND clicking it navigates to the `RunDetailInterrupted` view AND the banner uses the amber tier AND the banner subtitle is sourced from `workspace.runDetail.banner.reason.appClose`

#### Scenario: Legacy cancelled run without interrupt_reason falls back to generic amber subtitle

- **WHEN** the user navigates to a legacy run with `RunLog.outcome === "cancelled"` AND `RunLog.interrupt_reason === undefined`
- **THEN** the detail view renders `RunDetailInterrupted` AND the banner uses the amber tier AND the banner subtitle is sourced from `workspace.runDetail.banner.interruptedSubtitle` AND no JavaScript error is thrown

#### Scenario: Unknown interrupt_reason maps to generic reason subtitle

- **WHEN** the user navigates to a run whose `RunLog.interrupt_reason` deserializes to the `{ other: "agent-crash" }` variant
- **THEN** the detail view renders `RunDetailInterrupted` AND the banner subtitle is sourced from `workspace.runDetail.banner.reason.other` AND the raw `"agent-crash"` string SHALL NOT be rendered into the UI text

#### Scenario: Retry pre-fills modal without spawning across all three terminal outcomes

- **WHEN** the user clicks `[Retry with same goal]` in `RunDetailInterrupted` for a run with `goal === "describe auth flow"` AND the run's outcome is any of `"cancelled" | "failed" | "interrupted"`
- **THEN** the New Goal modal opens AND the textarea contains exactly the text `"describe auth flow"` AND no `spawn_goal` IPC invocation occurs until the user clicks `Run` in the modal

##### Example: state machine inputs to banner outputs

| outcome     | interrupt_reason            | banner tier | title key              | subtitle key                                  |
| ----------- | --------------------------- | ----------- | ---------------------- | --------------------------------------------- |
| cancelled   | undefined                   | amber       | interruptedTitle       | interruptedSubtitle                           |
| cancelled   | "user-cancel"               | amber       | interruptedTitle       | reason.userCancel                             |
| failed      | undefined                   | red         | failedTitle            | failedSubtitle                                |
| failed      | "user-cancel"               | red         | failedTitle            | failedSubtitle (reason ignored on red tier)   |
| interrupted | "app-close"                 | amber       | interruptedTitle       | reason.appClose                               |
| interrupted | "network-drop"              | amber       | interruptedTitle       | reason.networkDrop                            |
| interrupted | { other: "agent-crash" }    | amber       | interruptedTitle       | reason.other                                  |
| interrupted | undefined                   | amber       | interruptedTitle       | interruptedSubtitle                           |
