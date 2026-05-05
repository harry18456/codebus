import { existsSync } from 'node:fs'
import { mkdir, writeFile, readFile, appendFile } from 'node:fs/promises'
import { join } from 'node:path'
import { vaultPaths } from '../core/vault/layout.js'
import { initNestedRepo, autoCommit } from '../infra/git/nested-repo.js'
import { CODEBUS_SCHEMA_MARKDOWN } from '../schema/claude-md.js'

// Required entries in .codebus/.gitignore. Init merges these into any
// existing file (does not overwrite) so re-init or upgrades pick up new
// entries added in later codebus versions.
const REQUIRED_INTERNAL_GITIGNORE_LINES = [
  '.lock',
  'raw/code/',
  '**/.obsidian/'  // Obsidian per-vault config (workspace state, plugin binaries, cache)
]

export async function runInit(repoRoot: string): Promise<void> {
  const p = vaultPaths(repoRoot)

  await mkdir(p.root, { recursive: true })
  await mkdir(p.raw, { recursive: true })
  await mkdir(p.rawCode, { recursive: true })
  await mkdir(p.wiki, { recursive: true })
  for (const folder of p.wikiPageFolders) {
    await mkdir(folder, { recursive: true })
  }
  // wiki/goals/ removed in wiki-taxonomy-realign — schema no longer
  // mandates per-goal reading guides; log.md absorbs the narrative.
  await mkdir(p.output, { recursive: true })

  if (!existsSync(p.schemaMd)) {
    await writeFile(p.schemaMd, CODEBUS_SCHEMA_MARKDOWN)
  }

  if (!existsSync(p.goalsJsonl)) {
    await writeFile(p.goalsJsonl, '')
  }

  await mergeGitignoreLines(p.gitignore, REQUIRED_INTERNAL_GITIGNORE_LINES)

  await initNestedRepo(p.root)

  // Add .codebus to source repo .gitignore (only when source is a git repo).
  if (existsSync(join(repoRoot, '.git'))) {
    const giPath = join(repoRoot, '.gitignore')
    let content = ''
    if (existsSync(giPath)) content = await readFile(giPath, 'utf8')
    const lines = content.split('\n').map((l) => l.trim())
    if (!lines.includes('.codebus')) {
      const ensureNl = content.length && !content.endsWith('\n') ? '\n' : ''
      await appendFile(giPath, `${ensureNl}.codebus\n`)
    }
  }

  await autoCommit(p.root, 'init: codebus vault')
}

// Append any missing required lines to a .gitignore file. Creates the
// file if absent. Idempotent — running twice produces the same result.
async function mergeGitignoreLines(path: string, required: string[]): Promise<void> {
  let existing = ''
  if (existsSync(path)) existing = await readFile(path, 'utf8')
  const present = new Set(existing.split('\n').map((l) => l.trim()))
  const missing = required.filter((l) => !present.has(l))
  if (missing.length === 0) return
  const ensureNl = existing.length && !existing.endsWith('\n') ? '\n' : ''
  await appendFile(path, `${ensureNl}${missing.join('\n')}\n`)
}
