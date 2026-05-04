import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { lintWiki } from '../../../src/core/wiki/lint.js'
import type { PageType } from '../../../src/core/wiki/types.js'

interface PageOpts {
  title: string
  type?: PageType
  related?: string[]
  body?: string
}

function validPage(opts: PageOpts): string {
  const type = opts.type ?? 'concept'
  const related = opts.related ?? []
  const body = opts.body ?? ''
  const relatedYaml = related.length === 0
    ? '[]'
    : '\n  - ' + related.map((r) => `'${r}'`).join('\n  - ')
  return `---
title: ${opts.title}
type: ${type}
sources: []
goals: []
created: '2026-05-04'
updated: '2026-05-04'
related: ${relatedYaml}
stale: false
---
${body}
`
}

describe('lintWiki', () => {
  let vault: string
  beforeEach(() => {
    vault = mkdtempSync(join(tmpdir(), 'codebus-lint-'))
    // Mirror init.ts: create the 5 type folders + goals/.
    mkdirSync(join(vault, 'wiki', 'concepts'), { recursive: true })
    mkdirSync(join(vault, 'wiki', 'entities'), { recursive: true })
    mkdirSync(join(vault, 'wiki', 'modules'), { recursive: true })
    mkdirSync(join(vault, 'wiki', 'processes'), { recursive: true })
    mkdirSync(join(vault, 'wiki', 'synthesis'), { recursive: true })
    mkdirSync(join(vault, 'wiki', 'goals'), { recursive: true })
    writeFileSync(join(vault, 'wiki', 'overview.md'), '# Overview')
    writeFileSync(join(vault, 'wiki', 'index.md'), '# Index')
    writeFileSync(join(vault, 'wiki', 'log.md'), '# Log')
  })
  afterEach(() => { rmSync(vault, { recursive: true, force: true }) })

  it('returns no issues for a clean vault with no pages', async () => {
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(0)
    expect(result.warnCount).toBe(0)
    expect(result.pagesScanned).toBe(0)
  })

  it('returns no issues when pages have valid frontmatter and resolve all wikilinks', async () => {
    writeFileSync(join(vault, 'wiki', 'concepts', 'a.md'), validPage({ title: 'A', related: ['[[b]]'] }))
    writeFileSync(join(vault, 'wiki', 'concepts', 'b.md'), validPage({ title: 'B', related: ['[[a]]'] }))
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(0)
    expect(result.warnCount).toBe(0)
    expect(result.pagesScanned).toBe(2)
  })

  it('flags ERROR when frontmatter parse fails', async () => {
    writeFileSync(join(vault, 'wiki', 'concepts', 'broken.md'), '---\ninvalid yaml [[\n---\nbody')
    const result = await lintWiki(vault)
    expect(result.errorCount).toBeGreaterThan(0)
    expect(result.issues.some((i) => i.path === 'concepts/broken.md' && i.severity === 'error')).toBe(true)
  })

  it('flags ERROR for broken wikilink in related[]', async () => {
    writeFileSync(join(vault, 'wiki', 'concepts', 'a.md'), validPage({ title: 'A', related: ['[[nonexistent]]'] }))
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(1)
    const issue = result.issues.find((i) => i.severity === 'error')!
    expect(issue.message).toContain('broken wikilink in related')
    expect(issue.message).toContain('nonexistent')
  })

  it('flags WARN for broken wikilink in body (body is more lenient)', async () => {
    writeFileSync(
      join(vault, 'wiki', 'concepts', 'a.md'),
      validPage({ title: 'A', body: 'See [[nonexistent]] for details.' })
    )
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(0)
    expect(result.warnCount).toBe(1)
    expect(result.issues[0].severity).toBe('warn')
    expect(result.issues[0].message).toContain('broken wikilink in body')
  })

  it('handles body wikilinks with display text and section anchors', async () => {
    writeFileSync(join(vault, 'wiki', 'concepts', 'a.md'), validPage({ title: 'A' }))
    writeFileSync(
      join(vault, 'wiki', 'concepts', 'b.md'),
      validPage({ title: 'B', body: 'See [[a|the alpha]] and [[a#section]] and [[a#section|alpha section]].' })
    )
    const result = await lintWiki(vault)
    // All 3 body links resolve to "a" — no broken
    expect(result.warnCount).toBe(0)
  })

  it('resolves wikilinks across different type folders', async () => {
    // [[checkout-flow]] in concepts/ links to processes/checkout-flow.md.
    // Obsidian resolves by filename, ignoring folder.
    writeFileSync(
      join(vault, 'wiki', 'concepts', 'a.md'),
      validPage({ title: 'A', related: ['[[checkout-flow]]'] })
    )
    writeFileSync(
      join(vault, 'wiki', 'processes', 'checkout-flow.md'),
      validPage({ title: 'Checkout', type: 'process' })
    )
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(0)
    expect(result.warnCount).toBe(0)
    expect(result.pagesScanned).toBe(2)
  })

  it('flags WARN for folder/type mismatch', async () => {
    // file lives in concepts/ but frontmatter declares type=module
    writeFileSync(join(vault, 'wiki', 'concepts', 'a.md'), validPage({ title: 'A', type: 'module' }))
    const result = await lintWiki(vault)
    const issue = result.issues.find((i) => i.message.includes('folder/type mismatch'))
    expect(issue).toBeDefined()
    expect(issue!.severity).toBe('warn')
    expect(issue!.path).toBe('concepts/a.md')
    expect(issue!.message).toContain("expected 'concept'")
  })

  it('flags WARN for duplicate slugs across folders (ambiguous wikilink target)', async () => {
    writeFileSync(join(vault, 'wiki', 'concepts', 'cart.md'), validPage({ title: 'Cart concept' }))
    writeFileSync(join(vault, 'wiki', 'entities', 'cart.md'), validPage({ title: 'Cart entity', type: 'entity' }))
    const result = await lintWiki(vault)
    const dupIssues = result.issues.filter((i) => i.message.includes("duplicate slug 'cart'"))
    expect(dupIssues).toHaveLength(2)
    expect(dupIssues.every((i) => i.severity === 'warn')).toBe(true)
  })

  it('flags WARN when page lives in wiki/ root (not in any type folder)', async () => {
    writeFileSync(join(vault, 'wiki', 'test.md'), validPage({ title: 'Test' }))
    const result = await lintWiki(vault)
    const issue = result.issues.find((i) => i.path === 'test.md')
    expect(issue).toBeDefined()
    expect(issue!.severity).toBe('warn')
    expect(issue!.message).toContain('wiki/ root')
  })

  it('flags WARN for missing special files', async () => {
    rmSync(join(vault, 'wiki', 'overview.md'))
    const result = await lintWiki(vault)
    const issue = result.issues.find((i) => i.path === 'overview.md')
    expect(issue).toBeDefined()
    expect(issue!.severity).toBe('warn')
    expect(issue!.message).toContain('missing')
  })

  it('does not de-duplicate identical errors across pages (each occurrence reported)', async () => {
    writeFileSync(join(vault, 'wiki', 'concepts', 'a.md'), validPage({ title: 'A', related: ['[[ghost]]'] }))
    writeFileSync(join(vault, 'wiki', 'concepts', 'b.md'), validPage({ title: 'B', related: ['[[ghost]]'] }))
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(2)
  })

  it('returns empty result when wiki/ does not exist', async () => {
    const empty = mkdtempSync(join(tmpdir(), 'codebus-empty-'))
    const result = await lintWiki(empty)
    expect(result.pagesScanned).toBe(0)
    expect(result.issues).toEqual([])
    rmSync(empty, { recursive: true, force: true })
  })
})
