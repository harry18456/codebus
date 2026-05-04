import { existsSync } from 'node:fs'
import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { parsePage } from './frontmatter.js'
import { PAGE_TYPE_FOLDERS, PAGE_TYPE_FROM_FOLDER, type PageType } from './types.js'

export type LintSeverity = 'error' | 'warn'

export interface LintIssue {
  // Path relative to wiki/ for display (e.g. "concepts/foo.md", "test.md")
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
const PAGE_FOLDER_NAMES = Object.values(PAGE_TYPE_FOLDERS)

// Body wikilink regex — matches [[slug]], [[slug|display]], [[slug#heading]],
// [[slug#heading|display]]; captures slug only.
const BODY_WIKILINK_REGEX = /\[\[([^\]|#\s]+)(?:#[^\]|]+)?(?:\|[^\]]+)?\]\]/g

// Strip [[ ]] from a related[] entry to get the bare slug. Tolerant of
// whitespace because YAML parsers may keep surrounding whitespace.
const RELATED_STRIP_REGEX = /^\s*\[\[([^\]]+)\]\]\s*$/

interface PageEntry {
  folder: string
  filename: string
  slug: string
  relPath: string
  fullPath: string
}

// Validate a vault's wiki/ subtree against schema rules + Obsidian
// compatibility expectations. Pure read — never writes. Used by:
//   - commands/goal.ts (auto-lint after every ingest, soft mode)
//   - commands/check.ts (standalone --check flag)
//
// vaultRoot is the .codebus/ path (e.g. /repo/.codebus/, NOT /repo/).
export async function lintWiki(vaultRoot: string): Promise<LintResult> {
  const wikiRoot = join(vaultRoot, 'wiki')
  const issues: LintIssue[] = []
  let pagesScanned = 0

  if (!existsSync(wikiRoot)) {
    return summarize(pagesScanned, issues)
  }

  // 1. Catalog all page entries across the 5 type folders. Slug = filename
  //    without .md (Obsidian wikilinks resolve by filename ignoring folder).
  const allPages: PageEntry[] = []
  const slugToPages = new Map<string, PageEntry[]>()
  for (const folder of PAGE_FOLDER_NAMES) {
    const folderPath = join(wikiRoot, folder)
    if (!existsSync(folderPath)) continue
    for (const f of await readdir(folderPath)) {
      if (!f.endsWith('.md')) continue
      const slug = f.replace(/\.md$/, '')
      const entry: PageEntry = {
        folder,
        filename: f,
        slug,
        relPath: `${folder}/${f}`,
        fullPath: join(folderPath, f)
      }
      allPages.push(entry)
      const existing = slugToPages.get(slug)
      if (existing) existing.push(entry)
      else slugToPages.set(slug, [entry])
    }
  }
  const pageSlugs = new Set(allPages.map((p) => p.slug))

  // 2. Cross-folder slug collision warning. Obsidian resolves [[slug]] by
  //    first match — multiple pages with the same slug make link target
  //    non-deterministic.
  for (const [slug, entries] of slugToPages) {
    if (entries.length > 1) {
      const others = entries.map((e) => e.relPath).join(', ')
      for (const e of entries) {
        issues.push({
          path: e.relPath,
          severity: 'warn',
          message: `duplicate slug '${slug}' across folders: ${others} — wikilink [[${slug}]] becomes ambiguous`
        })
      }
    }
  }

  // 3. Walk each page — parse frontmatter, verify wikilinks, check folder/type alignment.
  for (const entry of allPages) {
    const content = await readFile(entry.fullPath, 'utf8')
    let parsed
    try {
      parsed = parsePage(content)
      pagesScanned++
    } catch (err) {
      issues.push({
        path: entry.relPath,
        severity: 'error',
        message: `frontmatter parse failed: ${(err as Error).message}`
      })
      continue
    }

    // Folder ↔ frontmatter type alignment (warn — agent may legitimately
    // place a borderline page; folder is the strong signal in Obsidian
    // sidebar, type is the taxonomic claim).
    const expectedType = PAGE_TYPE_FROM_FOLDER[entry.folder] as PageType | undefined
    if (expectedType && parsed.frontmatter.type !== expectedType) {
      issues.push({
        path: entry.relPath,
        severity: 'warn',
        message: `folder/type mismatch: file in '${entry.folder}/' but frontmatter type is '${parsed.frontmatter.type}' (expected '${expectedType}')`
      })
    }

    // Validate frontmatter related[] entries — strict, must be parseable
    // [[wikilink]] format and resolve to existing page.
    for (const ref of parsed.frontmatter.related) {
      const m = ref.match(RELATED_STRIP_REGEX)
      if (!m) {
        issues.push({
          path: entry.relPath,
          severity: 'error',
          message: `related[] entry not in [[wikilink]] format: ${ref}`
        })
        continue
      }
      const slug = m[1].trim()
      if (!pageSlugs.has(slug)) {
        issues.push({
          path: entry.relPath,
          severity: 'error',
          message: `broken wikilink in related: [[${slug}]] (no page named ${slug}.md in any wiki/<type>/ folder)`
        })
      }
    }

    // Validate body wikilinks (warn — body may legit reference future pages).
    const seen = new Set<string>()
    for (const m of parsed.body.matchAll(BODY_WIKILINK_REGEX)) {
      const slug = m[1].trim()
      if (seen.has(slug)) continue
      seen.add(slug)
      if (!pageSlugs.has(slug)) {
        issues.push({
          path: entry.relPath,
          severity: 'warn',
          message: `broken wikilink in body: [[${slug}]] (no page named ${slug}.md in any wiki/<type>/ folder)`
        })
      }
    }
  }

  // 4. Detect pages outside the 5 type folders. Special files at wiki/
  //    root are correct; anything else .md at root is a misplacement.
  const wikiEntries = await readdir(wikiRoot, { withFileTypes: true })
  for (const e of wikiEntries) {
    if (e.isFile() && e.name.endsWith('.md') && !SPECIAL_FILES.includes(e.name)) {
      issues.push({
        path: e.name,
        severity: 'warn',
        message: `page lives in wiki/ root — schema §3 expects wiki/<type>/${e.name} (one of: concepts, entities, modules, processes, synthesis)`
      })
    }
  }

  // 5. Special files presence (warn — vault works without them but agent
  //    may create stale-looking output if missing).
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
