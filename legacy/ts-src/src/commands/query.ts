import { existsSync } from 'node:fs'
import { readdir, readFile } from 'node:fs/promises'
import { vaultPaths } from '../core/vault/layout.js'
import type { LLMProvider, StreamEvent } from '../infra/llm/types.js'

export interface RunQueryOptions {
  repoRoot: string
  query: string
  provider: LLMProvider
  onEvent?: (e: StreamEvent) => void
}

const NEED_GOAL_HINT = '請先用 --goal 建 wiki'

async function hasAnyMarkdownAcrossFolders(folders: readonly string[]): Promise<boolean> {
  for (const folder of folders) {
    if (!existsSync(folder)) continue
    const entries = await readdir(folder)
    if (entries.some((f) => f.endsWith('.md'))) return true
  }
  return false
}

export async function runQuery(opts: RunQueryOptions): Promise<void> {
  const p = vaultPaths(opts.repoRoot)

  // 5-folder structure (concepts/entities/modules/processes/synthesis):
  // accept query if ANY folder contains a .md. We don't gate on a specific
  // folder existing — init always mkdirs all five — but vault may have
  // been hand-edited.
  if (!(await hasAnyMarkdownAcrossFolders(p.wikiPageFolders))) {
    throw new Error(`${NEED_GOAL_HINT} (.codebus/wiki/{concepts,entities,modules,processes,synthesis}/ 皆無 .md)`)
  }

  const schema = existsSync(p.schemaMd) ? await readFile(p.schemaMd, 'utf8') : ''
  const indexMd = existsSync(p.wikiIndex) ? await readFile(p.wikiIndex, 'utf8') : '(empty)'
  const systemPrompt =
    `${schema}\n\n# Current wiki index\n\n${indexMd}\n\n# Mode: Query\n\n` +
    `Answer the user's question by reading wiki/{concepts,entities,modules,processes,synthesis}/*.md. ` +
    `Cite pages using [[wikilink]]. Do NOT write any files.`

  for await (const event of opts.provider.invoke({
    systemPrompt,
    userMessage: opts.query,
    mode: 'query',
    cwd: p.root,
    vaultRoot: p.root
  })) {
    opts.onEvent?.(event)
  }
}
