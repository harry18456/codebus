// Backs SHALL clauses in
// openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//   Requirement: Entry page exposes folder-picker workspace onramp
//   Requirement: Workspace onramp drives scan, kb-build, explore, then generate via SSE

import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import OnrampProgress from '~/components/workspace-onramp/OnrampProgress.vue'
import type { SseEvent } from '~/composables/useSseTask'

function progress(data: Record<string, unknown>): SseEvent {
  return { type: 'progress', data }
}

function thought(step: number): SseEvent {
  return { type: 'agent_thought', data: { step } }
}

describe('<OnrampProgress>', () => {
  it('renders 「掃描中」 label and current counter when phase=scanning', () => {
    const wrapper = mount(OnrampProgress, {
      props: {
        phase: 'scanning',
        events: [progress({ current: 42, total: 120, phase: 'scanning' })]
      }
    })
    expect(wrapper.text()).toContain('掃描中')
    expect(wrapper.text()).toContain('42')
    expect(wrapper.text()).toContain('120')
  })

  it('renders 「建立索引中」 label and counter when phase=indexing', () => {
    const wrapper = mount(OnrampProgress, {
      props: {
        phase: 'indexing',
        events: [progress({ current: 30, total: 120, phase: 'embedding' })]
      }
    })
    expect(wrapper.text()).toContain('建立索引中')
    expect(wrapper.text()).toContain('30')
  })

  it('renders 「探索中」 label and step counter when phase=exploring', () => {
    const wrapper = mount(OnrampProgress, {
      props: {
        phase: 'exploring',
        events: [thought(3)]
      }
    })
    expect(wrapper.text()).toContain('探索中')
    expect(wrapper.text()).toContain('step 3')
  })

  it('renders 「產生教學中」 label and counter when phase=generating', () => {
    const wrapper = mount(OnrampProgress, {
      props: {
        phase: 'generating',
        events: [progress({ current: 2, total: 5, phase: 'generating' })]
      }
    })
    expect(wrapper.text()).toContain('產生教學中')
    expect(wrapper.text()).toContain('2')
    expect(wrapper.text()).toContain('5')
  })

  it('shows current_file when a progress event includes one', () => {
    const wrapper = mount(OnrampProgress, {
      props: {
        phase: 'scanning',
        events: [
          progress({
            current: 10,
            total: 100,
            phase: 'scanning',
            current_file: 'src/lib.rs'
          })
        ]
      }
    })
    expect(wrapper.find('[data-testid="onramp-progress-file"]').text()).toBe(
      'src/lib.rs'
    )
  })
})
