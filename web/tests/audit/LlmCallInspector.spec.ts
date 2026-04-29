import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import LlmCallInspector from '~/components/audit/LlmCallInspector.vue'
import type { LlmCallEntry } from '~/composables/useAuditJsonl'
import fixture from './fixtures/llm-calls.json'

const ROWS = fixture as LlmCallEntry[]

function mountInspector(activeIndex: number | null) {
  return mount(LlmCallInspector, {
    props: { rows: ROWS, activeIndex },
    attachTo: document.body
  })
}

describe('LlmCallInspector', () => {
  it('renders nothing when activeIndex is null', () => {
    const wrapper = mountInspector(null)
    expect(wrapper.find('aside').exists()).toBe(false)
    wrapper.unmount()
  })

  it('renders four tabs in canonical order: wire / response / tokens / timeline', () => {
    const wrapper = mountInspector(0)
    const tabs = wrapper.findAll('button[data-tab]')
    expect(tabs).toHaveLength(4)
    expect(tabs.map((t) => t.attributes('data-tab'))).toEqual([
      'wire',
      'response',
      'tokens',
      'timeline'
    ])
    wrapper.unmount()
  })

  it('prev/next emits clamped index at boundaries', async () => {
    const wrapper = mountInspector(0)
    await wrapper.find('button[data-action="prev"]').trigger('click')
    let emits = wrapper.emitted('select-index') ?? []
    expect(emits[0]).toEqual([0])

    await wrapper.setProps({ rows: ROWS, activeIndex: ROWS.length - 1 })
    await wrapper.find('button[data-action="next"]').trigger('click')
    emits = wrapper.emitted('select-index') ?? []
    expect(emits.at(-1)).toEqual([ROWS.length - 1])
    wrapper.unmount()
  })

  it('shows Pass 2 sanitize ON banner with D-015 reference when sanitizer_pass2_applied is true', () => {
    // fixture[0] has sanitizer_pass2_applied: true.
    const wrapper = mountInspector(0)
    expect(wrapper.text()).toContain('Pass 2 sanitize ON')
    expect(wrapper.text()).toContain('D-015')
    wrapper.unmount()
  })

  it('renders em-dash placeholder when cost_usd is null', async () => {
    // fixture[2] has cost_usd: null.
    const wrapper = mountInspector(2)
    // Switch to tokens tab.
    await wrapper.find('button[data-tab="tokens"]').trigger('click')
    const costCell = wrapper.find('[data-testid="cost-cell"]')
    expect(costCell.exists()).toBe(true)
    expect(costCell.text()).toContain('—')
    expect(costCell.text()).not.toContain('$0')
    expect(costCell.text()).not.toContain('null')
    wrapper.unmount()
  })

  it('Escape keydown on window emits close', async () => {
    const wrapper = mountInspector(0)
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await nextTick()
    expect(wrapper.emitted('close')).toBeTruthy()
    wrapper.unmount()
  })

  it('close button emits close', async () => {
    const wrapper = mountInspector(0)
    await wrapper.find('button[data-action="close"]').trigger('click')
    expect(wrapper.emitted('close')).toBeTruthy()
    wrapper.unmount()
  })

  it('switching to response tab renders response payload (pretty JSON)', async () => {
    const wrapper = mountInspector(0)
    await wrapper.find('button[data-tab="response"]').trigger('click')
    // fixture[0].response.choices[0].message.content snippet
    expect(wrapper.text()).toContain('list src/storage first')
    wrapper.unmount()
  })

  it('response tab shows error message when response is null and error is set', async () => {
    // fixture[4] has response: null + error.class TimeoutError
    const wrapper = mountInspector(4)
    await wrapper.find('button[data-tab="response"]').trigger('click')
    expect(wrapper.text()).toContain('no response')
    expect(wrapper.text()).toContain('request exceeded 10s')
    wrapper.unmount()
  })

  it('header shows N / total based on rows.length', () => {
    const wrapper = mountInspector(2)
    // Position string "3 / 6"
    expect(wrapper.text()).toMatch(/3\s*\/\s*6/)
    wrapper.unmount()
  })

  it('non-prev/next click of a middle index emits the clicked index unchanged', async () => {
    const wrapper = mountInspector(2)
    await wrapper.find('button[data-action="next"]').trigger('click')
    expect(wrapper.emitted('select-index')?.[0]).toEqual([3])

    await wrapper.find('button[data-action="prev"]').trigger('click')
    expect(wrapper.emitted('select-index')?.at(-1)).toEqual([1])
    wrapper.unmount()
  })
})
