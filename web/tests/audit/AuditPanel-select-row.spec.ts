import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import AuditPanel, {
  type AuditRow,
  type AuditTab
} from '~/components/audit/AuditPanel.vue'

const SAMPLE_ROWS: AuditRow[] = [
  { ts: '14:00:00', body: 'first row body' },
  { ts: '14:00:01', body: 'second row body' },
  { ts: '14:00:02', body: 'third row body' }
]

describe('AuditPanel select-row emit', () => {
  it('emits select-row with the clicked row index (zero-based)', async () => {
    const wrapper = mount(AuditPanel, {
      props: { activeTab: 'llm' as AuditTab, rows: SAMPLE_ROWS }
    })
    const rowEls = wrapper.findAll('[data-testid="audit-row"]')
    expect(rowEls).toHaveLength(SAMPLE_ROWS.length)
    await rowEls[2]!.trigger('click')
    const events = wrapper.emitted('select-row') ?? []
    expect(events).toHaveLength(1)
    expect(events[0]).toEqual([2])
  })

  it('fires for every tab uniformly', async () => {
    for (const tab of ['sanitize', 'tool', 'reasoning', 'token', 'llm', 'kb_growth', 'generator'] as AuditTab[]) {
      const wrapper = mount(AuditPanel, {
        props: { activeTab: tab, rows: SAMPLE_ROWS }
      })
      const rowEls = wrapper.findAll('[data-testid="audit-row"]')
      await rowEls[0]!.trigger('click')
      const events = wrapper.emitted('select-row') ?? []
      expect(events.length).toBeGreaterThanOrEqual(1)
      expect(events[0]).toEqual([0])
      wrapper.unmount()
    }
  })

  it('does not throw when parent has no listener for select-row', async () => {
    const wrapper = mount(AuditPanel, {
      props: { activeTab: 'tool' as AuditTab, rows: SAMPLE_ROWS }
    })
    const rowEls = wrapper.findAll('[data-testid="audit-row"]')
    await expect(rowEls[0]!.trigger('click')).resolves.toBeUndefined()
    wrapper.unmount()
  })

  it('does not internally render any inspector / drawer / modal', () => {
    const wrapper = mount(AuditPanel, {
      props: { activeTab: 'llm' as AuditTab, rows: SAMPLE_ROWS }
    })
    const html = wrapper.html().toLowerCase()
    expect(html).not.toContain('class="inspector')
    expect(html).not.toContain('class="drawer')
    expect(html).not.toContain('class="modal')
    expect(html).not.toContain('class="overlay')
    wrapper.unmount()
  })

  // Regression: existing 3 scenarios from the original frontend-shell spec.
  it('regression: 7 tabs render in canonical order', () => {
    const wrapper = mount(AuditPanel, {
      props: { activeTab: 'sanitize' as AuditTab, rows: [] }
    })
    const tabs = wrapper.findAll('button[data-tab]')
    expect(tabs).toHaveLength(7)
    expect(tabs.map((t) => t.attributes('data-tab'))).toEqual([
      'sanitize',
      'tool',
      'reasoning',
      'token',
      'llm',
      'kb_growth',
      'generator'
    ])
    wrapper.unmount()
  })

  it('regression: empty rows renders empty-state placeholder', () => {
    const wrapper = mount(AuditPanel, {
      props: { activeTab: 'reasoning' as AuditTab, rows: [] }
    })
    expect(wrapper.find('[data-empty="true"]').exists()).toBe(true)
    wrapper.unmount()
  })
})
