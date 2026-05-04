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

export async function runQuery(opts: RunQueryOptions): Promise<void> {
  const p = vaultPaths(opts.repoRoot)

  if (!existsSync(p.wikiPages)) {
    throw new Error(`${NEED_GOAL_HINT} (.codebus/wiki/pages/ 不存在)`)
  }
  const files = await readdir(p.wikiPages)
  if (files.filter((f) => f.endsWith('.md')).length === 0) {
    throw new Error(`${NEED_GOAL_HINT} (.codebus/wiki/pages/ 為空)`)
  }

  const schema = existsSync(p.schemaMd) ? await readFile(p.schemaMd, 'utf8') : ''
  const indexMd = existsSync(p.wikiIndex) ? await readFile(p.wikiIndex, 'utf8') : '(empty)'
  const systemPrompt =
    `${schema}\n\n# Current wiki index\n\n${indexMd}\n\n# Mode: Query\n\n` +
    `Answer the user's question by reading wiki/pages/*.md. ` +
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
