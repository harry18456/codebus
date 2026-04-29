import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import QaTurnCard from '~/components/qa/QaTurnCard.vue'
import type { QaTurn } from '~/composables/useQaSession'

function makeTurn(overrides: Partial<QaTurn> = {}): QaTurn {
  return {
    id: 'turn_default',
    question: 'why atomic write?',
    originatingStationId: 's03-production',
    taskId: 'qa_default01',
    ragHits: null,
    reactSteps: [],
    kbGrowth: [],
    answer: null,
    status: 'pending',
    ...overrides
  }
}

describe('QaTurnCard four-phase rendering', () => {
  it('renders all four phases when turn is complete', () => {
    const turn = makeTurn({
      status: 'done',
      ragHits: [
        {
          score: 0.71,
          file_path: 'src/storage/atomic.ts',
          line_start: 12,
          line_end: 38,
          snippet: 'temp + rename',
          related_stations: ['s03-production']
        },
        {
          score: 0.69,
          file_path: 'src/storage/index.ts',
          line_start: 4,
          line_end: 10,
          snippet: 're-export',
          related_stations: ['s03-production']
        }
      ],
      reactSteps: [
        {
          step: 0,
          thought: { text: 'check tests' },
          actions: [
            { tool: 'list_dir', observation: 'a.ts\nb.ts', tokens_used: 0, isError: false }
          ]
        }
      ],
      answer: {
        text: 'Atomic via temp + rename.',
        citations: [
          {
            file_path: 'src/storage/atomic.ts',
            line_start: 12,
            line_end: 38,
            related_stations: ['s03-production']
          },
          {
            file_path: 'tests/storage/atomic.test.ts',
            line_start: 1,
            line_end: 22,
            related_stations: ['s03-production', 's05-tests']
          }
        ]
      }
    })
    const wrapper = mount(QaTurnCard, { props: { turn } })
    const text = wrapper.text()
    // User message
    expect(text).toContain('why atomic write?')
    // RAG hits header
    expect(text).toContain('① RAG 探查')
    // ReAct steps header
    expect(text).toContain('② ReAct loop')
    // Answer text
    expect(text).toContain('Atomic via temp + rename.')
    // Citations rendered (file:line for both)
    expect(text).toContain('src/storage/atomic.ts:12-38')
    expect(text).toContain('tests/storage/atomic.test.ts:1-22')
    wrapper.unmount()
  })

  it('hides RAG section when ragHits is null', () => {
    const turn = makeTurn({
      status: 'done',
      ragHits: null,
      reactSteps: [
        {
          step: 0,
          thought: { text: 'thinking' },
          actions: []
        }
      ],
      answer: { text: 'answer here', citations: [] }
    })
    const wrapper = mount(QaTurnCard, { props: { turn } })
    expect(wrapper.text()).not.toContain('① RAG 探查')
    expect(wrapper.text()).toContain('② ReAct loop')
    expect(wrapper.text()).toContain('answer here')
    wrapper.unmount()
  })

  it('shows pulse badge when status is streaming', () => {
    const turn = makeTurn({ status: 'streaming' })
    const wrapper = mount(QaTurnCard, { props: { turn } })
    const badge = wrapper.find('[data-status="streaming"]')
    expect(badge.exists()).toBe(true)
    const cls = badge.attributes('class') ?? ''
    expect(cls).toMatch(/animate-pulse|pulse/)
    wrapper.unmount()
  })

  it('surfaces error message when status is error', () => {
    const turn = makeTurn({
      status: 'error',
      error: { code: 'QA_FAILED', message: 'QA_FAILED: budget exhausted' }
    })
    const wrapper = mount(QaTurnCard, { props: { turn } })
    expect(wrapper.text()).toContain('budget exhausted')
    wrapper.unmount()
  })

  it('omits rollback button in P0 even when kbGrowth has events', () => {
    const turn = makeTurn({
      status: 'streaming',
      reactSteps: [{ step: 0, actions: [] }],
      kbGrowth: [
        {
          entry_id: 'a14f9c2e',
          source: 'src/storage/atomic.ts:12-38',
          related_stations: ['s03-production'],
          originating_station_id: 's03-production'
        }
      ]
    })
    const wrapper = mount(QaTurnCard, { props: { turn } })
    const html = wrapper.html()
    expect(/rollback/i.test(html)).toBe(false)
    expect(html).not.toContain('↶')
    // Metadata still rendered
    expect(wrapper.text()).toContain('a14f9c2e')
    expect(wrapper.text()).toContain('src/storage/atomic.ts:12-38')
    expect(wrapper.text()).toContain('s03-production')
    wrapper.unmount()
  })
})
