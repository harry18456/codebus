import { describe, it, expect } from 'vitest'
import { detectStaleSources, type StaleResult } from '../../../src/core/wiki/stale-detect.js'
import type { PageFrontmatter } from '../../../src/core/wiki/types.js'

const fm: PageFrontmatter = {
  title: 'X',
  type: 'concept',
  sources: [
    { path: 'src/a.py', sha256: 'aaa', at_commit: 'c1' },
    { path: 'src/b.py', sha256: 'bbb', at_commit: 'c1' }
  ],
  goals: [],
  created: '2026-05-04',
  updated: '2026-05-04',
  related: [],
  stale: false
}

describe('detectStaleSources', () => {
  it('returns clean when all source hashes match current', () => {
    const current = new Map([['src/a.py', 'aaa'], ['src/b.py', 'bbb']])
    const result: StaleResult = detectStaleSources(fm, current)
    expect(result.isStale).toBe(false)
    expect(result.changedSources).toEqual([])
  })

  it('returns stale when any source hash differs', () => {
    const current = new Map([['src/a.py', 'aaa'], ['src/b.py', 'bbb-NEW']])
    const result = detectStaleSources(fm, current)
    expect(result.isStale).toBe(true)
    expect(result.changedSources).toEqual(['src/b.py'])
  })

  it('returns stale when source missing from current', () => {
    const current = new Map([['src/a.py', 'aaa']])
    const result = detectStaleSources(fm, current)
    expect(result.isStale).toBe(true)
    expect(result.changedSources).toEqual(['src/b.py'])
  })

  it('handles empty sources gracefully', () => {
    const empty: PageFrontmatter = { ...fm, sources: [] }
    const result = detectStaleSources(empty, new Map())
    expect(result.isStale).toBe(false)
    expect(result.changedSources).toEqual([])
  })
})
