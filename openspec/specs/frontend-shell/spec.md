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
### Requirement: AuditPanel surfaces seven workspace-level audit JSONL tabs

The `AuditPanel.vue` component SHALL render exactly seven tabs in the order `sanitize`, `tool`, `reasoning`, `token`, `llm`, `kb_growth`, `generator`, mirroring the seven workspace-level audit JSONL files under `<workspace>/.codebus/` declared by CLAUDE.md (`七層 Audit JSONL` section). The component MUST expose an `activeTab` prop accepting any of these seven keys; passing an unrecognised key MUST be a TypeScript compile-time error.

The component MUST NOT render rows from in-source sample data. The `CB_AUDIT_SAMPLES` literal from `design/v1/shell.js` is mockup-only fixture data per `design/v1/README.md §四`; the production component MUST receive its rows via a `rows` prop (or equivalent injection) and MUST render an empty state when the array is empty. No `web/app/` source file may contain a literal copy of `CB_AUDIT_SAMPLES` or any element of it.

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
### Requirement: useSseTask consumes bearer through useSidecar

The `useSseTask` composable SHALL accept a `taskId: string` matching `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$` and connect to the SSE endpoint `<base_url>/tasks/<task_id>/events` via the browser-native `EventSource` API. The composable MUST obtain the bearer token by calling `useSidecar()` and MUST NOT receive bearer/base-url values as direct arguments — passing those as parameters would tempt callers to bypass the IPC-only rule.

The composable MUST implement automatic reconnection with exponential backoff (initial delay 1 s, doubling per attempt, capped at 30 s); the final delay MUST surface to the caller via a reactive `status` field with values drawn from the closed set `{"connecting", "open", "reconnecting", "closed", "error"}`. The reactive return surface MUST expose `events` (array of received SSE messages, capped at 1000 entries with FIFO eviction), `status`, `error`, and a `close()` function that disconnects the EventSource immediately.

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
