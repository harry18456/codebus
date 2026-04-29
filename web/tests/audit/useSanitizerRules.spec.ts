import { describe, expect, it, vi, beforeEach } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import fixture from './fixtures/sanitizer-rules.json'

// Mock useSidecar so the composable's `useSidecar().fetch('/sanitizer/rules')`
// resolves through a controllable spy. Tests reset the mock + composable
// module-level cache between cases so "fetched once per session" is testable
// in isolation.

const fetchMock = vi.fn()
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    fetch: (...args: unknown[]) => fetchMock(...args)
  })
}))

beforeEach(async () => {
  fetchMock.mockReset()
  // Reset module so module-level cache `Ref<SanitizerRulesSnapshot | null>`
  // restarts at null between tests. vi.resetModules wires this through the
  // dynamic import below.
  vi.resetModules()
})

async function importComposable() {
  return await import('~/composables/useSanitizerRules')
}

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { 'Content-Type': 'application/json' }
  })
}

describe('useSanitizerRules', () => {
  it('rules fetched once per session (module-level cache)', async () => {
    fetchMock.mockResolvedValue(jsonResponse(fixture))
    const { useSanitizerRules } = await importComposable()

    const first = useSanitizerRules()
    await first.loadOnce()

    const second = useSanitizerRules()
    await second.loadOnce()

    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(fetchMock).toHaveBeenCalledWith('/sanitizer/rules')
    expect(first.snapshot.value).not.toBeNull()
    expect(second.snapshot.value).toBe(first.snapshot.value)
    expect(first.snapshot.value?.rules).toHaveLength(fixture.rules.length)
  })

  it('lookup returns the matching rule entry', async () => {
    fetchMock.mockResolvedValue(jsonResponse(fixture))
    const { useSanitizerRules } = await importComposable()

    const api = useSanitizerRules()
    await api.loadOnce()

    const aws = api.lookup('detect_secrets_aws_v1')
    expect(aws).not.toBeNull()
    expect(aws).toEqual({
      rule_id: 'detect_secrets_aws_v1',
      kind: 'secret',
      description: 'AWS access key (static credential)',
      pattern_summary: 'AKIA[0-9A-Z]{16}',
      source: 'builtin'
    })
  })

  it('lookup returns null for unknown rule_id', async () => {
    fetchMock.mockResolvedValue(jsonResponse(fixture))
    const { useSanitizerRules } = await importComposable()

    const api = useSanitizerRules()
    await api.loadOnce()

    const result = api.lookup('nonexistent_rule_xyz')
    expect(result).toBeNull()
  })

  it('source does not request the full regex (no `pattern_full` / `regex_full` / `?full=true`)', () => {
    // Source-grep on the composable file. Reading the source directly
    // (not the bundled output) keeps this resilient to future refactors
    // that might inline new helpers but should still never reference
    // any "expose full regex" knob.
    const sourcePath = resolve(
      process.cwd(),
      'app/composables/useSanitizerRules.ts'
    )
    const source = readFileSync(sourcePath, 'utf-8')
    expect(source).not.toContain('pattern_full')
    expect(source).not.toContain('regex_full')
    expect(source).not.toContain('?full=true')
    expect(source).not.toContain('&full=true')
  })
})
