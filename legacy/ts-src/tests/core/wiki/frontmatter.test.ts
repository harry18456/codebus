import { describe, it, expect } from 'vitest'
import { parsePage, serializePage } from '../../../src/core/wiki/frontmatter.js'

const samplePage = `---
title: Payment Gateway
type: concept
sources:
  - path: src/services/payment.py
    sha256: abc123
    at_commit: deadbeef
goals:
  - "了解結帳流程"
created: '2026-05-04'
updated: '2026-05-04'
related:
  - "[[checkout-flow]]"
stale: false
---
# Payment Gateway

Body content here.
`

describe('parsePage', () => {
  it('parses frontmatter and body', () => {
    const { frontmatter, body } = parsePage(samplePage)
    expect(frontmatter.title).toBe('Payment Gateway')
    expect(frontmatter.type).toBe('concept')
    expect(frontmatter.sources).toEqual([
      { path: 'src/services/payment.py', sha256: 'abc123', at_commit: 'deadbeef' }
    ])
    expect(frontmatter.goals).toEqual(['了解結帳流程'])
    expect(frontmatter.related).toEqual(['[[checkout-flow]]'])
    expect(frontmatter.stale).toBe(false)
    expect(body.trim().startsWith('# Payment Gateway')).toBe(true)
  })

  it('throws on missing required field', () => {
    const bad = `---\ntitle: X\n---\nbody`
    expect(() => parsePage(bad)).toThrow(/required field/)
  })

  it('throws on invalid type', () => {
    const bad = `---\ntitle: X\ntype: thing\nsources: []\ngoals: []\ncreated: '2026-05-04'\nupdated: '2026-05-04'\nrelated: []\nstale: false\n---\nbody`
    expect(() => parsePage(bad)).toThrow(/Invalid page type/)
  })

  it('treats non-true stale value as false', () => {
    const partial = `---\ntitle: X\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-04'\nupdated: '2026-05-04'\nrelated: []\nstale: 'false'\n---\nbody`
    const { frontmatter } = parsePage(partial)
    expect(frontmatter.stale).toBe(false)
  })
})

describe('serializePage', () => {
  it('round-trips parse → serialize → parse', () => {
    const { frontmatter, body } = parsePage(samplePage)
    const serialized = serializePage(frontmatter, body)
    const reparsed = parsePage(serialized)
    expect(reparsed.frontmatter).toEqual(frontmatter)
  })
})
