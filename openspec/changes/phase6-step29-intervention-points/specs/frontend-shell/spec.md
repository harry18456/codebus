## ADDED Requirements

### Requirement: TopBar workspace switcher offers safe relocation through confirm modal

The frontend SHALL render a workspace identifier chip in `<TopBar>` (the layout-level chrome at the top of the App). The chip SHALL display the current workspace name (basename of the workspace path; D-002 forbids the absolute path being rendered as plain text on tutorial chrome to avoid leaking sensitive directory names). Clicking the chip SHALL open a small dropdown menu containing at least one option: "🔁 換資料夾" (switch workspace).

Selecting "🔁 換資料夾" SHALL trigger `useIntervention().requestSwitchWorkspace()` which opens `<InterventionConfirmModal>` carrying copy that explains:

1. Switching does NOT delete the current workspace's progress, KB, or audit logs (these are persisted under the workspace path and survive across switches)
2. Switching DOES require re-authorizing the new workspace path through the existing grant flow (per `authorization-audit` capability), because grants are bound to the workspace path
3. Returning to a previously-authorized workspace will skip the grant modal automatically (the App detects existing grant from `~/.codebus/authorization_audit.jsonl`)

On modal confirm, the frontend MUST `router.push('/')` to navigate back to the entry page; the entry page's existing decision tree (workspace pick → grant detection → station board OR grant flow) handles the rest. The frontend MUST NOT delete or rewrite any workspace-scoped state (`progress.json`, `route.json`, KB collection, audit JSONL files) as part of the switch.

The TopBar workspace chip and switcher dropdown MUST appear on every tutorial-level page (`/tutorial/...`, `/explorer/...`, `/audit/...`) but MUST NOT appear on the entry page (`/`) nor the grant page (`/workspace/grant`) — there is no "current workspace" on those pages and the chip would be misleading.

#### Scenario: Workspace chip renders current workspace basename on tutorial pages

- **WHEN** the user is on `/tutorial/ws_xxx/index` for a workspace whose path is `D:/side_project/some-repo`
- **THEN** `<TopBar>` MUST contain a chip displaying `some-repo` (the basename, not the full path)
- **AND** the chip MUST be visually identifiable as interactive (cursor pointer, hover state, or affordance icon)

#### Scenario: Workspace chip absent on entry and grant pages

- **WHEN** the user is on `/` (entry page) or `/workspace/grant`
- **THEN** the workspace chip MUST NOT render in `<TopBar>`
- **AND** no related dropdown nor switcher menu MUST be present in the DOM

#### Scenario: Switch confirm modal explains grant and progress preservation

- **WHEN** the user clicks the workspace chip and selects "🔁 換資料夾" from the dropdown
- **THEN** `<InterventionConfirmModal>` MUST render with copy that mentions (a) progress is preserved per-workspace, (b) re-authorizing the new workspace is required, and (c) returning to a previously-authorized workspace skips the grant modal
- **AND** the modal MUST offer Cancel and Confirm actions
- **AND** Cancel MUST close the modal without navigation, leaving the user on their current page

#### Scenario: Confirm switch routes to entry page without touching workspace state

- **WHEN** the user confirms the switch modal while on `/tutorial/ws_xxx/s02-mqtt-client`
- **THEN** the App MUST `router.push('/')` to the entry page
- **AND** no on-disk file under `<current_workspace>/codebus-tutorials/...` MUST be modified or deleted as part of the navigation
- **AND** no entry MUST be appended to `~/.codebus/authorization_audit.jsonl` as part of the switch (no `grant_revoked` event is emitted; switching is not an authorization change)

#### Scenario: Returning to previously-authorized workspace skips grant modal

- **WHEN** after switching, the user picks the original workspace path again on the entry page (where `~/.codebus/authorization_audit.jsonl` still contains a valid `grant_issued` row for that path matching the current Sanitizer rules version)
- **THEN** the App MUST navigate directly to the workspace's tutorial / station board without rendering the O-01 grant modal
- **AND** no new `grant_issued` row MUST be appended (the existing grant remains valid)
