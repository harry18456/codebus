import { computed, reactive, type ComputedRef } from 'vue'

import type { GrantScenario } from './useSidecar'

// Pure modal flow state — owns ack checkbox booleans, submit-enabled
// derivation, scenario tracking. NO IPC; the four typed wrappers
// (`grant` / `deny` / `revoke` / `status`) live on `useSidecar()` and
// MUST stay there per design D-A10. Splitting flow state from IPC keeps
// the modal testable in isolation when component tests land in Phase B.

export interface AuthorizationFlowState {
  scenario: GrantScenario
  baseAcks: Record<string, boolean>
  newKindAcks: Record<string, boolean>
}

const BASE_ACK_KEYS = [
  'raw_stays_local',
  'no_kb_persist'
] as const
type BaseAckKey = (typeof BASE_ACK_KEYS)[number]

interface UseAuthorizationParams {
  scenario: GrantScenario
  llmProvider: string
  newKinds?: string[]
}

export interface UseAuthorizationReturn {
  scenario: ComputedRef<GrantScenario>
  baseAckKeys: string[]
  ackFlags: AuthorizationFlowState
  submitEnabled: ComputedRef<boolean>
  setAck: (key: string, value: boolean) => void
  reset: (next?: Partial<UseAuthorizationParams>) => void
  buildUserAck: () => string[]
}

function providerAckKey(provider: string): string {
  return `outbound_to_${provider}`
}

function emptyBaseAcks(provider: string): Record<string, boolean> {
  const out: Record<string, boolean> = {}
  for (const k of BASE_ACK_KEYS) {
    out[k as BaseAckKey] = false
  }
  out[providerAckKey(provider)] = false
  return out
}

function emptyNewKindAcks(newKinds: string[]): Record<string, boolean> {
  const out: Record<string, boolean> = {}
  for (const kind of newKinds) {
    out[kind] = false
  }
  return out
}

export function useAuthorization(
  params: UseAuthorizationParams
): UseAuthorizationReturn {
  const provider = params.llmProvider
  const newKinds = params.newKinds ?? []

  const state: {
    current: AuthorizationFlowState
    provider: string
  } = reactive({
    current: {
      scenario: params.scenario,
      baseAcks: emptyBaseAcks(provider),
      newKindAcks: emptyNewKindAcks(newKinds)
    },
    provider
  })

  const submitEnabled = computed(() => {
    const allBase = Object.values(state.current.baseAcks).every(Boolean)
    const allNew = Object.values(state.current.newKindAcks).every(Boolean)
    return allBase && allNew
  })

  function setAck(key: string, value: boolean): void {
    if (key in state.current.baseAcks) {
      state.current.baseAcks[key] = value
      return
    }
    if (key in state.current.newKindAcks) {
      state.current.newKindAcks[key] = value
      return
    }
    // Unknown key: ignore silently rather than throw — this is UI flow state,
    // not a security boundary. The submit button stays disabled if the
    // intended ack never flips true, which is the correct fail-safe.
  }

  function reset(next?: Partial<UseAuthorizationParams>): void {
    const nextProvider = next?.llmProvider ?? state.provider
    const nextScenario = next?.scenario ?? state.current.scenario
    const nextNewKinds = next?.newKinds ?? Object.keys(state.current.newKindAcks)
    state.provider = nextProvider
    state.current.scenario = nextScenario
    state.current.baseAcks = emptyBaseAcks(nextProvider)
    state.current.newKindAcks = emptyNewKindAcks(nextNewKinds)
  }

  function buildUserAck(): string[] {
    const flags: string[] = []
    for (const k of BASE_ACK_KEYS) {
      if (state.current.baseAcks[k as BaseAckKey]) flags.push(k)
    }
    const providerKey = providerAckKey(state.provider)
    if (state.current.baseAcks[providerKey]) flags.push(providerKey)
    for (const [kind, ticked] of Object.entries(state.current.newKindAcks)) {
      if (ticked) flags.push(`new_kind:${kind}`)
    }
    return flags
  }

  const baseAckKeys = computed(() => Object.keys(state.current.baseAcks)).value

  const scenarioRef = computed(() => state.current.scenario)

  return {
    scenario: scenarioRef,
    baseAckKeys,
    ackFlags: state.current,
    submitEnabled,
    setAck,
    reset,
    buildUserAck
  }
}
