// Backs SHALL clauses in
// openspec/changes/phase7-onboarding-polish/specs/provider-onboarding/spec.md
//   Scenario: Welcome page next button always enabled
//   Scenario: Welcome page contains no provider-specific ToS link

import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

const pushSpy = vi.fn()
vi.mock('vue-router', () => ({
  useRouter: () => ({ push: pushSpy })
}))

import Welcome from '~/pages/onboarding/welcome.vue'

describe('/onboarding/welcome', () => {
  it('renders intro copy + always-enabled Next', () => {
    const wrapper = mount(Welcome)
    expect(wrapper.find('[data-testid="onboarding-welcome"]').exists()).toBe(true)
    const next = wrapper.get('[data-testid="onboarding-welcome-next"]')
    expect((next.element as HTMLButtonElement).disabled).toBe(false)
  })

  it('Next routes to /onboarding/providers', async () => {
    pushSpy.mockClear()
    const wrapper = mount(Welcome)
    await wrapper.get('[data-testid="onboarding-welcome-next"]').trigger('click')
    expect(pushSpy).toHaveBeenCalledWith('/onboarding/providers')
  })

  it('contains no provider-specific ToS link', () => {
    const wrapper = mount(Welcome)
    // Scenario: Welcome page contains no provider-specific ToS link
    // (1) DOM MUST NOT contain any anchor whose href resolves to a
    //     provider's terms-of-service URL.
    const anchors = wrapper.findAll('a')
    for (const a of anchors) {
      const href = a.attributes('href') ?? ''
      expect(href).not.toMatch(/openai\.com|anthropic\.com/i)
    }
    // (2) Legal-acknowledgement copy MUST be phrased provider-agnostically.
    expect(wrapper.text()).not.toMatch(/OpenAI/i)
    expect(wrapper.text()).not.toMatch(/Anthropic/i)
  })
})
