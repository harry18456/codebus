import { ref, type Ref } from 'vue'

// useIntervention — module-level singleton driving the three Phase 6
// intervention points (skip / regen / switch workspace).
//
// Spec: openspec/changes/phase6-step29-intervention-points/specs/...
// Design Decision 4: state lives at module scope; every call to
// `useIntervention()` returns references to the SAME ref instances.
// `<InterventionConfirmModal>` subscribes to `pendingAction`; the leaf
// buttons (`<SkipStationButton>` / `<RegenStationButton>` /
// `<SwitchWorkspaceMenu>`) call `requestSkip(...)` / `requestRegen(...)` /
// `requestSwitchWorkspace(...)` imperatively and pass the action to
// perform on confirm.

export interface SkipPayload {
  stationId: string
  stationTitle: string
}

export interface RegenPayload {
  stationId: string
  stationTitle: string
  taskId: string
  workspaceRoot: string
}

export interface SwitchPayload {
  // Switch workspace doesn't need any payload; the modal copy and the
  // navigation target are both static. The interface exists so the
  // discriminator union shape stays uniform.
  reason?: string
}

export type PendingAction =
  | { kind: 'skip'; payload: SkipPayload; onConfirm: () => Promise<void> | void }
  | { kind: 'regen'; payload: RegenPayload; onConfirm: () => Promise<void> | void }
  | {
      kind: 'switch'
      payload: SwitchPayload
      onConfirm: () => Promise<void> | void
    }

export interface UseInterventionApi {
  pendingAction: Ref<PendingAction | null>
  requestSkip: (
    payload: SkipPayload & { onConfirm: () => Promise<void> | void }
  ) => void
  requestRegen: (
    payload: RegenPayload & { onConfirm: () => Promise<void> | void }
  ) => void
  requestSwitchWorkspace: (
    payload: SwitchPayload & { onConfirm: () => Promise<void> | void }
  ) => void
  confirm: () => Promise<void>
  cancel: () => void
}

const _pendingAction = ref<PendingAction | null>(null)

function requestSkip(
  args: SkipPayload & { onConfirm: () => Promise<void> | void }
): void {
  const { onConfirm, ...payload } = args
  _pendingAction.value = { kind: 'skip', payload, onConfirm }
}

function requestRegen(
  args: RegenPayload & { onConfirm: () => Promise<void> | void }
): void {
  const { onConfirm, ...payload } = args
  _pendingAction.value = { kind: 'regen', payload, onConfirm }
}

function requestSwitchWorkspace(
  args: SwitchPayload & { onConfirm: () => Promise<void> | void }
): void {
  const { onConfirm, ...payload } = args
  _pendingAction.value = { kind: 'switch', payload, onConfirm }
}

async function confirm(): Promise<void> {
  const action = _pendingAction.value
  if (action === null) return
  // Clear the pending action BEFORE invoking onConfirm so the modal
  // closes immediately; the action's own side-effects (router push,
  // sidecar fetch, etc.) run after the modal is gone. If the onConfirm
  // throws, the action is already cleared — callers handle errors via
  // their own UI (e.g. RegenStationButton displays sidecar errors).
  _pendingAction.value = null
  await action.onConfirm()
}

function cancel(): void {
  _pendingAction.value = null
}

export function useIntervention(): UseInterventionApi {
  return {
    pendingAction: _pendingAction,
    requestSkip,
    requestRegen,
    requestSwitchWorkspace,
    confirm,
    cancel
  }
}

// Test-only export. Production code MUST NOT call this.
export function _resetForTest(): void {
  _pendingAction.value = null
}
