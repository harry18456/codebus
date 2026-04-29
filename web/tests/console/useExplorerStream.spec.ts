import { describe, expect, it, vi } from 'vitest'
import { ref, nextTick } from 'vue'
import { getOpenedEventSources, lastEventSource } from '../setup'

// Stub useSidecar so the inner useSseTask sees ready=true with a valid bearer
// and base URL — without booting the real Tauri IPC handshake. Mock the alias
// path; vitest.config.ts maps `~` to `app/` so this matches the relative
// `./useSidecar` import from useSseTask too (vitest dedupes by resolved path).
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: globalThis.fetch
  })
}))

import { useExplorerStream } from '~/composables/useExplorerStream'

const TASK_ID = 'explore_4f2a8b91'

describe('useExplorerStream', () => {
  it('opens exactly one EventSource (single dispatch entry)', async () => {
    expect(getOpenedEventSources()).toHaveLength(0)
    const stream = useExplorerStream(TASK_ID)
    await nextTick()
    expect(getOpenedEventSources()).toHaveLength(1)
    // Touching every reactive surface MUST NOT open a second connection.
    void stream.stepBuckets.value
    void stream.progress.value
    void stream.coverageBanner.value
    void stream.budgetBanner.value
    void stream.auditRows.value
    expect(getOpenedEventSources()).toHaveLength(1)
    stream.close()
  })

  it('agent_thought upserts thought while preserving prior actions', async () => {
    const stream = useExplorerStream(TASK_ID)
    await nextTick()
    const es = lastEventSource()

    es._emit('agent_action_result', {
      step: 3,
      tool: 'read_file',
      observation: 'first',
      tokens_used: 0
    })
    es._emit('agent_thought', {
      step: 3,
      thought: 'open b.py',
      action: [{ tool: 'read_file', args: { path: 'src/b.py' } }]
    })
    es._emit('agent_action_result', {
      step: 3,
      tool: 'trace_import',
      observation: 'second',
      tokens_used: 0
    })
    await nextTick()

    const bucket = stream.stepBuckets.value.get(3)
    expect(bucket).toBeDefined()
    expect(bucket?.thought?.text).toBe('open b.py')
    expect(bucket?.thought?.actions).toHaveLength(1)
    expect(bucket?.actions).toHaveLength(2)
    expect(bucket?.actions[0]?.observation).toBe('first')
    expect(bucket?.actions[1]?.observation).toBe('second')
    stream.close()
  })

  it('flags isError via observation heuristic (error: prefix or traceback)', async () => {
    const stream = useExplorerStream(TASK_ID)
    await nextTick()
    const es = lastEventSource()

    es._emit('agent_action_result', {
      step: 1,
      tool: 't',
      observation: 'error: not found',
      tokens_used: 0
    })
    es._emit('agent_action_result', {
      step: 1,
      tool: 't',
      observation: 'Traceback (most recent call last)',
      tokens_used: 0
    })
    es._emit('agent_action_result', {
      step: 1,
      tool: 't',
      observation: 'fine result',
      tokens_used: 0
    })
    await nextTick()

    const actions = stream.stepBuckets.value.get(1)?.actions ?? []
    expect(actions.map((a) => a.isError)).toEqual([true, true, false])
    stream.close()
  })

  it('progress overwrite is monotonic-friendly (no history retained)', async () => {
    const stream = useExplorerStream(TASK_ID)
    await nextTick()
    const es = lastEventSource()
    es._emit('progress', { phase: 'exploring', current: 2, total: 5 })
    es._emit('progress', { phase: 'exploring', current: 3, total: 5 })
    await nextTick()
    expect(stream.progress.value).toEqual({ current: 3, total: 5 })
    stream.close()
  })

  it('progress with non-exploring phase is ignored', async () => {
    const stream = useExplorerStream(TASK_ID)
    await nextTick()
    const es = lastEventSource()
    es._emit('progress', { phase: 'scanning', current: 10, total: 50 })
    await nextTick()
    expect(stream.progress.value).toBeNull()
    stream.close()
  })

  it('coverage_gaps is latest-only (overwrite semantics)', async () => {
    const stream = useExplorerStream(TASK_ID)
    await nextTick()
    const es = lastEventSource()
    es._emit('coverage_gaps', {
      round: 0,
      gaps: [{ description: 'g1', suggested_target: null }, { description: 'g2', suggested_target: null }],
      will_recurse: true,
      skip_reason: null
    })
    es._emit('coverage_gaps', {
      round: 1,
      gaps: [{ description: 'g3', suggested_target: 'x' }],
      will_recurse: false,
      skip_reason: 'no_gaps'
    })
    await nextTick()
    expect(stream.coverageBanner.value?.round).toBe(1)
    expect(stream.coverageBanner.value?.gaps).toHaveLength(1)
    stream.close()
  })

  it('budget_warning latches per kind (steps and tokens preserved separately)', async () => {
    const stream = useExplorerStream(TASK_ID)
    await nextTick()
    const es = lastEventSource()
    es._emit('budget_warning', { kind: 'tokens', current: 80, budget: 100, pct: 0.8 })
    es._emit('budget_warning', { kind: 'steps', current: 8, budget: 10, pct: 0.8 })
    await nextTick()
    expect(stream.budgetBanner.value.tokens?.current).toBe(80)
    expect(stream.budgetBanner.value.steps?.current).toBe(8)
    stream.close()
  })

  it('auditRows rolling window caps at 200 entries (FIFO eviction)', async () => {
    const stream = useExplorerStream(TASK_ID)
    await nextTick()
    const es = lastEventSource()
    for (let i = 0; i < 250; i += 1) {
      es._emit('agent_thought', { step: i, thought: `t${i}`, action: [] })
    }
    await nextTick()
    expect(stream.auditRows.value).toHaveLength(200)
    // Oldest 50 evicted: first remaining row corresponds to step 50.
    expect(stream.auditRows.value[0]?.body).toContain('t50')
    expect(stream.auditRows.value[199]?.body).toContain('t249')
    stream.close()
  })

  it('done event flips done flag exactly once', async () => {
    const stream = useExplorerStream(TASK_ID)
    await nextTick()
    const es = lastEventSource()
    expect(stream.done.value).toBe(false)
    es._emit('done', { task_id: TASK_ID })
    await nextTick()
    expect(stream.done.value).toBe(true)
    // Second done should not toggle or re-fire.
    es._emit('done', { task_id: TASK_ID })
    await nextTick()
    expect(stream.done.value).toBe(true)
    stream.close()
  })
})
