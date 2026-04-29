import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, h, provide } from 'vue'

// Mock useQaSession so QAEntry's imperative call lands on a spy.
const startMock = vi.fn(async (_prompt: string, _stationId: string | null) => {})
vi.mock('~/composables/useQaSession', () => ({
  useQaSession: () => ({
    start: startMock,
    open: { value: false },
    turns: { value: [] },
    currentTaskId: { value: null },
    status: { value: 'idle' },
    error: { value: null },
    openDrawer: vi.fn(),
    close: vi.fn()
  })
}))

const routerPushMock = vi.fn()
vi.mock('vue-router', () => ({
  useRouter: () => ({ push: routerPushMock })
}))

import QAEntry from '~/components/content/QAEntry.vue'

beforeEach(() => {
  startMock.mockReset()
  routerPushMock.mockReset()
})

function makeProvideHost(stationId: string | null) {
  return defineComponent({
    setup() {
      if (stationId !== null) provide('currentStationId', stationId)
      return () => h(QAEntry, { prompt: 'why atomic write?' })
    }
  })
}

describe('QAEntry imperative call', () => {
  it('click invokes useQaSession.start with prompt + injected stationId', async () => {
    const Host = makeProvideHost('s03-production')
    const wrapper = mount(Host)
    await wrapper.find('button').trigger('click')
    expect(startMock).toHaveBeenCalledTimes(1)
    expect(startMock).toHaveBeenCalledWith('why atomic write?', 's03-production')
    wrapper.unmount()
  })

  it('falls back to null when no currentStationId is provided', async () => {
    const Host = makeProvideHost(null)
    const wrapper = mount(Host)
    await wrapper.find('button').trigger('click')
    expect(startMock).toHaveBeenCalledTimes(1)
    expect(startMock).toHaveBeenCalledWith('why atomic write?', null)
    wrapper.unmount()
  })

  it('does NOT call router.push or change the URL', async () => {
    const Host = makeProvideHost('s03-production')
    const wrapper = mount(Host)
    await wrapper.find('button').trigger('click')
    expect(routerPushMock).not.toHaveBeenCalled()
    wrapper.unmount()
  })
})
