import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  useIntervention,
  _resetForTest
} from '~/composables/useIntervention'

beforeEach(() => {
  _resetForTest()
})

afterEach(() => {
  _resetForTest()
})

describe('useIntervention singleton state machine', () => {
  it('two callers receive the same singleton state (Object.is)', () => {
    const a = useIntervention()
    const b = useIntervention()
    expect(Object.is(a.pendingAction, b.pendingAction)).toBe(true)
  })

  it('initial pendingAction is null', () => {
    const api = useIntervention()
    expect(api.pendingAction.value).toBeNull()
  })

  it('requestSkip sets pendingAction to kind=skip with payload + onConfirm', () => {
    const api = useIntervention()
    const onConfirm = vi.fn()
    api.requestSkip({
      stationId: 's02-mqtt-client',
      stationTitle: 'MQTT Client',
      onConfirm
    })
    const action = api.pendingAction.value
    expect(action).not.toBeNull()
    expect(action?.kind).toBe('skip')
    expect(action?.payload).toEqual({
      stationId: 's02-mqtt-client',
      stationTitle: 'MQTT Client'
    })
    expect(typeof action?.onConfirm).toBe('function')
  })

  it('requestRegen sets pendingAction to kind=regen with payload + onConfirm', () => {
    const api = useIntervention()
    const onConfirm = vi.fn()
    api.requestRegen({
      stationId: 's03-storage',
      stationTitle: 'Storage adapter',
      taskId: 'generate_abc12345',
      workspaceRoot: 'D:/projects/demo',
      onConfirm
    })
    const action = api.pendingAction.value
    expect(action).not.toBeNull()
    expect(action?.kind).toBe('regen')
    expect(action?.payload).toEqual({
      stationId: 's03-storage',
      stationTitle: 'Storage adapter',
      taskId: 'generate_abc12345',
      workspaceRoot: 'D:/projects/demo'
    })
    expect(typeof action?.onConfirm).toBe('function')
  })

  it('requestSwitchWorkspace sets pendingAction to kind=switch with onConfirm', () => {
    const api = useIntervention()
    const onConfirm = vi.fn()
    api.requestSwitchWorkspace({ onConfirm })
    const action = api.pendingAction.value
    expect(action).not.toBeNull()
    expect(action?.kind).toBe('switch')
    expect(typeof action?.onConfirm).toBe('function')
  })

  it('confirm() invokes onConfirm and clears pendingAction', async () => {
    const api = useIntervention()
    const onConfirm = vi.fn().mockResolvedValue(undefined)
    api.requestSkip({
      stationId: 's02-mqtt-client',
      stationTitle: 'MQTT Client',
      onConfirm
    })
    await api.confirm()
    expect(onConfirm).toHaveBeenCalledTimes(1)
    expect(api.pendingAction.value).toBeNull()
  })

  it('cancel() clears pendingAction without invoking onConfirm', () => {
    const api = useIntervention()
    const onConfirm = vi.fn()
    api.requestSkip({
      stationId: 's02-mqtt-client',
      stationTitle: 'MQTT Client',
      onConfirm
    })
    api.cancel()
    expect(onConfirm).not.toHaveBeenCalled()
    expect(api.pendingAction.value).toBeNull()
  })

  it('confirm() with no pending action is a no-op', async () => {
    const api = useIntervention()
    await expect(api.confirm()).resolves.toBeUndefined()
    expect(api.pendingAction.value).toBeNull()
  })

  it('requestX while another action is pending replaces it (single ref)', () => {
    const api = useIntervention()
    const skipConfirm = vi.fn()
    api.requestSkip({
      stationId: 's01',
      stationTitle: 's1',
      onConfirm: skipConfirm
    })
    expect(api.pendingAction.value?.kind).toBe('skip')
    const switchConfirm = vi.fn()
    api.requestSwitchWorkspace({ onConfirm: switchConfirm })
    expect(api.pendingAction.value?.kind).toBe('switch')
    expect(skipConfirm).not.toHaveBeenCalled()
  })
})
