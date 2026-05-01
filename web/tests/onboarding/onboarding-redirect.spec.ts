// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-onboarding/spec.md
//   Requirement: Startup detection redirects to onboarding when any LLM dependency is not configured

import { describe, expect, it, vi } from 'vitest'

import {
  decideOnboardingRedirect,
  type HealthzResponse
} from '~/middleware/onboarding-redirect.global'

function fixedFetcher(value: HealthzResponse | null) {
  return () => Promise.resolve(value)
}

describe('decideOnboardingRedirect', () => {
  it('redirects to /onboarding/welcome when llm_chat is not-configured', async () => {
    const target = await decideOnboardingRedirect(
      '/',
      fixedFetcher({
        status: 'ok',
        dependency: { llm_chat: 'not-configured', llm_embed: 'ready' }
      })
    )
    expect(target).toBe('/onboarding/welcome')
  })

  it('redirects when navigating to a tutorial URL with not-configured llm_embed', async () => {
    const target = await decideOnboardingRedirect(
      '/tutorial/ws_xxx/s02-mqtt-client',
      fixedFetcher({
        status: 'ok',
        dependency: { llm_chat: 'ready', llm_embed: 'not-configured' }
      })
    )
    expect(target).toBe('/onboarding/welcome')
  })

  it('does NOT redirect when already on an /onboarding/* route', async () => {
    const fetchSpy = vi.fn(() =>
      Promise.resolve({
        status: 'ok',
        dependency: { llm_chat: 'not-configured', llm_embed: 'not-configured' }
      } as HealthzResponse)
    )
    const target = await decideOnboardingRedirect(
      '/onboarding/welcome',
      fetchSpy
    )
    expect(target).toBeNull()
    expect(fetchSpy).not.toHaveBeenCalled()
  })

  it('does NOT redirect when both lanes are ready', async () => {
    const target = await decideOnboardingRedirect(
      '/',
      fixedFetcher({
        status: 'ok',
        dependency: { llm_chat: 'ready', llm_embed: 'ready' }
      })
    )
    expect(target).toBeNull()
  })

  it('returns null gracefully when healthz is unreachable', async () => {
    const target = await decideOnboardingRedirect('/', fixedFetcher(null))
    expect(target).toBeNull()
  })
})
