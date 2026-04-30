import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick, ref, type Ref } from 'vue'

// Hoisted mock for vue-router so we can drive useRoute().name through
// each test scenario.
const { useRouteMock } = vi.hoisted(() => ({ useRouteMock: vi.fn() }))
vi.mock('vue-router', async () => {
  const actual = await vi.importActual<typeof import('vue-router')>('vue-router')
  return {
    ...actual,
    useRoute: () => useRouteMock(),
    useRouter: () => ({ push: vi.fn() })
  }
})

import SwitchWorkspaceMenu from '~/components/intervention/SwitchWorkspaceMenu.vue'
import {
  useIntervention,
  _resetForTest as resetIntervention
} from '~/composables/useIntervention'

function setRouteName(name: string, path: string = '/'): void {
  useRouteMock.mockReturnValue({
    name,
    path,
    fullPath: path,
    query: {},
    params: {}
  })
}

beforeEach(() => {
  useRouteMock.mockReset()
  resetIntervention()
})

afterEach(() => {
  resetIntervention()
})

describe('SwitchWorkspaceMenu render & interaction', () => {
  it('renders chip with workspace basename when on a tutorial-level page', () => {
    setRouteName('tutorial-workspace_id-station_id', '/tutorial/ws_xxx/s02')
    const wrapper = mount(SwitchWorkspaceMenu, {
      props: { workspaceRoot: 'D:/projects/some-repo' }
    })
    const chip = wrapper.find('[data-testid="workspace-chip"]')
    expect(chip.exists()).toBe(true)
    expect(chip.text()).toContain('some-repo')
    expect(chip.text()).not.toContain('D:/projects')
  })

  it('does NOT render on entry page (route name=index)', () => {
    setRouteName('index', '/')
    const wrapper = mount(SwitchWorkspaceMenu, {
      props: { workspaceRoot: 'D:/projects/some-repo' }
    })
    expect(wrapper.find('[data-testid="workspace-chip"]').exists()).toBe(false)
  })

  it('does NOT render on grant page (route name=workspace-grant)', () => {
    setRouteName('workspace-grant', '/workspace/grant')
    const wrapper = mount(SwitchWorkspaceMenu, {
      props: { workspaceRoot: 'D:/projects/some-repo' }
    })
    expect(wrapper.find('[data-testid="workspace-chip"]').exists()).toBe(false)
  })

  it('chip click toggles dropdown visibility', async () => {
    setRouteName('tutorial-workspace_id-station_id', '/tutorial/ws/s01')
    const wrapper = mount(SwitchWorkspaceMenu, {
      props: { workspaceRoot: 'D:/projects/some-repo' }
    })
    expect(
      wrapper.find('[data-testid="workspace-dropdown"]').exists()
    ).toBe(false)
    await wrapper.find('[data-testid="workspace-chip"]').trigger('click')
    await nextTick()
    expect(
      wrapper.find('[data-testid="workspace-dropdown"]').exists()
    ).toBe(true)
  })

  it('selecting "🔁 換資料夾" calls useIntervention().requestSwitchWorkspace()', async () => {
    setRouteName('tutorial-workspace_id-station_id', '/tutorial/ws/s01')
    const wrapper = mount(SwitchWorkspaceMenu, {
      props: { workspaceRoot: 'D:/projects/some-repo' }
    })
    await wrapper.find('[data-testid="workspace-chip"]').trigger('click')
    await nextTick()
    await wrapper
      .find('[data-testid="workspace-switch-action"]')
      .trigger('click')
    await nextTick()
    const action = useIntervention().pendingAction.value
    expect(action).not.toBeNull()
    expect(action?.kind).toBe('switch')
  })
})
