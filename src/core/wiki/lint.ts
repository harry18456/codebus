import { existsSync } from 'node:fs'
import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { parsePage } from './frontmatter.js'

export type LintSeverity = 'error' | 'warn'

export interface LintIssue {
  // Path relative to wiki/ for display (e.g. "pages/foo.md", "test.md")
  path: string
  severity: LintSeverity
  message: string
}

export interface LintResult {
  pagesScanned: number
  issues: LintIssue[]
  errorCount: number
  warnCount: number
}

const SPECIAL_FILES = ['overview.md', 'index.md', 'log.md']

// Body wikilink regex — matches [[slug]], [[slug|display]], [[slug#heading]],
// [[slug#heading|display]]; captures slug only.
const BODY_WIKILINK_REGEX = /\[\[([^\]|#\s]+)(?:#[^\]|]+)?(?:\|[^\]]+)?\]\]/g

// Strip [[ ]] from a related[] entry to get the bare slug. Tolerant of
// whitespace because YAML parsers may keep surrounding whitespace.
const RELATED_STRIP_REGEX = /^\s*\[\[([^\]]+)\]\]\s*$/

// Validate a vault's wiki/ subtree against schema rules + Obsidian
// compatibility expectations. Pure read — never writes. Used by:
//   - commands/goal.ts (auto-lint after every ingest, soft mode)
//   - commands/check.ts (standalone --check flag)
//
// vaultRoot is the .codebus/ path (e.g. /repo/.codebus/, NOT /repo/).
export async function lintWiki(vaultRoot: string): Promise<LintResult> {
  const wikiRoot = join(vaultRoot, 'wiki')
  const pagesDir = join(wikiRoot, 'pages')
  const issues: LintIssue[] = []
  let pagesScanned = 0

  if (!existsSync(wikiRoot)) {
    return summarize(pagesScanned, issues)
  }

  // 1. Catalog known page slugs (filename without .md) to validate wikilinks against.
  const pageSlugs = new Set<string>()
  if (existsSync(pagesDir)) {
    for (const f of await readdir(pagesDir)) {
      if (f.endsWith('.md')) pageSlugs.add(f.replace(/\.md$/, ''))
    }
  }

  // 2. Walk wiki/pages/*.md — parse frontmatter + verify all wikilinks resolve.
  if (existsSync(pagesDir)) {
    const files = await readdir(pagesDir)
    for (const f of files) {
      if (!f.endsWith('.md')) continue
      const fullPath = join(pagesDir, f)
      const content = await readFile(fullPath, 'utf8')
      const relPath = `pages/${f}`
      let parsed
      try {
        parsed = parsePage(content)
        pagesScanned++
      } catch (err) {
        issues.push({
          path: relPath,
          severity: 'error',
          message: `frontmatter parse failed: ${(err as Error).message}`
        })
        continue
      }

      // Validate frontmatter related[] entries
      for (const ref of parsed.frontmatter.related) {
        const m = ref.match(RELATED_STRIP_REGEX)
        if (!m) {
          issues.push({
            path: relPath,
            severity: 'error',
            message: `related[] entry not in [[wikilink]] format: ${ref}`
          })
          continue
        }
        const slug = m[1].trim()
        if (!pageSlugs.has(slug)) {
          issues.push({
            path: relPath,
            severity: 'error',
            message: `broken wikilink in related: [[${slug}]] (no matching wiki/pages/${slug}.md)`
          })
        }
      }

      // Validate body wikilinks (warn — body may legit reference future pages)
      const seen = new Set<string>()
      for (const m of parsed.body.matchAll(BODY_WIKILINK_REGEX)) {
        const slug = m[1].trim()
        if (seen.has(slug)) continue
        seen.add(slug)
        if (!pageSlugs.has(slug)) {
          issues.push({
            path: relPath,
            severity: 'warn',
            message: `broken wikilink in body: [[${slug}]] (no matching wiki/pages/${slug}.md)`
          })
        }
      }
    }
  }

  // 3. Detect pages outside wiki/pages/ — schema §3 expects pages there.
  // Special files (overview/index/log) at wiki/ root are correct.
  if (existsSync(wikiRoot)) {
    const wikiEntries = await readdir(wikiRoot, { withFileTypes: true })
    for (const e of wikiEntries) {
      if (e.isFile() && e.name.endsWith('.md') && !SPECIAL_FILES.includes(e.name)) {
        issues.push({
          path: e.name,
          severity: 'warn',
          message: `page lives in wiki/ root — schema §3 expects wiki/pages/${e.name}`
        })
      }
    }
  }

  // 4. Special files presence (warn — vault works without them but agent may
  // create stale-looking output if missing).
  for (const sf of SPECIAL_FILES) {
    if (!existsSync(join(wikiRoot, sf))) {
      issues.push({
        path: sf,
        severity: 'warn',
        message: `${sf} missing — schema §3 expects this special file`
      })
    }
  }

  return summarize(pagesScanned, issues)
}

function summarize(pagesScanned: number, issues: LintIssue[]): LintResult {
  return {
    pagesScanned,
    issues,
    errorCount: issues.filter((i) => i.severity === 'error').length,
    warnCount: issues.filter((i) => i.severity === 'warn').length
  }
}
