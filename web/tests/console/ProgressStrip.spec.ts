import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import ProgressStrip from '~/components/console/ProgressStrip.vue'

describe('ProgressStrip', () => {
  it('renders 5 cells when total is 5 with correct done/now/queued states', () => {
    const wrapper = mount(ProgressStrip, {
      props: { progress: { current: 4, total: 5 } }
    })

    const cells = wrapper.findAll('[data-state]')
    expect(cells).toHaveLength(5)

    // Cells 1-3 (index 0,1,2) are done (index < current - 1 = 3)
    expect(cells[0]!.attributes('data-state')).toBe('done')
    expect(cells[1]!.attributes('data-state')).toBe('done')
    expect(cells[2]!.attributes('data-state')).toBe('done')
    // Cell 4 (index 3) is in progress (index === current - 1)
    expect(cells[3]!.attributes('data-state')).toBe('now')
    // Cell 5 (index 4) is queued
    expect(cells[4]!.attributes('data-state')).toBe('queued')

    expect(wrapper.text()).toContain('step 4 / 5')
  })

  it('renders placeholder when progress is null', () => {
    const wrapper = mount(ProgressStrip, {
      props: { progress: null }
    })

    expect(wrapper.text()).toContain('step — / —')
    const cells = wrapper.findAll('[data-state]')
    expect(cells.length).toBeLessThanOrEqual(1)
  })

  it('does not render any "stations" counter text', () => {
    const wrapperWithProgress = mount(ProgressStrip, {
      props: { progress: { current: 2, total: 3 } }
    })
    expect(wrapperWithProgress.text()).not.toMatch(/\d+\s*stations?/i)

    const wrapperNull = mount(ProgressStrip, { props: { progress: null } })
    expect(wrapperNull.text()).not.toMatch(/\d+\s*stations?/i)
  })
})
