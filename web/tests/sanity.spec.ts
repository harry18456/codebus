import { describe, expect, it } from 'vitest'
import { getOpenedEventSources, lastEventSource } from './setup'

describe('vitest infra sanity', () => {
  it('runs with happy-dom + globals', () => {
    expect(1 + 1).toBe(2)
    expect(typeof globalThis.document).toBe('object')
  })

  it('FakeEventSource is registered globally and tracked', () => {
    expect(getOpenedEventSources()).toHaveLength(0)
    const es = new EventSource('http://127.0.0.1:0/test')
    expect(getOpenedEventSources()).toHaveLength(1)
    expect(lastEventSource()).toBe(es)

    let opened = false
    es.onopen = () => {
      opened = true
    }
    ;(es as unknown as { _simulateOpen: () => void })._simulateOpen()
    expect(opened).toBe(true)
  })

  it('FakeEventSource _emit dispatches to addEventListener-registered handlers', () => {
    const es = new EventSource('http://127.0.0.1:0/test')
    let received: string | null = null
    es.addEventListener('agent_thought', (event) => {
      received = (event as MessageEvent<string>).data
    })
    ;(
      es as unknown as { _emit: (type: string, data: unknown) => void }
    )._emit('agent_thought', { step: 1, thought: 'hello' })
    expect(received).toBe(JSON.stringify({ step: 1, thought: 'hello' }))
  })
})
