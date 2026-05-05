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
    // Mirror init.ts post wiki-taxonomy-realign: 5 type folders only.
    // wiki/goals/ is no longer created by init; overview.md is no longer
    // a recognized special file.
    mkdirSync(join(vault, 'wiki', 'concepts'), { recursive: true })
    mkdirSync(join(vault, 'wiki', 'entities'), { recursive: true })
    mkdirSync(join(vault, 'wiki', 'modules'), { recursive: true })
    mkdirSync(join(vault, 'wiki', 'processes'), { recursive: true })
    mkdirSync(join(vault, 'wiki', 'synthesis'), { recursive: true })
    writeFileSync(join(vault, 'wiki', 'index.md'), '# Index')
    writeFileSync(join(vault, 'wiki', 'log.md'), '# Log')
  })
  afterEach(() => { rmSync(vault, { recursive: true, force: true }) })

  it('returns no issues for a clean vault with no pages', async () => {
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(0)
    expect(result.warnCount).toBe(0)
    expect(result.pagesScanned).toBe(0)
    // index.md + log.md exist (beforeEach) → 2 nav files (overview no
    // longer counted; goals/ no longer scanned).
    expect(result.navFilesScanned).toBe(2)
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

  it('flags ERROR for related[] entry not in [[wikilink]] format', async () => {
    // Plain string (no surrounding [[ ]]) — fails the format guard before
    // catalog lookup, so error message is about format not broken-target.
    writeFileSync(
      join(vault, 'wiki', 'concepts', 'a.md'),
      validPage({ title: 'A', related: ['plain-text'] })
    )
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(1)
    const issue = result.issues.find((i) => i.severity === 'error')!
    expect(issue.message).toContain('related[] entry not in [[wikilink]] format')
    expect(issue.message).toContain('plain-text')
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

  it('does NOT flag folder/type mismatch (folder is organizational hint, not contract)', async () => {
    // Post wiki-taxonomy-realign: frontmatter type is authoritative;
    // folder is the recommended visual home. Lint no longer reports
    // mismatch.
    writeFileSync(join(vault, 'wiki', 'concepts', 'a.md'), validPage({ title: 'A', type: 'module' }))
    const result = await lintWiki(vault)
    const mismatchIssues = result.issues.filter((i) => i.message.includes('folder/type mismatch'))
    expect(mismatchIssues).toHaveLength(0)
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

  it('flags WARN for missing index.md', async () => {
    rmSync(join(vault, 'wiki', 'index.md'))
    const result = await lintWiki(vault)
    const issue = result.issues.find((i) => i.path === 'index.md')
    expect(issue).toBeDefined()
    expect(issue!.severity).toBe('warn')
    expect(issue!.message).toContain('missing')
  })

  it('flags WARN for missing log.md', async () => {
    rmSync(join(vault, 'wiki', 'log.md'))
    const result = await lintWiki(vault)
    const issue = result.issues.find((i) => i.path === 'log.md')
    expect(issue).toBeDefined()
    expect(issue!.severity).toBe('warn')
    expect(issue!.message).toContain('missing')
  })

  it('does NOT flag missing overview.md (no longer a special file)', async () => {
    // overview.md was removed from SPECIAL_FILES in wiki-taxonomy-realign.
    // beforeEach no longer creates it; lint must not warn about its
    // absence (any "missing" issue keyed at overview.md is a regression).
    const result = await lintWiki(vault)
    const overviewIssues = result.issues.filter((i) => i.path === 'overview.md')
    expect(overviewIssues).toHaveLength(0)
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
    expect(result.navFilesScanned).toBe(0)
    expect(result.issues).toEqual([])
    rmSync(empty, { recursive: true, force: true })
  })

  it('flags WARN for broken wikilink in index.md body', async () => {
    writeFileSync(join(vault, 'wiki', 'index.md'), '- [[ghost]]\n')
    const result = await lintWiki(vault)
    const issue = result.issues.find((i) => i.path === 'index.md')
    expect(issue).toBeDefined()
    expect(issue!.severity).toBe('warn')
    expect(issue!.message).toContain('ghost')
  })

  it('flags WARN for broken wikilink in log.md body', async () => {
    writeFileSync(join(vault, 'wiki', 'log.md'), '## [2026-05-04] goal: "x" → covers [[ghost]]\n')
    const result = await lintWiki(vault)
    const issue = result.issues.find((i) => i.path === 'log.md')
    expect(issue).toBeDefined()
    expect(issue!.severity).toBe('warn')
    expect(issue!.message).toContain('ghost')
  })

  it('resolves wikilinks pointing at existing nav files (index/log)', async () => {
    // index/log are valid wikilink targets in Obsidian — they live at
    // wiki/ root, are real .md files, and [[index]] / [[log]] resolve.
    writeFileSync(join(vault, 'wiki', 'index.md'), '見 [[log]]\n')
    const result = await lintWiki(vault)
    const indexIssues = result.issues.filter((i) => i.path === 'index.md')
    expect(indexIssues).toHaveLength(0)
  })

  it('flags WARN when wikilink targets a missing nav file (log.md)', async () => {
    rmSync(join(vault, 'wiki', 'log.md'))
    writeFileSync(join(vault, 'wiki', 'index.md'), '見 [[log]]\n')
    const result = await lintWiki(vault)
    const issue = result.issues.find((i) => i.path === 'index.md' && i.message.includes('log'))
    expect(issue).toBeDefined()
    expect(issue!.severity).toBe('warn')
  })

  it('flags [[goal-slug]] as broken even when wiki/goals/<slug>.md exists', async () => {
    // Post wiki-taxonomy-realign: wiki/goals/ is no longer a recognized
    // directory; even if a leftover goal-guide file exists from a prior
    // codebus version, its slug SHALL NOT be added to the catalog.
    mkdirSync(join(vault, 'wiki', 'goals'), { recursive: true })
    writeFileSync(join(vault, 'wiki', 'goals', 'project-purpose.md'), '# Goal: project-purpose')
    writeFileSync(join(vault, 'wiki', 'index.md'), '## Goals\n- [[project-purpose]]\n')
    const result = await lintWiki(vault)
    const indexIssue = result.issues.find(
      (i) => i.path === 'index.md' && i.message.includes('project-purpose')
    )
    expect(indexIssue).toBeDefined()
    expect(indexIssue!.severity).toBe('warn')
    expect(indexIssue!.message).toContain('broken wikilink in body')
  })

  it('counts only existing nav files index.md + log.md (ignores wiki/goals/)', async () => {
    // Even if goal-guide leftover files exist, navFilesScanned must
    // count only the two surviving specials.
    mkdirSync(join(vault, 'wiki', 'goals'), { recursive: true })
    writeFileSync(join(vault, 'wiki', 'goals', 'one.md'), '# one')
    writeFileSync(join(vault, 'wiki', 'goals', 'two.md'), '# two')
    const result = await lintWiki(vault)
    // beforeEach creates index.md + log.md → 2 nav files (goal guides
    // are no longer scanned).
    expect(result.navFilesScanned).toBe(2)
  })

  it('does not count missing nav files in navFilesScanned', async () => {
    rmSync(join(vault, 'wiki', 'log.md'))
    const result = await lintWiki(vault)
    // only index.md remains → 1 nav file scanned
    expect(result.navFilesScanned).toBe(1)
  })

  it('clean run reports `N pages + 2 nav files scanned` in coverage line', async () => {
    writeFileSync(join(vault, 'wiki', 'concepts', 'a.md'), validPage({ title: 'A' }))
    writeFileSync(join(vault, 'wiki', 'concepts', 'b.md'), validPage({ title: 'B' }))
    writeFileSync(join(vault, 'wiki', 'concepts', 'c.md'), validPage({ title: 'C' }))
    const result = await lintWiki(vault)
    expect(result.errorCount).toBe(0)
    expect(result.warnCount).toBe(0)
    expect(result.pagesScanned).toBe(3)
    expect(result.navFilesScanned).toBe(2)
  })
})
