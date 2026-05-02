# frontend-shell Specification

## Purpose

TBD - created by archiving change 'phase6-shell'. Update Purpose after archive.

## Requirements

### Requirement: Design tokens originate from a single source

The `web/` package SHALL declare design tokens (surfaces, text colors, accent colors, typography stacks) at exactly one canonical location: `web/tailwind.config.ts`. The token values MUST mirror `design/v1/tokens.css` (the design source of truth) and MUST NOT be redeclared inside Vue single-file-component `<style>` blocks, inline `style=""` attributes, or `app.vue`-level CSS.

Components MUST consume tokens via Tailwind utility classes (e.g., `bg-surface-0`, `text-text-base`, `bg-accent`); raw hex values (e.g., `#0b0d10`) and Tailwind built-in palette names that override the token system (e.g., `bg-slate-950`, `bg-indigo-500`) MUST NOT appear in any file under `web/app/`. The seven oklch accent semantics are non-negotiable: `accent` is reserved for agent/primary, `accent-2` for brand secondary, `green` for judge/success, `yellow` for coverage/warning, `orange` for neutral highlight, `red` for kill/error, `purple` for sanitizer/privacy.

#### Scenario: Tailwind config exposes the seven accent tokens

- **WHEN** `web/tailwind.config.ts` is loaded and its `theme.extend.colors` object is inspected
- **THEN** the keys MUST contain `accent`, `accent-2`, `green`, `yellow`, `orange`, `red`, and `purple`
- **AND** each value MUST be the same `oklch(...)` literal as the corresponding `--accent` family variable in `design/v1/tokens.css`

#### Scenario: No Tailwind built-in palette overrides design tokens

- **WHEN** any file under `web/app/` is grepped for the patterns `bg-slate-`, `bg-indigo-`, `bg-zinc-`, `text-slate-`, `text-indigo-`, `text-zinc-`
- **THEN** zero matches MUST be returned
- **AND** any color need MUST be satisfied by token-based utility classes (`bg-surface-{0..4}`, `text-text-{base,dim,mute}`, `bg-{accent,accent-2,green,yellow,orange,red,purple}`)

#### Scenario: Purple stays sanitizer-exclusive

- **WHEN** any Vue file or component template uses the `purple` token (background, text, or border)
- **THEN** the surrounding context MUST relate to sanitizer or privacy semantics (e.g., a sanitize audit row, a redaction badge, a privacy disclaimer)
- **AND** the `purple` token MUST NOT be applied to non-sanitizer contexts such as buttons, links, or generic accents


<!-- @trace
source: phase6-shell
updated: 2026-04-27
code:
  - web/tailwind.config.ts
  - CLAUDE.md
  - tauri/src-tauri/src/lib.rs
  - docs/implementation-plan.md
  - web/app/pages/index.vue
  - web/app/app.vue
  - web/app/composables/useSidecar.ts
  - web/app/components/AppShell.vue
  - web/app/components/audit/AuditPanel.vue
  - tauri/src-tauri/src/sidecar.rs
  - web/app/components/layout/TopBar.vue
  - web/app/layouts/default.vue
  - web/nuxt.config.ts
  - web/app/composables/useSseTask.ts
-->

---
### Requirement: Sidecar bearer and base URL come from Tauri IPC

The `useSidecar` composable SHALL be the single entry point that exposes the sidecar bearer token and base URL to all `web/` code. The composable MUST obtain these values via `@tauri-apps/api/core::invoke` (or equivalent Tauri IPC primitive) at runtime; bearer tokens, ports, or base URLs MUST NOT appear as hardcoded literals anywhere under `web/app/` or `web/composables/`.

The composable MUST expose a `fetch` wrapper that automatically injects `Authorization: Bearer <token>` on every outbound request to the sidecar. Direct calls to the global `fetch()` API targeting the sidecar host MUST go through `useSidecar().fetch` rather than constructing the `Authorization` header manually at each call site.

This rule extends CLAUDE.md invariant #5 (`Bearer + loopback 不可鬆綁`) to the frontend layer: the bearer MUST stay in memory and MUST NOT be persisted to localStorage, sessionStorage, IndexedDB, or any HTTP cache.

