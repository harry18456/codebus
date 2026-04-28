import { ref, type Ref } from 'vue'

// Single source for sidecar bearer + base URL. Values come from a Tauri IPC
// handshake at app start and stay in memory only — they MUST NOT be written
// to localStorage, sessionStorage, IndexedDB, or document.cookie. The
// `Sidecar bearer and base URL come from Tauri IPC` invariant and CLAUDE.md
// invariant #5 (`Bearer + loopback 不可鬆綁`) extend to the frontend through
// this composable.

interface SidecarHandshake {
  bearer: string
  port: number
}

const bearer = ref('')
const baseUrl = ref('')
const ready = ref(false)
let bootstrapped = false

async function bootstrap(): Promise<void> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const handshake = await invoke<SidecarHandshake>('sidecar_handshake')
    if (!handshake.bearer || !Number.isInteger(handshake.port)) {
      // eslint-disable-next-line no-console
      console.error('[useSidecar] handshake returned invalid shape:', handshake)
      return
    }
    bearer.value = handshake.bearer
    baseUrl.value = `http://127.0.0.1:${handshake.port}`
    ready.value = true
  } catch (err) {
    // Tauri IPC unavailable (browser-only `npm run dev`, or
    // `sidecar_handshake` not yet registered). Stay in ready=false; pages can
    // surface this as a "sidecar not connected" state.
    //
    // Surface the real error to DevTools so manual smoke can see whether
    // it's a missing-Tauri-runtime issue (browser context) or a real
    // sidecar spawn failure (PingError::Spawn / HandshakeClosed / …).
    // eslint-disable-next-line no-console
    console.error('[useSidecar] bootstrap failed:', err)
  }
}

async function sidecarFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  if (!ready.value) {
    throw new Error('useSidecar: handshake not complete; bearer is unavailable')
  }
  const headers = new Headers(init?.headers ?? {})
  if (!headers.has('Authorization')) {
    headers.set('Authorization', `Bearer ${bearer.value}`)
  }
  const target =
    typeof input === 'string' && input.startsWith('/')
      ? `${baseUrl.value}${input}`
      : input
  return fetch(target, { ...init, headers })
}

// ----- auth-flow typed wrappers --------------------------------------------
// Match the sidecar Pydantic schemas defined in
// `sidecar/src/codebus_agent/auth/service.py`. Wrappers go through
// `sidecarFetch` so the Authorization header + base URL come from the same
// in-memory bearer + handshake — no parallel auth handshake mechanism.

export type WorkspaceType = 'folder' | 'topic'
export type GrantScenario =
  | 'first_run'
  | 'scope_reconfirm'
  | 'scope_upgrade_new_kind'

export interface GrantScope {
  llm_provider: string
  llm_model: string
  outbound_endpoint: string
}

export interface GrantRequest {
  workspace_type: WorkspaceType
  workspace_source: Record<string, unknown>
  scenario: GrantScenario
  scope: GrantScope
  sanitizer_rules_version: string
  user_ack: string[]
}

export interface GrantResponse {
  session_id: string
  workspace_id: string
  granted_at: string
}

export interface DenyRequest {
  workspace_type: WorkspaceType
  workspace_source: Record<string, unknown>
  scenario: GrantScenario
  reason: 'user_cancelled' | 'app_closed'
}

export interface RevokeRequest {
  session_id: string
  trigger: 'settings_revoke'
}

export interface AuthStatusResponse {
  has_active_grant: boolean
  session_id: string | null
  last_grant: Record<string, unknown> | null
  current_rules_version: string
}

class AuthError extends Error {
  readonly code: string
  readonly status: number
  constructor(code: string, status: number, message: string) {
    super(message)
    this.code = code
    this.status = status
  }
}

async function postJson<T>(path: string, body: unknown): Promise<T> {
  const res = await sidecarFetch(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body)
  })
  return parseAuthResponse<T>(res)
}

async function parseAuthResponse<T>(res: Response): Promise<T> {
  if (res.status === 204) {
    return undefined as unknown as T
  }
  if (!res.ok) {
    let detail: { code?: string; message?: string } | undefined
    try {
      const payload = (await res.json()) as { detail?: typeof detail }
      detail = payload?.detail
    } catch {
      detail = undefined
    }
    throw new AuthError(
      detail?.code ?? `HTTP_${res.status}`,
      res.status,
      detail?.message ?? `auth endpoint failed with status ${res.status}`
    )
  }
  return (await res.json()) as T
}

async function grant(req: GrantRequest): Promise<GrantResponse> {
  return postJson<GrantResponse>('/auth/grant', req)
}

async function deny(req: DenyRequest): Promise<void> {
  await postJson<void>('/auth/deny', req)
}

async function revoke(req: RevokeRequest): Promise<void> {
  await postJson<void>('/auth/revoke', req)
}

async function status(workspaceId: string): Promise<AuthStatusResponse> {
  const params = new URLSearchParams({ workspace_id: workspaceId })
  const res = await sidecarFetch(`/auth/status?${params.toString()}`)
  return parseAuthResponse<AuthStatusResponse>(res)
}

/**
 * Mirror of `codebus_agent.auth.service.workspace_id_for_path`. SHA-256
 * of the canonical lowercase POSIX path, prefix `ws_` + 12 hex chars.
 * Use this when the page only has a workspace path string and needs to
 * call `useSidecar().status(workspaceId)`.
 */
async function workspaceIdForPath(path: string): Promise<string> {
  const canonical = path.replace(/\\/g, '/').toLowerCase()
  const encoded = new TextEncoder().encode(canonical)
  const digest = await crypto.subtle.digest('SHA-256', encoded)
  const hex = Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
  return `ws_${hex.slice(0, 12)}`
}

interface SidecarApi {
  bearer: Ref<string>
  baseUrl: Ref<string>
  ready: Ref<boolean>
  fetch: typeof fetch
  grant: (req: GrantRequest) => Promise<GrantResponse>
  deny: (req: DenyRequest) => Promise<void>
  revoke: (req: RevokeRequest) => Promise<void>
  status: (workspaceId: string) => Promise<AuthStatusResponse>
  workspaceIdForPath: (path: string) => Promise<string>
}

export function useSidecar(): SidecarApi {
  if (!bootstrapped) {
    bootstrapped = true
    void bootstrap()
  }
  return {
    bearer,
    baseUrl,
    ready,
    fetch: sidecarFetch,
    grant,
    deny,
    revoke,
    status,
    workspaceIdForPath
  }
}

export { AuthError }
