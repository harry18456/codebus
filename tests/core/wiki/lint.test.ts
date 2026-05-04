import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { lintWiki } from '../../../src/core/wiki/lint.js'

function validPage(title: string, related: string[] = [], body = ''): string {
  const relatedYaml = related.length === 0
    ? '[]'
    : '\n  - ' + related.map((r) => `'${r}'`).join('\n  - ')
  return `---
title: ${title}
type: concept
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
    mkdirSync(join(vault, 'wiki', 'pages'), { recursive: true })
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
    writeFileSync(join(vault, 'wiki', 'pages', 'a.md'), validPage('A', ['[[b]]']))
    writeFileSync(join(vault, 'wiki', 'pages', 'b.md'), validPage('B', ['[[a]]']))
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(0)
    expect(result.pagesScanned).toBe(2)
  })

  it('flags ERROR when frontmatter parse fails', async () => {
    writeFileSync(join(vault, 'wiki', 'pages', 'broken.md'), '---\ninvalid yaml [[\n---\nbody')
    const result = await lintWiki(vault)
    expect(result.errorCount).toBeGreaterThan(0)
    expect(result.issues.some((i) => i.path === 'pages/broken.md' && i.severity === 'error')).toBe(true)
  })

  it('flags ERROR for broken wikilink in related[]', async () => {
    writeFileSync(join(vault, 'wiki', 'pages', 'a.md'), validPage('A', ['[[nonexistent]]']))
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(1)
    const issue = result.issues.find((i) => i.severity === 'error')!
    expect(issue.message).toContain('broken wikilink in related')
    expect(issue.message).toContain('nonexistent')
  })

  it('flags WARN for broken wikilink in body (body is more lenient)', async () => {
    writeFileSync(
      join(vault, 'wiki', 'pages', 'a.md'),
      validPage('A', [], 'See [[nonexistent]] for details.')
    )
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(0)
    expect(result.warnCount).toBe(1)
    expect(result.issues[0].severity).toBe('warn')
    expect(result.issues[0].message).toContain('broken wikilink in body')
  })

  it('handles body wikilinks with display text and section anchors', async () => {
    writeFileSync(join(vault, 'wiki', 'pages', 'a.md'), validPage('A'))
    writeFileSync(
      join(vault, 'wiki', 'pages', 'b.md'),
      validPage('B', [], 'See [[a|the alpha]] and [[a#section]] and [[a#section|alpha section]].')
    )
    const result = await lintWiki(vault)
    // All 3 body links resolve to "a" — no broken
    expect(result.warnCount).toBe(0)
  })

  it('flags WARN when page lives in wiki/ root (not wiki/pages/)', async () => {
    writeFileSync(join(vault, 'wiki', 'test.md'), validPage('Test'))
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
    writeFileSync(join(vault, 'wiki', 'pages', 'a.md'), validPage('A', ['[[ghost]]']))
    writeFileSync(join(vault, 'wiki', 'pages', 'b.md'), validPage('B', ['[[ghost]]']))
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