The composable's return shape SHALL additionally expose four typed authorization wrappers (`grant`, `deny`, `revoke`, `status`) that route to the corresponding `/auth/*` sidecar endpoints. The wrappers MUST share the same bearer + baseUrl resolved through Tauri IPC and MUST NOT introduce a parallel auth-only handshake mechanism. The wrappers' TypeScript signatures SHALL match the sidecar's Pydantic request/response schemas exactly:

```typescript
interface SidecarApi {
  bearer: Ref<string>
  baseUrl: Ref<string>
  ready: Ref<boolean>
  fetch: typeof fetch
  grant: (req: GrantRequest) => Promise<GrantResponse>
  deny: (req: DenyRequest) => Promise<void>
  revoke: (req: RevokeRequest) => Promise<void>
  status: (workspaceId: string) => Promise<AuthStatusResponse>
}
```

Auth IPC consumers (notably `AuthorizationModal.vue` and any future Settings page revoke entry) MUST call these typed wrappers rather than constructing raw `useSidecar().fetch('/auth/grant')` calls. Calling the auth endpoints by raw fetch is forbidden.

#### Scenario: No bearer literal under web/app/

- **WHEN** the entire `web/app/` tree is grepped for bearer-shaped literals matching `Bearer\s+[A-Za-z0-9_\-]{16,}`, hex strings of 32+ characters, or `localhost:\d{4,5}` URLs
- **THEN** zero matches MUST be returned
- **AND** the only legitimate bearer/port consumer MUST be `useSidecar.ts` reading from Tauri IPC

#### Scenario: useSidecar.fetch injects Authorization header

- **WHEN** any component invokes `useSidecar().fetch('/healthz')`
- **THEN** the resulting HTTP request MUST contain `Authorization: Bearer <token>` where `<token>` is the value just retrieved from Tauri IPC
- **AND** the request URL MUST be resolved relative to the base URL exposed by the same composable

#### Scenario: Bearer never persisted to web storage

- **WHEN** `useSidecar` initializes and obtains a bearer from Tauri
- **THEN** the bearer value MUST NOT be written to `localStorage`, `sessionStorage`, `IndexedDB`, or `document.cookie`
- **AND** browser dev-tools inspection of these storage areas MUST yield no bearer-shaped string

#### Scenario: useSidecar exposes typed auth wrappers

- **WHEN** `useSidecar()` is called from any Vue component
- **THEN** the returned object MUST contain function-typed members named exactly `grant`, `deny`, `revoke`, and `status`
- **AND** each function MUST internally call `this.fetch(...)` against the corresponding `/auth/<verb>` path (relative URL) so the Authorization header is injected by the existing fetch wrapper
- **AND** the four functions MUST NOT make any `fetch` call against any path other than `/auth/*`

#### Scenario: Auth endpoints called only through typed wrappers

