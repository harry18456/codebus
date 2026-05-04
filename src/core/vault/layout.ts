import { join } from 'node:path'

export interface VaultPaths {
  root: string
  git: string
  gitignore: string
  goalsJsonl: string
  schemaMd: string
  raw: string
  rawCode: string
  wiki: string
  wikiOverview: string
  wikiIndex: string
  wikiLog: string
  wikiPages: string
  wikiGoals: string
  output: string
  lock: string
}

export function vaultPaths(repoRoot: string): VaultPaths {
  const root = join(repoRoot, '.codebus')
  const wiki = join(root, 'wiki')
  const raw = join(root, 'raw')
  return {
    root,
    git: join(root, '.git'),
    gitignore: join(root, '.gitignore'),
    goalsJsonl: join(root, 'goals.jsonl'),
    schemaMd: join(root, 'CLAUDE.md'),
    raw,
    rawCode: join(raw, 'code'),
    wiki,
    wikiOverview: join(wiki, 'overview.md'),
    wikiIndex: join(wiki, 'index.md'),
    wikiLog: join(wiki, 'log.md'),
    wikiPages: join(wiki, 'pages'),
    wikiGoals: join(wiki, 'goals'),
    output: join(root, 'output'),
    lock: join(root, '.lock')
  }
}
