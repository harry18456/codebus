import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import QaCitations from '~/components/qa/QaCitations.vue'
import type { Citation } from '~/composables/useQaSession'

describe('QaCitations', () => {
  it('renders nothing when citations is empty', () => {
    const wrapper = mount(QaCitations, { props: { citations: [] } })
    expect(wrapper.findAll('[data-testid="citation-row"]')).toHaveLength(0)
    expect(wrapper.findAll('[data-station-id]')).toHaveLength(0)
    wrapper.unmount()
  })

  it('emits navigate-to-station with station id when a chip is clicked', async () => {
    const citations: Citation[] = [
      {
        file_path: 'src/storage/atomic.ts',
        line_start: 12,
        line_end: 38,
        related_stations: ['s03-production']
      }
    ]
    const wrapper = mount(QaCitations, { props: { citations } })
    const chip = wrapper.find('[data-station-id="s03-production"]')
    expect(chip.exists()).toBe(true)
    await chip.trigger('click')
    const events = wrapper.emitted('navigate-to-station') ?? []
    expect(events).toHaveLength(1)
    expect(events[0]).toEqual(['s03-production'])
    wrapper.unmount()
  })

  it('renders file:line as plain text (NOT wrapped in <a> or <button>)', () => {
    const citations: Citation[] = [
      {
        file_path: 'tests/storage/atomic.test.ts',
        line_start: 1,
        line_end: 22,
        related_stations: ['s05-tests']
      }
    ]
    const wrapper = mount(QaCitations, { props: { citations } })
    const fileLineNode = wrapper
      .findAll('[data-testid="citation-file-line"]')[0]
    expect(fileLineNode).toBeDefined()
    expect(fileLineNode!.text()).toContain('tests/storage/atomic.test.ts:1-22')
    // Walk up the parent chain checking neither <a> nor <button> wraps it.
    let el: HTMLElement | null = fileLineNode!.element as HTMLElement
    while (el) {
      const tag = el.tagName.toUpperCase()
      expect(tag).not.toBe('A')
      expect(tag).not.toBe('BUTTON')
      el = el.parentElement
    }
    wrapper.unmount()
  })
})
