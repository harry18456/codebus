import { existsSync } from 'node:fs'
import { mkdir, writeFile, readFile, appendFile } from 'node:fs/promises'
import { join } from 'node:path'
import { vaultPaths } from '../core/vault/layout.js'
import { initNestedRepo, autoCommit } from '../infra/git/nested-repo.js'
import { CODEBUS_SCHEMA_MARKDOWN } from '../schema/claude-md.js'

const INTERNAL_GITIGNORE = '.lock\nraw/code/\n'

export async function runInit(repoRoot: string): Promise<void> {
  const p = vaultPaths(repoRoot)

  await mkdir(p.root, { recursive: true })
  await mkdir(p.raw, { recursive: true })
  await mkdir(p.rawCode, { recursive: true })
  await mkdir(p.wiki, { recursive: true })
  await mkdir(p.wikiPages, { recursive: true })
  await mkdir(p.wikiGoals, { recursive: true })
  await mkdir(p.output, { recursive: true })

  if (!existsSync(p.schemaMd)) {
    await writeFile(p.schemaMd, CODEBUS_SCHEMA_MARKDOWN)
  }

  if (!existsSync(p.goalsJsonl)) {
    await writeFile(p.goalsJsonl, '')
  }

  if (!existsSync(p.gitignore)) {
    await writeFile(p.gitignore, INTERNAL_GITIGNORE)
  }

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
