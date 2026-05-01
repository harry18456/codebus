import { ref, watch, type Ref } from 'vue'
import { useSidecar } from './useSidecar'
import type { SseEvent } from './useSseTask'

// useProviderConfig — module-level singleton for the provider pool /
// role bindings / PII mode state surfaced on `/settings`.
//
// Spec: openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Requirement: Provider pool CRUD touches keyring and config
//   Requirement: Role binding change propagates via hot-swap
// Plus: openspec/changes/provider-settings-and-onboarding/specs/sidecar-runtime/spec.md
//   Requirement: Sidecar accepts provider config mutation endpoints (consumer)
//
// Invariants:
//   - State lives at module scope; every `useProviderConfig()` call returns
//     references to the SAME ref instances (same convention as
//     `useQaSession` / `useIntervention`).
//   - This composable NEVER touches API keys. The Tauri keyring IPC is the
//     only path through which secrets travel; the sidecar settings endpoint
//     payload schema also forbids the field.
//   - Mutations subscribe to the app-level SSE channel `provider_config_changed`
//     so other open tabs / components stay in lockstep without polling.

export interface ProviderSpec {
  id: string
  type: string
  model: string
  base_url: string
}

export type RoleName = 'reasoning' | 'judge' | 'chat' | 'embed'

export type PiiMode = 'rule' | 'llm'

export interface ProviderPoolSnapshot {
  providers: ProviderSpec[]
  bindings: Record<string, string>
  pii_mode: PiiMode
  pii_provider_id: string | null
}

export interface UseProviderConfigApi {
  providers: Ref<ProviderSpec[]>
  bindings: Ref<Record<string, string>>
  piiMode: Ref<PiiMode>
  piiProviderId: Ref<string | null>
  loaded: Ref<boolean>
  loadConfig: () => Promise<void>
  upsertProvider: (spec: ProviderSpec) => Promise<void>
  deleteProvider: (id: string) => Promise<void>
  setBinding: (role: RoleName, providerId: string) => Promise<void>
  setPiiMode: (mode: PiiMode, providerId?: string | null) => Promise<void>
  attachEventStream: (events: Ref<SseEvent[]>) => void
}

const _providers = ref<ProviderSpec[]>([])
const _bindings = ref<Record<string, string>>({})
const _piiMode = ref<PiiMode>('rule')
const _piiProviderId = ref<string | null>(null)
const _loaded = ref(false)

function applySnapshot(snap: ProviderPoolSnapshot): void {
  _providers.value = snap.providers.map((p) => ({ ...p }))
  _bindings.value = { ...snap.bindings }
  _piiMode.value = snap.pii_mode
  _piiProviderId.value = snap.pii_provider_id
  _loaded.value = true
}

async function loadConfig(): Promise<void> {
  const { fetch } = useSidecar()
  const res = await fetch('/settings/providers')
  if (!res.ok) {
    throw new Error(`loadConfig failed: ${res.status}`)
  }
  applySnapshot((await res.json()) as ProviderPoolSnapshot)
}

async function upsertProvider(spec: ProviderSpec): Promise<void> {
  const { fetch } = useSidecar()
  const res = await fetch('/settings/providers', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(spec)
  })
  if (!res.ok) {
    throw new Error(`upsertProvider failed: ${res.status}`)
  }
  // Optimistic local update; SSE re-fetch will reconcile if server differs.
  const filtered = _providers.value.filter((p) => p.id !== spec.id)
  _providers.value = [...filtered, { ...spec }]
}

async function deleteProvider(id: string): Promise<void> {
  const { fetch } = useSidecar()
  const res = await fetch(`/settings/providers/${id}`, { method: 'DELETE' })
  if (!res.ok) {
    throw new Error(`deleteProvider failed: ${res.status}`)
  }
  _providers.value = _providers.value.filter((p) => p.id !== id)
}

async function setBinding(role: RoleName, providerId: string): Promise<void> {
  const { fetch } = useSidecar()
  const body: Partial<Record<RoleName, string>> = { [role]: providerId }
  const res = await fetch('/settings/bindings', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body)
  })
  if (!res.ok) {
    throw new Error(`setBinding failed: ${res.status}`)
  }
  _bindings.value = { ..._bindings.value, [role]: providerId }
}

async function setPiiMode(
  mode: PiiMode,
  providerId: string | null = null
): Promise<void> {
  const { fetch } = useSidecar()
  const res = await fetch('/settings/pii-mode', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ mode, provider_id: providerId })
  })
  if (!res.ok) {
    throw new Error(`setPiiMode failed: ${res.status}`)
  }
  _piiMode.value = mode
  _piiProviderId.value = providerId
}

// SSE-driven re-fetch on `provider_config_changed`. Page layer feeds the
// app channel events ref into this hook; the composable observes new
// entries and debounces a re-fetch within the 100 ms budget the spec
// gives clients to converge on the new snapshot. The 50 ms broker
// coalesce window plus this 100 ms debounce caps the user-perceived
// staleness at roughly 150 ms after the mutation.
const RE_FETCH_DEBOUNCE_MS = 100
let _attachedEvents: Ref<SseEvent[]> | null = null
let _watchStop: (() => void) | null = null
let _refetchTimer: ReturnType<typeof setTimeout> | null = null

function attachEventStream(events: Ref<SseEvent[]>): void {
  if (_watchStop) {
    _watchStop()
    _watchStop = null
  }
  _attachedEvents = events
  let lastSeenLength = events.value.length
  _watchStop = watch(
    () => events.value.length,
    (next) => {
      if (next <= lastSeenLength) {
        lastSeenLength = next
        return
      }
      const fresh = events.value.slice(lastSeenLength)
      lastSeenLength = next
      const hasProviderChange = fresh.some(
        (ev) => ev?.type === 'provider_config_changed'
      )
      if (!hasProviderChange) return
      if (_refetchTimer) clearTimeout(_refetchTimer)
      _refetchTimer = setTimeout(() => {
        _refetchTimer = null
        void loadConfig().catch(() => {
          // Re-fetch errors surface to console only — the SSE event is
          // a hint, not a hard contract; pages can still observe stale
          // state until the next manual refresh.
        })
      }, RE_FETCH_DEBOUNCE_MS)
    }
  )
}

export function useProviderConfig(): UseProviderConfigApi {
  return {
    providers: _providers,
    bindings: _bindings,
    piiMode: _piiMode,
    piiProviderId: _piiProviderId,
    loaded: _loaded,
    loadConfig,
    upsertProvider,
    deleteProvider,
    setBinding,
    setPiiMode,
    attachEventStream
  }
}

// Test-only reset hook. Lives in production source so the test suite
// can call it from `beforeEach` without monkey-patching internal state.
export function _resetForTest(): void {
  _providers.value = []
  _bindings.value = {}
  _piiMode.value = 'rule'
  _piiProviderId.value = null
  _loaded.value = false
  if (_watchStop) {
    _watchStop()
    _watchStop = null
  }
  _attachedEvents = null
  if (_refetchTimer) {
    clearTimeout(_refetchTimer)
    _refetchTimer = null
  }
}
