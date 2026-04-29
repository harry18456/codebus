import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { ref, nextTick } from 'vue'
import ConsoleTimeline from '~/components/console/ConsoleTimeline.vue'
import type { StepBucket } from '~/composables/useExplorerStream'

function bucket(step: number, partial: Partial<StepBucket> = {}): StepBucket {
  return {
    step,
    actions: partial.actions ?? [],
    thought: partial.thought,
    judge: partial.judge
  }
}

describe('ConsoleTimeline', () => {
  it('renders cards in step ascending order regardless of arrival order', () => {
    const buckets = new Map<number, StepBucket>()
    buckets.set(2, bucket(2, { thought: { text: 't2', actions: [] } }))
    buckets.set(1, bucket(1, { thought: { text: 't1', actions: [] } }))
    buckets.set(3, bucket(3, { thought: { text: 't3', actions: [] } }))

    const wrapper = mount(ConsoleTimeline, { props: { stepBuckets: buckets } })
    const cards = wrapper.findAll('article[data-step]')
    expect(cards).toHaveLength(3)
    expect(cards[0]?.attributes('data-step')).toBe('1')
    expect(cards[1]?.attributes('data-step')).toBe('2')
    expect(cards[2]?.attributes('data-step')).toBe('3')
  })

  it('renders a waiting placeholder when stepBuckets is empty', () => {
    const wrapper = mount(ConsoleTimeline, {
      props: { stepBuckets: new Map() }
    })
    expect(wrapper.find('article[data-step]').exists()).toBe(false)
    const placeholder = wrapper.find('[data-testid="timeline-placeholder"]')
    expect(placeholder.exists()).toBe(true)
    expect(placeholder.text()).toContain('Explorer')
  })

  it('upserts late-arriving event in place via stable :key', async () => {
    const buckets = ref(new Map<number, StepBucket>())
    buckets.value.set(2, bucket(2, { thought: { text: 'open', actions: [] } }))
    buckets.value = new Map(buckets.value)

    const wrapper = mount(ConsoleTimeline, {
      props: { stepBuckets: buckets.value }
    })
    expect(wrapper.find('article[data-step]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="step-judge"]').exists()).toBe(false)

    // Late judge arrives — same step.
    const updated = new Map(buckets.value)
    updated.set(
      2,
      bucket(2, {
        thought: { text: 'open', actions: [] },
        judge: { relevance: 0.9, reason: 'good' }
      })
    )
    await wrapper.setProps({ stepBuckets: updated })
    await nextTick()

    const cards = wrapper.findAll('article[data-step]')
    expect(cards).toHaveLength(1)
    expect(cards[0]?.attributes('data-step')).toBe('2')
    // JUDGE section now shows on the same card.
    expect(wrapper.find('[data-testid="step-judge"]').exists()).toBe(true)
  })
})
