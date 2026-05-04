import { existsSync } from 'node:fs'
import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { parsePage } from './frontmatter.js'
import { PAGE_TYPE_FOLDERS, PAGE_TYPE_FROM_FOLDER, type PageType } from './types.js'

export type LintSeverity = 'error' | 'warn'

export interface LintIssue {
  // Path relative to wiki/ for display (e.g. "concepts/foo.md", "test.md",
  // "goals/x.md").
  path: string
  severity: LintSeverity
  message: string
}

export interface LintResult {
  // Knowledge pages with frontmatter, found under the 5 type folders and
  // parseable. Parse-failed files are NOT counted (they appear as errors
  // instead).
  pagesScanned: number
  // Navigation/support files actually read: special files at wiki/ root
  // (overview/index/log) + goal guides under wiki/goals/. Counted only
  // when the file exists. Body wikilinks in these files are validated
  // against the same slug catalog as knowledge pages.
  navFilesScanned: number
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

// Scan a body of markdown text for [[wikilink]] occurrences, push a warn
// for any slug not in the catalog. Used by both knowledge-page bodies and
// nav-file bodies (which have no frontmatter to strip).
function scanBodyWikilinks(
  content: string,
  relPath: string,
  pageSlugs: Set<string>,
  issues: LintIssue[]
): void {
  const seen = new Set<string>()
  for (const m of content.matchAll(BODY_WIKILINK_REGEX)) {
    const slug = m[1].trim()
    if (seen.has(slug)) continue
    seen.add(slug)
    if (!pageSlugs.has(slug)) {
      issues.push({
        path: relPath,
        severity: 'warn',
        message: `broken wikilink in body: [[${slug}]] (no page named ${slug}.md in any wiki/<type>/ folder)`
      })
    }
  }
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
  let navFilesScanned = 0

  if (!existsSync(wikiRoot)) {
    return summarize(pagesScanned, navFilesScanned, issues)
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

  // 1b. Special files at wiki/ root (overview/index/log) are also legitimate
  //     wikilink targets in Obsidian — \`[[overview]]\` should resolve to
  //     wiki/overview.md just like a knowledge-page slug resolves. Only add
  //     to the catalog if the file actually exists; missing specials are
  //     reported separately and a link to them is then correctly broken.
  for (const sf of SPECIAL_FILES) {
    if (existsSync(join(wikiRoot, sf))) {
      pageSlugs.add(sf.replace(/\.md$/, ''))
    }
  }

  // 1c. Goal guides at wiki/goals/*.md are also valid link targets — agent
  //     legitimately writes \`[[<goal-slug>]]\` from index.md / log.md to
  //     point readers at the per-goal reading guide. Collect filenames now
  //     so section 6 can skip a second readdir.
  const goalsDir = join(wikiRoot, 'goals')
  const goalGuideFiles: string[] = []
  if (existsSync(goalsDir)) {
    for (const f of await readdir(goalsDir)) {
      if (!f.endsWith('.md')) continue
      goalGuideFiles.push(f)
      pageSlugs.add(f.replace(/\.md$/, ''))
    }
  }

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

    // Body wikilinks (warn — body may legit reference future pages).
    scanBodyWikilinks(parsed.body, entry.relPath, pageSlugs, issues)
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

  // 5. Special files (overview/index/log) — presence check + body wikilink
  //    scan. These files are catalogs/summaries dense with [[wikilink]];
  //    a broken link here breaks wiki navigation more than a knowledge-page
  //    body link does, so it's worth surfacing.
  for (const sf of SPECIAL_FILES) {
    const fullPath = join(wikiRoot, sf)
    if (!existsSync(fullPath)) {
      issues.push({
        path: sf,
        severity: 'warn',
        message: `${sf} missing — schema §3 expects this special file`
      })
      continue
    }
    navFilesScanned++
    const content = await readFile(fullPath, 'utf8')
    scanBodyWikilinks(content, sf, pageSlugs, issues)
  }

  // 6. Goal guides at wiki/goals/*.md — body wikilink scan. Goal guides
  //    are free-form (no frontmatter contract) but they cite pages via
  //    [[wikilink]]; a delete-and-forget on a referenced page leaves the
  //    guide hanging. File list was collected in section 1c.
  for (const f of goalGuideFiles) {
    navFilesScanned++
    const content = await readFile(join(goalsDir, f), 'utf8')
    scanBodyWikilinks(content, `goals/${f}`, pageSlugs, issues)
  }

  return summarize(pagesScanned, navFilesScanned, issues)
}

function summarize(pagesScanned: number, navFilesScanned: number, issues: LintIssue[]): LintResult {
  return {
    pagesScanned,
    navFilesScanned,
    issues,
    errorCount: issues.filter((i) => i.severity === 'error').length,
    warnCount: issues.filter((i) => i.severity === 'warn').length
  }
}
