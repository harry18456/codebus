// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-onboarding/spec.md
//   Scenario: Welcome page next button always enabled

import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

const pushSpy = vi.fn()
vi.mock('vue-router', () => ({
  useRouter: () => ({ push: pushSpy })
}))

import Welcome from '~/pages/onboarding/welcome.vue'

describe('/onboarding/welcome', () => {
  it('renders intro copy + ToS link + always-enabled Next', () => {
    const wrapper = mount(Welcome)
    expect(wrapper.find('[data-testid="onboarding-welcome"]').exists()).toBe(true)
    const tos = wrapper.get('[data-testid="onboarding-welcome-tos-link"]')
    expect(tos.attributes('href')).toMatch(/openai\.com/)
    const next = wrapper.get('[data-testid="onboarding-welcome-next"]')
    expect((next.element as HTMLButtonElement).disabled).toBe(false)
  })

  it('Next routes to /onboarding/providers', async () => {
    pushSpy.mockClear()
    const wrapper = mount(Welcome)
    await wrapper.get('[data-testid="onboarding-welcome-next"]').trigger('click')
    expect(pushSpy).toHaveBeenCalledWith('/onboarding/providers')
  })
})
