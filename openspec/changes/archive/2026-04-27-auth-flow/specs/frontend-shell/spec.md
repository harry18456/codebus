## MODIFIED Requirements

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
