import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'

const routerPushSpy = vi.fn()
const { useRouteMock } = vi.hoisted(() => ({ useRouteMock: vi.fn() }))
vi.mock('vue-router', async () => {
  const actual = await vi.importActual<typeof import('vue-router')>('vue-router')
  return {
    ...actual,
    useRoute: () => useRouteMock(),
    useRouter: () => ({ push: routerPushSpy })
  }
})

// Mock useTutorialFiles so we can assert no fs writes happen during switch.
const writeSpy = vi.fn()
const readSpy = vi.fn()
vi.mock('~/composables/useTutorialFiles', () => ({
  useTutorialFiles: () => ({
    readTutorialFile: readSpy,
    writeProgressFile: writeSpy,
    listTutorialTasks: vi.fn()
  })
}))

import InterventionConfirmModal from '~/components/intervention/InterventionConfirmModal.vue'
import SwitchWorkspaceMenu from '~/components/intervention/SwitchWorkspaceMenu.vue'
import {
  useIntervention,
  _resetForTest as resetIntervention
} from '~/composables/useIntervention'

beforeEach(() => {
  routerPushSpy.mockReset()
  writeSpy.mockReset()
  readSpy.mockReset()
  useRouteMock.mockReturnValue({
    name: 'tutorial-workspace_id-station_id',
    path: '/tutorial/ws_xxx/s02-mqtt-client',
    fullPath: '/tutorial/ws_xxx/s02-mqtt-client',
    query: {},
    params: {}
  })
  resetIntervention()
})

afterEach(() => {
  resetIntervention()
})

describe('switch workspace flow integration', () => {
  it('confirm modal copy mentions progress preservation, re-grant, and skip-grant on return', async () => {
    const menu = mount(SwitchWorkspaceMenu, {
      props: { workspaceRoot: 'D:/projects/some-repo' },
      attachTo: document.body
    })
    const modal = mount(InterventionConfirmModal, { attachTo: document.body })

    await menu.find('[data-testid="workspace-chip"]').trigger('click')
    await nextTick()
    await menu.find('[data-testid="workspace-switch-action"]').trigger('click')
    await nextTick()
    expect(useIntervention().pendingAction.value?.kind).toBe('switch')
    const text = modal.text()
    // (a) progress preserved
    expect(text).toContain('進度')
    // (b) re-grant required
    expect(text).toContain('grant')
    // (c) returning skips the grant
    expect(text).toContain('之前')
    menu.unmount()
    modal.unmount()
  })

  it('confirm push to entry route without filesystem writes nor audit-log appends', async () => {
    const menu = mount(SwitchWorkspaceMenu, {
      props: { workspaceRoot: 'D:/projects/some-repo' },
      attachTo: document.body
    })
    const modal = mount(InterventionConfirmModal, { attachTo: document.body })

    await menu.find('[data-testid="workspace-chip"]').trigger('click')
    await nextTick()
    await menu.find('[data-testid="workspace-switch-action"]').trigger('click')
    await nextTick()
    await modal.find('[data-testid="intervention-confirm"]').trigger('click')
    await nextTick()

    // Routed back to entry page
    expect(routerPushSpy).toHaveBeenCalledWith('/')
    // No fs side-effects: progress.json not written, no read, etc.
    expect(writeSpy).not.toHaveBeenCalled()
    expect(readSpy).not.toHaveBeenCalled()
    // No grant_revoked event would be emitted from the click — the
    // intervention layer never touches the authorization audit log.
    // (We don't have a direct hook into the audit log here, but its
    // single writer lives in the auth subpackage; the intervention
    // composable contains no reference to it.)
    expect(useIntervention().pendingAction.value).toBeNull()
    menu.unmount()
    modal.unmount()
  })

  it('cancel modal does NOT trigger router.push', async () => {
    const menu = mount(SwitchWorkspaceMenu, {
      props: { workspaceRoot: 'D:/projects/some-repo' },
      attachTo: document.body
    })
    const modal = mount(InterventionConfirmModal, { attachTo: document.body })

    await menu.find('[data-testid="workspace-chip"]').trigger('click')
    await nextTick()
    await menu.find('[data-testid="workspace-switch-action"]').trigger('click')
    await nextTick()
    await modal.find('[data-testid="intervention-cancel"]').trigger('click')
    await nextTick()

    expect(routerPushSpy).not.toHaveBeenCalled()
    expect(useIntervention().pendingAction.value).toBeNull()
    menu.unmount()
    modal.unmount()
  })
})