- **WHEN** `web/app/` is grepped for the literal pattern `useSidecar\(\)\.fetch\(\s*['"`]/auth/` (raw fetch against /auth/ paths)
- **THEN** zero matches MUST be returned
- **AND** the only legitimate caller path MUST be `useSidecar().grant(...)` / `.deny(...)` / `.revoke(...)` / `.status(...)`


<!-- @trace
source: auth-flow
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/auth.py
  - sidecar/src/codebus_agent/auth/service.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/main.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/auth/errors.py
  - web/app/pages/workspace/grant.vue
  - sidecar/src/codebus_agent/auth/audit_logger.py
  - sidecar/src/codebus_agent/api/__init__.py
  - web/app/composables/useAuthorization.ts
  - web/app/composables/useSidecar.ts
  - docs/authorization.md
  - sidecar/src/codebus_agent/auth/paths.py
  - sidecar/src/codebus_agent/api/auth.py
  - sidecar/src/codebus_agent/auth/__init__.py
  - docs/sidecar-api.md
  - docs/implementation-plan.md
  - web/app/components/auth/AuthorizationModal.vue
tests:
  - sidecar/tests/auth/test_error_codes_disjoint.py
  - sidecar/tests/auth/__init__.py
  - sidecar/tests/auth/test_audit_logger.py
  - sidecar/tests/auth/test_paths.py
  - sidecar/tests/auth/test_service.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/api/test_auth_endpoints.py
-->

---
### Requirement: AuditPanel surfaces seven workspace-level audit JSONL tabs

The `AuditPanel.vue` component SHALL render exactly seven tabs in the order `sanitize`, `tool`, `reasoning`, `token`, `llm`, `kb_growth`, `generator`, mirroring the seven workspace-level audit JSONL files under `<workspace>/.codebus/` declared by CLAUDE.md (`七層 Audit JSONL` section). The component MUST expose an `activeTab` prop accepting any of these seven keys; passing an unrecognised key MUST be a TypeScript compile-time error.

The component MUST NOT render rows from in-source sample data. The `CB_AUDIT_SAMPLES` literal from `design/v1/shell.js` is mockup-only fixture data per `design/v1/README.md §四`; the production component MUST receive its rows via a `rows` prop (or equivalent injection) and MUST render an empty state when the array is empty. No `web/app/` source file may contain a literal copy of `CB_AUDIT_SAMPLES` or any element of it.

The component SHALL emit `select-row` with the clicked row's index in the current `rows` prop when the user clicks a row in the body. The emit MUST fire for every tab uniformly — even tabs with no overlay wiring at the parent level (the parent decides whether to react to the emit). The emit signature MUST equal `(e: 'select-row', index: number) => void`. The component MUST NOT internally render any inspector / drawer / modal in response to the click; row-click → overlay binding is a parent-layer concern. (Rationale: keeps AuditPanel a dumb display surface so per-kind inspectors — `LlmCallInspector` for `llm`, `SanitizerAuditInspector` for `sanitize`, etc. — can be hosted at page level rather than coupled into AuditPanel.)

The `sanitize` tab body MUST render each row with a placeholder identifier chip showing `<REDACTED:{kind}#{placeholder_index}>` derived from the row's `kind` and `placeholder_index` fields. The chip MUST use the `purple` token family (`bg-purple/12`, `text-purple`, `border-purple/40` or equivalent token-based utilities) — sanitizer is the exclusive owner of `purple` per the existing `Purple stays sanitizer-exclusive` Scenario in `Requirement: Design tokens originate from a single source`. The chip MUST NOT use `red`, `orange`, `yellow`, or any other token color regardless of the row's `kind` value, because those tokens are reserved for other audit-row semantics (kill / coverage / warning).

