import { describe, it, expect } from 'vitest'
import { mergePage } from '../../../src/core/wiki/page-merge.js'
import type { ParsedPage } from '../../../src/core/wiki/types.js'

const existing: ParsedPage = {
  frontmatter: {
    title: 'Payment Gateway',
    type: 'concept',
    sources: [{ path: 'src/payment.py', sha256: 'abc', at_commit: 'c1' }],
    goals: ['結帳流程'],
    created: '2026-05-01',
    updated: '2026-05-01',
    related: ['[[checkout-flow]]'],
    stale: false
  },
  body: '# Payment Gateway\n\nOriginal body.\n'
}

const incoming: ParsedPage = {
  frontmatter: {
    title: 'Payment Gateway',
    type: 'concept',
    sources: [{ path: 'src/refund.py', sha256: 'def', at_commit: 'c2' }],
    goals: ['退款處理'],
    created: '2026-05-04',
    updated: '2026-05-04',
    related: ['[[refund-flow]]'],
    stale: true
  },
  body: 'Refund-perspective content.\n'
}

describe('mergePage', () => {
  it('unions sources / goals / related arrays', () => {
    const merged = mergePage(existing, incoming, '退款處理', '2026-05-04')
    expect(merged.frontmatter.sources).toEqual([
      { path: 'src/payment.py', sha256: 'abc', at_commit: 'c1' },
      { path: 'src/refund.py', sha256: 'def', at_commit: 'c2' }
    ])
    expect(merged.frontmatter.goals).toEqual(['結帳流程', '退款處理'])
    expect(merged.frontmatter.related).toEqual(['[[checkout-flow]]', '[[refund-flow]]'])
  })

  it('locks title / type / created from existing', () => {
    const merged = mergePage(existing, incoming, '退款處理', '2026-05-04')
    expect(merged.frontmatter.title).toBe('Payment Gateway')
    expect(merged.frontmatter.type).toBe('concept')
    expect(merged.frontmatter.created).toBe('2026-05-01')
  })

  it('updates `updated` to today', () => {
    const merged = mergePage(existing, incoming, '退款處理', '2026-05-04')
    expect(merged.frontmatter.updated).toBe('2026-05-04')
  })

  it('appends ## from goal section to body', () => {
    const merged = mergePage(existing, incoming, '退款處理', '2026-05-04')
    expect(merged.body).toContain('Original body.')
    expect(merged.body).toContain('## from goal: 退款處理 (2026-05-04)')
    expect(merged.body).toContain('Refund-perspective content.')
  })

  it('does not duplicate goal in goals array if already present', () => {
    const merged = mergePage(existing, incoming, '結帳流程', '2026-05-04')
    expect(merged.frontmatter.goals).toEqual(['結帳流程', '退款處理'])
  })

  it('preserves stale flag from existing (not overridden by incoming)', () => {
    const merged = mergePage(existing, incoming, 'g', '2026-05-04')
    expect(merged.frontmatter.stale).toBe(false)
  })
})
