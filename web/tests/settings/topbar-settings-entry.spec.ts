// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/frontend-shell/spec.md
//   Requirement: TopBar exposes a settings entry routed to /settings

import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

const pushSpy = vi.fn()
vi.mock('vue-router', () => ({
  useRoute: () => ({ query: {} }),
  useRouter: () => ({ push: pushSpy })
}))

import TopBar from '~/components/layout/TopBar.vue'

describe('<TopBar> settings entry', () => {
  it('renders the topbar-settings button on tutorial-level pages', () => {
    const wrapper = mount(TopBar, {
      props: { workspace: 'demo', kill: 'READY' as const }
    })
    expect(wrapper.find('[data-testid="topbar-settings"]').exists()).toBe(true)
  })

  it('clicking the button routes via router.push("/settings")', async () => {
    pushSpy.mockClear()
    const wrapper = mount(TopBar, {
      props: { workspace: 'demo', kill: 'READY' as const }
    })
    await wrapper.get('[data-testid="topbar-settings"]').trigger('click')
    expect(pushSpy).toHaveBeenCalledWith('/settings')
  })
})
