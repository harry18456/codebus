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
      return
    }
    bearer.value = handshake.bearer
    baseUrl.value = `http://127.0.0.1:${handshake.port}`
    ready.value = true
  } catch {
    // Tauri IPC unavailable (browser-only `npm run dev`, or
    // `sidecar_handshake` not yet registered). Stay in ready=false; pages can
    // surface this as a "sidecar not connected" state.
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

interface SidecarApi {
  bearer: Ref<string>
  baseUrl: Ref<string>
  ready: Ref<boolean>
  fetch: typeof fetch
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
    fetch: sidecarFetch
  }
}
