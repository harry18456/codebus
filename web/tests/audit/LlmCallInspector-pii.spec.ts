// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/llm-call-inspector/spec.md
//   Requirement: LlmCallInspector renders provider id and filters PII detection role

import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import LlmCallInspector from '~/components/audit/LlmCallInspector.vue'
import type { LlmCallEntry } from '~/composables/useAuditJsonl'

function mkEntry(role: string, idx: number, providerId = 'openai-default'): LlmCallEntry {
  return {
    timestamp: `2026-05-01T10:00:0${idx}.000Z`,
    request_id: `req_${idx}`,
    role: role as LlmCallEntry['role'],
    module: 'explorer',
    provider_id: providerId,
    model: 'gpt-4o-mini',
    call_type: 'chat',
    prompt_tokens: 100,
    completion_tokens: 50,
    cost_usd: 0.0001,
    latency_ms: 200,
    sanitizer_pass2_applied: false,
    request: { messages: [] },
    response: { choices: [] }
  }
}

const MIXED: LlmCallEntry[] = [
  mkEntry('reasoning', 0),
  mkEntry('chat', 1),
  mkEntry('pii_detection', 2),
  mkEntry('judge', 3),
  mkEntry('pii_detection', 4)
]

describe('<LlmCallInspector> PII filter', () => {
  it('renders provider_id chip with the literal id', () => {
    const wrapper = mount(LlmCallInspector, {
      props: { rows: [mkEntry('chat', 0)], activeIndex: 0 }
    })
    const chip = wrapper.get('[data-testid="llm-inspector-provider-id"]')
    expect(chip.text()).toBe('openai-default')
  })

  it('default hidePiiDetection clamps next nav at the last chat row', async () => {
    const wrapper = mount(LlmCallInspector, {
      props: { rows: MIXED, activeIndex: 3 } // judge row (index 3 in raw)
    })
    // Position label reads against visible rows = 3 chat-ish rows; at
    // activeIndex 3 (judge) we are at the last visible row.
    expect(wrapper.text()).toContain('3 / 3')
    await wrapper.find('button[data-action="next"]').trigger('click')
    const emitted = wrapper.emitted('select-index') ?? []
    // Clamping: nav stays at the same real index (3 = judge); does not
    // advance into pii_detection (real index 4).
    expect(emitted.at(-1)?.[0]).toBe(3)
  })

  it('shows toggle button with hidden count when PII rows exist', () => {
    const wrapper = mount(LlmCallInspector, {
      props: { rows: MIXED, activeIndex: 0 }
    })
    const toggle = wrapper.get('[data-testid="llm-inspector-toggle-pii"]')
    expect(toggle.text()).toContain('2')
    expect(toggle.text()).toMatch(/PII detection/i)
  })

  it('clicking toggle emits toggle-pii-visible', async () => {
    const wrapper = mount(LlmCallInspector, {
      props: { rows: MIXED, activeIndex: 0 }
    })
    await wrapper.get('[data-testid="llm-inspector-toggle-pii"]').trigger('click')
    expect(wrapper.emitted('toggle-pii-visible')).toBeTruthy()
  })
})
