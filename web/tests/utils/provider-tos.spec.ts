// Backs SHALL clauses in
// openspec/changes/phase7-onboarding-polish/specs/provider-onboarding/spec.md
//   Scenario: Providers page renders contextual ToS link per type

import { describe, expect, it } from 'vitest'

import { PROVIDER_TYPE_TOS_URL, getTosUrl } from '~/utils/provider-tos'

describe('getTosUrl (per-app PROVIDER_TYPE_TOS_URL constant)', () => {
  it('openai_chat resolves to OpenAI ToS URL', () => {
    expect(getTosUrl('openai_chat')).toBe(
      'https://openai.com/policies/terms-of-use/'
    )
  })

  it('openai_embedding resolves to OpenAI ToS URL', () => {
    expect(getTosUrl('openai_embedding')).toBe(
      'https://openai.com/policies/terms-of-use/'
    )
  })

  it('unknown future type returns null (no broken or default-OpenAI fallback)', () => {
    expect(getTosUrl('unknown_future' as never)).toBeNull()
  })

  it('PROVIDER_TYPE_TOS_URL contains exactly the P0 entries', () => {
    expect(Object.keys(PROVIDER_TYPE_TOS_URL).sort()).toEqual([
      'openai_chat',
      'openai_embedding'
    ])
  })
})