The `sanitize` tab MUST display a `pass` chip on each row showing one of the literal strings `Pass 1` / `Pass 2` / `Pass 3` (the integer `1` / `2` / `3` from the row's `pass` field mapped to human-readable labels via the same lookup the inspector uses). The chip MUST NOT show numeric `1`/`2`/`3` alone, because numeric pass values are not self-describing to a non-engineering audit reviewer.

#### Scenario: All seven tabs render in canonical order

- **WHEN** `<AuditPanel />` mounts with default props
- **THEN** the rendered tab strip MUST contain exactly seven button elements
- **AND** their `data-tab` attribute values MUST equal `sanitize`, `tool`, `reasoning`, `token`, `llm`, `kb_growth`, `generator` in that left-to-right order

#### Scenario: Empty rows show empty state, never sample data

- **WHEN** `<AuditPanel :active-tab="'sanitize'" :rows="[]" />` is rendered
- **THEN** the body region MUST display a documented empty-state message
- **AND** the rendered DOM MUST NOT contain text matching `secret`, `pii_id`, `src/config.py`, `tests/fixtures/.env.test`, or any other string from `design/v1/shell.js::CB_AUDIT_SAMPLES.sanitize[*]`

#### Scenario: No CB_AUDIT_SAMPLES literal under web/app/

- **WHEN** the entire `web/app/` tree is grepped for the symbol `CB_AUDIT_SAMPLES`
- **THEN** zero matches MUST be returned
- **AND** any sample-style data needed for testing MUST live in `web/tests/` (when the test framework lands in Phase B), not in production source

#### Scenario: Row click emits select-row with the clicked index

- **WHEN** the user clicks the third row inside an `<AuditPanel :active-tab="'llm'" :rows="threeEntries" />`
- **THEN** the component MUST emit `select-row` exactly once
- **AND** the emit's payload MUST equal `2` (zero-based index of the clicked row)

#### Scenario: select-row fires for every tab regardless of parent wiring

- **WHEN** the user clicks a row in `<AuditPanel :active-tab="'tool'" :rows="someEntries" />` while the parent does not bind the emit
- **THEN** the component MUST still emit `select-row` (no conditional suppression based on tab)
- **AND** the emit MUST NOT throw when the parent has no listener

#### Scenario: AuditPanel does not render any inspector / drawer / modal of its own

- **WHEN** `<AuditPanel :active-tab="'llm'" :rows="entries" />` is mounted in isolation (no parent overlay wiring)
- **THEN** clicking a row MUST NOT cause any new DOM element with class containing `inspector`, `drawer`, `modal`, or `overlay` to mount inside the AuditPanel root
- **AND** any inspector overlay MUST be hosted by the parent page, not by AuditPanel itself

#### Scenario: Sanitize tab placeholder chip uses purple token exclusively

- **WHEN** `<AuditPanel :active-tab="'sanitize'" :rows="[{rule_id:'aws_access_key',kind:'secret',placeholder_index:1,pass:1,...}]" />` is rendered
- **THEN** the row MUST contain a chip whose visible text equals `<REDACTED:secret#1>`
- **AND** the chip's class list MUST include a `purple`-family token utility (e.g., `bg-purple/12`, `text-purple`, `border-purple/40`)
- **AND** the chip's class list MUST NOT include any of `bg-red`, `bg-orange`, `bg-yellow`, `bg-green`, `bg-accent`, `bg-accent-2`, or their `text-` / `border-` variants

#### Scenario: Sanitize tab pass chip shows human-readable label, not numeric

- **WHEN** a `sanitize` row with `pass: 2` is rendered in the AuditPanel body
- **THEN** the rendered DOM MUST contain a chip with visible text exactly `Pass 2`
- **AND** the chip MUST NOT have visible text equal to the bare numeric `2`
- **AND** the equivalent labels for `pass: 1` and `pass: 3` MUST be `Pass 1` and `Pass 3` respectively

#### Scenario: Sanitize tab row click is hosted by parent SanitizerAuditInspector, not AuditPanel

- **WHEN** `<AuditPanel :active-tab="'sanitize'" :rows="entries" @select-row="parentHandler" />` is mounted with a parent that hosts `<SanitizerAuditInspector>`
- **THEN** clicking a row MUST emit `select-row` to the parent (consistent with the cross-tab emit contract)
- **AND** AuditPanel MUST NOT mount any DOM matching `SanitizerAuditInspector`, `inspector`, `drawer`, or `modal` inside its own root in response to the click
- **AND** the parent's `<SanitizerAuditInspector>` MUST be the only DOM that surfaces the row's full metadata view


<!-- @trace
source: sanitizer-audit-inspector-p0
updated: 2026-04-29
code:
  - docs/decisions.md
  - web/app/components/audit/sanitizerAuditBanner.ts
  - sidecar/src/codebus_agent/api/__init__.py
  - web/app/composables/useAuditJsonl.ts
  - web/app/composables/useSanitizerRules.ts
  - web/app/pages/explorer/[task_id].vue
  - web/app/pages/audit/sanitizer.vue
  - sidecar/src/codebus_agent/api/sanitizer_rules.py
  - web/app/components/audit/SanitizerAuditInspector.vue
  - web/app/components/audit/AuditPanel.vue
  - web/app/composables/useSanitizeAudit.ts
  - CLAUDE.md
  - docs/implementation-plan.md
  - web/app/pages/tutorial/[workspace_id]/[station_id].vue
tests:
  - web/tests/audit/fixtures/README.md
  - web/tests/audit/sanitize-overlay-integration.spec.ts
  - web/tests/audit/sanitizer-banner-single-source.spec.ts
  - web/tests/audit/AuditPanel-sanitize-tab.spec.ts
  - web/tests/audit/fixtures/sanitize-audit.jsonl
  - web/tests/audit/useSanitizerRules.spec.ts
  - sidecar/tests/api/test_sanitizer_rules.py
  - web/tests/audit/sanitizer-page.spec.ts
  - web/tests/audit/SanitizerAuditInspector.spec.ts
  - web/tests/audit/useSanitizeAudit.spec.ts
  - web/tests/audit/fixtures/sanitizer-rules.json
  - sidecar/tests/sanitizer/test_rules_version_parity.py
-->

---
### Requirement: useSseTask consumes bearer through useSidecar

The `useSseTask` composable SHALL accept a `taskId: string` matching `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$` and connect to the SSE endpoint `<base_url>/tasks/<task_id>/events` via the browser-native `EventSource` API. The composable MUST obtain the bearer token by calling `useSidecar()` and MUST NOT receive bearer/base-url values as direct arguments — passing those as parameters would tempt callers to bypass the IPC-only rule.

The composable MUST implement automatic reconnection with exponential backoff (initial delay 1 s, doubling per attempt, capped at 30 s); the final delay MUST surface to the caller via a reactive `status` field with values drawn from the closed set `{"connecting", "open", "reconnecting", "closed", "error"}`. The reactive return surface MUST expose `events` (array of received SSE messages, capped at 1000 entries with FIFO eviction), `status`, `error`, and a `close()` function that disconnects the EventSource immediately.

The composable MUST distinguish between two distinct error sources from the underlying `EventSource`:

1. **Connection-level errors** — fired by the browser when the SSE connection drops, fails to open, or is closed by the server. These dispatch as a generic `Event` (NOT a `MessageEvent`) and MUST be handled exclusively by the `EventSource.onerror` reconnection path; they MUST NOT push any entry into the reactive `events` array.
2. **Server-emitted `error` events** — fired when the sidecar transmits an SSE message with `event: error\ndata: <json>`. These dispatch as a `MessageEvent` whose `data` field is the JSON string. They MUST be appended to the `events` array as `{type: "error", data: <parsed json>}`.

The composable MUST NOT register `'error'` inside the catch-all `addEventListener` loop alongside other named events (`progress`, `done`, etc.), because EventSource's connection-error event shares the `'error'` name with server-emitted `event: error` SSE messages and a single shared handler cannot tell them apart. Instead, the composable MUST register a dedicated `'error'` listener whose callback gates the push by checking `event instanceof MessageEvent && typeof event.data === 'string'` before treating it as a server message.

#### Scenario: Bearer arrives via useSidecar, not parameters

- **WHEN** `useSseTask`'s function signature is inspected
- **THEN** the parameter list MUST be exactly `(taskId: string)` — no `bearer`, `token`, `baseUrl`, `headers`, or equivalent values may be accepted
- **AND** the implementation MUST call `useSidecar()` to obtain the bearer at runtime

#### Scenario: Invalid task_id rejected pre-connect

- **WHEN** `useSseTask("scan_INVALID")` is invoked (pattern violates the regex)
- **THEN** the composable MUST return a closed-state result without opening any `EventSource`
- **AND** `status.value` MUST equal `"error"` and `error.value` MUST reference the regex constraint

#### Scenario: Reconnect uses exponential backoff capped at 30 s

- **WHEN** the SSE connection drops mid-stream
- **THEN** the composable MUST attempt reconnection after delays of 1 s, 2 s, 4 s, 8 s, 16 s, 30 s, 30 s, ... in that sequence
- **AND** while waiting between attempts, `status.value` MUST equal `"reconnecting"`
- **AND** when a reconnect succeeds, `status.value` MUST flip back to `"open"`

#### Scenario: Events array capped at 1000 entries

- **WHEN** the SSE stream delivers a 1001st event without `close()` being called
- **THEN** the `events` reactive array MUST drop the oldest entry and append the newest
- **AND** the array length MUST remain exactly 1000 after the FIFO eviction

#### Scenario: Named error listener ignores connection-level errors

- **WHEN** the underlying `EventSource` dispatches a generic `Event` named `"error"` (e.g., the server closes the connection cleanly after a `done` event, or the network drops)
- **THEN** the composable's dedicated `'error'` listener MUST NOT push any entry into the `events` reactive array
- **AND** the `EventSource.onerror` reconnection path MUST still execute (close + scheduleReconnect)
- **AND** when the same EventSource later dispatches a `MessageEvent` named `"error"` with a JSON `data` string (i.e., a server-emitted `event: error\ndata: {...}` SSE message), the composable MUST append exactly one entry `{type: "error", data: <parsed json>}` to the `events` array


<!-- @trace
source: sidecar-sse-named-events-and-error-listener-fix
updated: 2026-05-03
code:
  - web/app/components/workspace-onramp/FolderPickerButton.vue
  - web/app/components/workspace-onramp/OnrampProgress.vue
  - web/app/components/workspace-onramp/WorkspaceOnrampCard.vue
  - web/app/composables/useWorkspaceOnramp.ts
  - web/dist
  - web/app/utils/workspace-id.ts
  - web/app/components/AppShell.vue
  - tauri/src-tauri/src/lib.rs
  - tauri/src-tauri/Cargo.toml
  - web/package.json
  - tauri/src-tauri/capabilities/default.json
  - web/app/pages/index.vue
  - sidecar/src/codebus_agent/api/tasks.py
  - web/app/composables/useSseTask.ts
tests:
  - web/tests/utils/workspace-id.spec.ts
  - sidecar/tests/auth/test_workspace_id_parity.py
  - web/tests/onramp/WorkspaceOnrampCard.spec.ts
  - tauri/src-tauri/tests/dialog_plugin_smoke.rs
  - web/tests/onramp/FolderPickerButton.spec.ts
  - web/tests/onramp/useWorkspaceOnramp.spec.ts
  - web/tests/onboarding/index-page-redirect.spec.ts
  - web/tests/setup.ts
  - sidecar/tests/api/test_tasks_sse_wire_format.py
  - web/tests/onramp/OnrampProgress.spec.ts
  - web/tests/composables/useSseTask.connection-error.spec.ts
-->

---
### Requirement: Frontend typecheck baseline stays at zero errors

The `web/` package SHALL maintain a `vue-tsc` typecheck baseline of exactly zero errors at every committed state of `main`. Any Spectra change that lands under `openspec/changes/archive/` MUST NOT introduce a new TS diagnostic that would cause `cd web && npm run typecheck` (which delegates to `nuxt typecheck` → vue-tsc in project-references build mode) to exit non-zero.

A defensive test `web/tests/types/typecheck-baseline.spec.ts` SHALL spawn `vue-tsc --build --noEmit` from inside the test process and assert exit code zero. The `--build` flag is required because `web/tsconfig.json` is a project-references-only file (`files: []` + four references); `vue-tsc -p .` without `--build` does not traverse the references and silently checks nothing. The test MUST run as part of the normal `npm run test` invocation (i.e., `vitest run`) so the next regression is caught at the CI / pre-commit gate, not at the next manual `npm run typecheck` audit.

If a future change legitimately needs to relax this baseline (e.g., a planned migration with intermediate type errors), the change proposal MUST justify the relaxation in its Non-Goals section and the defensive test MUST be updated in the same change to permit the documented diagnostics — silent baseline drift MUST NOT occur.

#### Scenario: vue-tsc reports zero errors against the current main

- **WHEN** the repository is at a clean checkout of `main` and `cd web && npx vue-tsc --build --noEmit` runs to completion
- **THEN** the process exit code MUST be `0`
- **AND** the stdout/stderr MUST NOT contain any line matching the pattern `error TS\d+:`

#### Scenario: Defensive vitest test asserts zero typecheck errors

- **WHEN** `npm run test` runs to completion against a clean `main`
- **THEN** the `tests/types/typecheck-baseline.spec.ts` test MUST pass
- **AND** the assertion MUST be that `vue-tsc --build --noEmit` exited with code `0`

#### Scenario: Defensive vitest test surfaces the offending diagnostic on regression

- **WHEN** an edit introduces a new TS diagnostic to any `.vue` or `.ts` file under `web/app/` and `npm run test` runs
- **THEN** the `tests/types/typecheck-baseline.spec.ts` test MUST report a failure
- **AND** the failure message MUST contain the captured `vue-tsc` stdout/stderr (which includes the offending file path and the TS error code) so the developer can locate the regression from the vitest output alone

---
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

---
### Requirement: TopBar exposes a settings entry routed to /settings

The frontend SHALL render a settings entry button (gear icon) in `<TopBar>`. Clicking the button MUST route to `/settings` via the Vue Router. The existing `open-settings` emit on `<TopBar>` MUST be wired by the layout host to a router push handler so the existing event signature stays intact while the actual navigation happens at the layout level.

The settings entry MUST appear on every layout-level page (`/tutorial/...` / `/explorer/...` / `/audit/...` / `/settings`) but MUST NOT appear on `/onboarding/*` routes — the onboarding wizard explicitly does not allow escape into other UI surfaces (see `provider-onboarding` Requirement "Onboarding wizard exposes three sequential routes").

#### Scenario: Settings button visible on tutorial page

- **WHEN** the user is on `/tutorial/ws_xxx/index`
- **THEN** `<TopBar>` MUST render a button with `data-testid="topbar-settings"`
- **AND** clicking it MUST route to `/settings`

#### Scenario: Settings button hidden on onboarding routes

- **WHEN** the user is on `/onboarding/welcome`, `/onboarding/providers`, or `/onboarding/done`
- **THEN** `<TopBar>` MUST NOT render any button with `data-testid="topbar-settings"`

---
### Requirement: useProviderConfig composable exposes provider pool state

The frontend SHALL ship `web/app/composables/useProviderConfig.ts` as a module-level singleton (matching the `useQaSession` / `useIntervention` convention). The composable MUST expose:

- `providers: Ref<ProviderEntry[]>` — read-only snapshot of the provider pool
- `bindings: Ref<{ reasoning: string; judge: string; chat: string; embed: string }>` — current role bindings
- `piiMode: Ref<{ mode: 'rule' | 'llm'; provider_id: string | null }>`
- `loadConfig(): Promise<void>` — fetches `/settings/providers` and updates state
- `upsertProvider(entry: ProviderEntry): Promise<void>` — POSTs to `/settings/providers`
- `deleteProvider(id: string): Promise<void>` — DELETEs `/settings/providers/{id}`
- `setBinding(role: string, provider_id: string): Promise<void>` — PUTs `/settings/bindings`
- `setPiiMode(mode: 'rule' | 'llm', provider_id?: string): Promise<void>` — PUTs `/settings/pii-mode`

The composable MUST subscribe to the app-level `provider_config_changed` SSE event and re-fetch state automatically on receipt. The composable MUST NOT cache API keys; all `api_key` flows go through Tauri keyring IPC directly without crossing this composable.

#### Scenario: Two callers receive same singleton state

- **WHEN** two components both call `useProviderConfig()`
- **THEN** the returned `providers` / `bindings` / `piiMode` refs MUST satisfy `Object.is(a.providers, b.providers) === true` for all three

#### Scenario: SSE event triggers re-fetch

- **WHEN** the composable is mounted and the app-level SSE channel emits `provider_config_changed`
- **THEN** the composable MUST issue a `GET /settings/providers` request within 100 ms
- **AND** the local refs MUST update once the response arrives

#### Scenario: useProviderConfig source has no api_key field

- **WHEN** the test suite greps `web/app/composables/useProviderConfig.ts` for the literal string `api_key`
- **THEN** zero matches MUST be found in non-comment lines

---
### Requirement: Index page redirects to onboarding when LLM dependencies are not configured

The route entry `/` (rendered by `web/app/pages/index.vue`) SHALL call `GET /healthz` on mount and redirect via `router.replace('/onboarding/welcome')` when any of `dependency.llm_chat` / `dependency.llm_embed` reports `not-configured`. The page MUST NOT render its existing entry-point UI (workspace picker etc.) until the dependency snapshot confirms readiness.

This redirect logic is duplicated from the global Nuxt route middleware (`provider-onboarding` Requirement "Startup detection redirects to onboarding when any LLM dependency is not configured") because direct landing on `/` is a common path that benefits from inline gating without relying on middleware ordering.

#### Scenario: Empty keyring routes to onboarding

- **WHEN** the user opens the app for the first time and `pages/index.vue` mounts
- **THEN** the page MUST issue `GET /healthz`
- **AND** when the response has `dependency.llm_chat: "not-configured"`, the page MUST redirect to `/onboarding/welcome`
- **AND** the workspace picker UI MUST NOT render

#### Scenario: Configured keyring renders entry UI

- **WHEN** the user has completed onboarding and revisits `/`
- **THEN** the page MUST issue `GET /healthz`
- **AND** when the response has `dependency.llm_chat: "ready"` and `dependency.llm_embed: "ready"`, the page MUST render the existing workspace picker UI without redirecting
