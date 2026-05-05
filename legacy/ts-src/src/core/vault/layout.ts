import { join } from 'node:path'
import { PAGE_TYPE_FOLDERS, type PageType } from '../wiki/types.js'

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
  // Karpathy-style 5-bucket structure (concepts / entities / modules /
  // processes / synthesis). Per-type absolute paths for callers that
  // need a specific bucket; wikiPageFolders gives the iteration order.
  wikiConcepts: string
  wikiEntities: string
  wikiModules: string
  wikiProcesses: string
  wikiSynthesis: string
  wikiPageFolders: readonly string[]
  wikiTypeFolderMap: Readonly<Record<PageType, string>>
  // wikiGoals removed in wiki-taxonomy-realign — wiki/goals/ is no longer
  // a schema-managed directory. Per-goal narrative now folds into log.md.
  output: string
  lock: string
}

export function vaultPaths(repoRoot: string): VaultPaths {
  const root = join(repoRoot, '.codebus')
  const wiki = join(root, 'wiki')
  const raw = join(root, 'raw')
  const wikiConcepts = join(wiki, PAGE_TYPE_FOLDERS.concept)
  const wikiEntities = join(wiki, PAGE_TYPE_FOLDERS.entity)
  const wikiModules = join(wiki, PAGE_TYPE_FOLDERS.module)
  const wikiProcesses = join(wiki, PAGE_TYPE_FOLDERS.process)
  const wikiSynthesis = join(wiki, PAGE_TYPE_FOLDERS.synthesis)
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
    wikiConcepts,
    wikiEntities,
    wikiModules,
    wikiProcesses,
    wikiSynthesis,
    wikiPageFolders: [wikiConcepts, wikiEntities, wikiModules, wikiProcesses, wikiSynthesis],
    wikiTypeFolderMap: {
      concept: wikiConcepts,
      entity: wikiEntities,
      module: wikiModules,
      process: wikiProcesses,
      synthesis: wikiSynthesis
    },
    output: join(root, 'output'),
    lock: join(root, '.lock')
  }
}
