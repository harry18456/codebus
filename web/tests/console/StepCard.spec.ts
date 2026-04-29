import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import StepCard from '~/components/console/StepCard.vue'
import type { StepBucket } from '~/composables/useExplorerStream'

// StepCard renders the THINK / ACT / JUDGE three-beat for a single step.
// Spec: openspec/changes/agent-console-p0/specs/agent-console/spec.md
//   "StepCard renders ReAct three beats in arrival order"

function makeBucket(overrides: Partial<StepBucket> = {}): StepBucket {
  return {
    step: 1,
    actions: [],
    ...overrides
  }
}

describe('StepCard', () => {
  it('renders THINK + ACT + JUDGE in fixed visual order when bucket is complete', () => {
    const bucket = makeBucket({
      step: 3,
      thought: {
        text: 'inspect storage adapter contracts',
        actions: [{ tool: 'list_dir', args: { path: '/src/storage' } }]
      },
      actions: [
        {
          tool: 'list_dir',
          observation: '8 entries',
          tokens_used: 412,
          isError: false
        }
      ],
      judge: {
        relevance: 0.8,
        reason: 'types.ts is the interface root'
      }
    })

    const wrapper = mount(StepCard, { props: { bucket } })
    const html = wrapper.html()

    expect(wrapper.find('[data-testid="step-think"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="step-act"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="step-judge"]').exists()).toBe(true)

    // Fixed visual order: THINK before ACT before JUDGE.
    const thinkIdx = html.indexOf('data-testid="step-think"')
    const actIdx = html.indexOf('data-testid="step-act"')
    const judgeIdx = html.indexOf('data-testid="step-judge"')
    expect(thinkIdx).toBeGreaterThan(-1)
    expect(actIdx).toBeGreaterThan(thinkIdx)
    expect(judgeIdx).toBeGreaterThan(actIdx)

    expect(wrapper.text()).toContain('inspect storage adapter contracts')
    expect(wrapper.text()).toContain('list_dir')
    expect(wrapper.text()).toContain('8 entries')
    expect(wrapper.text()).toContain('412 tokens')
    // Judge relevance formatted to two decimals.
    expect(wrapper.text()).toContain('0.80')
    expect(wrapper.text()).toContain('types.ts is the interface root')
  })

  it('hides JUDGE section when bucket has no judge field', () => {
    const bucket = makeBucket({
      step: 4,
      thought: { text: 'still thinking', actions: [] },
      actions: [
        {
          tool: 'read_file',
          observation: 'hello',
          tokens_used: 100,
          isError: false
        }
      ]
    })

    const wrapper = mount(StepCard, { props: { bucket } })

    expect(wrapper.find('[data-testid="step-think"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="step-act"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="step-judge"]').exists()).toBe(false)
  })

  it('hides THINK and ACT sections when their fields are absent', () => {
    const bucket = makeBucket({
      step: 5,
      judge: { relevance: 0.42, reason: 'late judge only' }
    })

    const wrapper = mount(StepCard, { props: { bucket } })

    expect(wrapper.find('[data-testid="step-think"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="step-act"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="step-judge"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('0.42')
  })

  it('renders failed action with data-state="error" but does not hide it', () => {
    const bucket = makeBucket({
      step: 6,
      actions: [
        {
          tool: 'read_file',
          observation: 'ok one',
          tokens_used: 50,
          isError: false
        },
        {
          tool: 'read_file',
          observation: 'error: file not found',
          tokens_used: 12,
          isError: true
        }
      ]
    })

    const wrapper = mount(StepCard, { props: { bucket } })

    const rows = wrapper.findAll('[data-testid="action-row"]')
    expect(rows).toHaveLength(2)

    const errorRow = wrapper.find('[data-state="error"]')
    expect(errorRow.exists()).toBe(true)
    expect(errorRow.text()).toContain('error: file not found')
  })

  it('renders em dash placeholder when tokens_used === 0', () => {
    const bucket = makeBucket({
      step: 7,
      actions: [
        {
          tool: 'list_dir',
          observation: 'placeholder observation',
          tokens_used: 0,
          isError: false
        }
      ]
    })

    const wrapper = mount(StepCard, { props: { bucket } })
    const txt = wrapper.text()

    expect(txt).toContain('—')
    expect(txt).not.toContain('0 tokens')
    expect(txt).not.toContain('$0')
  })

  it('renders a "..." indicator when observation length === 500', () => {
    const longObs = 'a'.repeat(500)
    const bucket = makeBucket({
      step: 8,
      actions: [
        {
          tool: 'read_file',
          observation: longObs,
          tokens_used: 99,
          isError: false
        }
      ]
    })

    const wrapper = mount(StepCard, { props: { bucket } })
    const truncated = wrapper.find('[data-testid="obs-truncated"]')
    expect(truncated.exists()).toBe(true)
    expect(truncated.text()).toContain('…')
  })

  it('does not render the truncation indicator for short observations', () => {
    const bucket = makeBucket({
      step: 9,
      actions: [
        {
          tool: 'read_file',
          observation: 'short obs',
          tokens_used: 10,
          isError: false
        }
      ]
    })

    const wrapper = mount(StepCard, { props: { bucket } })
    expect(wrapper.find('[data-testid="obs-truncated"]').exists()).toBe(false)
  })
})
