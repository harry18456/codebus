import { existsSync } from 'node:fs'
import { appendFile, readdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { vaultPaths } from '../core/vault/layout.js'
import { acquireLock, releaseLock } from '../core/vault/lock.js'
import { syncRepoToRaw } from '../infra/fs/raw-sync.js'
import { sha256File } from '../infra/fs/file-ops.js'
import { getSourceVersion } from '../infra/git/source-version.js'
import { autoCommit } from '../infra/git/nested-repo.js'
import { parsePage, serializePage } from '../core/wiki/frontmatter.js'
import { detectStaleSources } from '../core/wiki/stale-detect.js'
import { utcTodayISO } from '../core/wiki/date.js'
import { runInit } from './init.js'
import type { LLMProvider, StreamEvent } from '../infra/llm/types.js'

export interface RunGoalOptions {
  repoRoot: string
  goal: string
  provider: LLMProvider
  onEvent?: (e: StreamEvent) => void
}

export async function runGoal(opts: RunGoalOptions): Promise<void> {
  const p = vaultPaths(opts.repoRoot)

  if (!existsSync(p.root)) await runInit(opts.repoRoot)

  const lock = await acquireLock(p.lock)
  try {
    await syncRepoToRaw(opts.repoRoot, p.rawCode)

    const ver = await getSourceVersion(opts.repoRoot)
    const goalEntry = {
      goal: opts.goal,
      source_commit: ver.commit,
      uncommitted: ver.uncommitted,
      timestamp: new Date().toISOString()
    }
    await appendFile(p.goalsJsonl, JSON.stringify(goalEntry) + '\n')

    const schema = await readFile(p.schemaMd, 'utf8')
    const indexMd = existsSync(p.wikiIndex) ? await readFile(p.wikiIndex, 'utf8') : '(empty)'
    const systemPrompt = `${schema}\n\n# Current wiki index\n\n${indexMd}\n\n# Goal\n\n${opts.goal}`

    // cwd = vault root (.codebus/) — system-level isolates the user source
    // repo per spec §3.2 + spike E. Agent reads via raw/code/<path>
    // (cwd-relative). Cwd-external Writes get permission_denials in -p mode
    // (acceptEdits only auto-accepts cwd-internal writes).
    for await (const event of opts.provider.invoke({
      systemPrompt,
      userMessage: `Build/update the wiki for this goal: ${opts.goal}`,
      mode: 'ingest',
      cwd: p.root,
      vaultRoot: p.root
    })) {
      opts.onEvent?.(event)
    }

    await enrichSourceMetadata(p.wikiPages, p.rawCode, ver.commit)
    await flagStalePages(p.wikiPages, p.rawCode)
    await autoCommit(p.root, `wiki: ${opts.goal}`)
  } finally {
    await releaseLock(lock)
  }
}

// CRITICAL (review iter-8): only enrich pages where AT LEAST ONE source
// lacks sha256+at_commit (= newly written by agent in this run). Carry-over
// pages from prior runs MUST keep their old sha256 so flagStalePages can
// detect drift against current raw. The earlier broken impl unconditionally
// rewrote every page's sha256 to current raw hash → flagStalePages compared
// same-hash-vs-same-hash → never stale → §10 mechanism dead.
async function enrichSourceMetadata(
  pagesDir: string,
  rawCodeDir: string,
  commitHash: string | null
): Promise<void> {
  if (!existsSync(pagesDir)) return
  const files = await readdir(pagesDir)
  for (const f of files) {
    if (!f.endsWith('.md')) continue
    const fullPath = join(pagesDir, f)
    const content = await readFile(fullPath, 'utf8')
    let parsed
    try { parsed = parsePage(content) } catch { continue }

    if (parsed.frontmatter.sources.length === 0) continue
    const allEnriched = parsed.frontmatter.sources.every(
      (s) => Boolean(s.sha256) && Boolean(s.at_commit)
    )
    if (allEnriched) continue

    const enrichedSources = await Promise.all(
      parsed.frontmatter.sources.map(async (src) => {
        if (src.sha256 && src.at_commit) return src
        const rawPath = join(rawCodeDir, src.path)
        const sha256 = existsSync(rawPath) ? await sha256File(rawPath) : ''
        return {
          path: src.path,
          sha256,
          at_commit: commitHash ?? ''
        }
      })
    )
    const updated = serializePage(
      { ...parsed.frontmatter, sources: enrichedSources, updated: utcTodayISO() },
      parsed.body
    )
    await writeFile(fullPath, updated)
  }
}

async function flagStalePages(pagesDir: string, rawCodeDir: string): Promise<void> {
  if (!existsSync(pagesDir)) return
  const files = await readdir(pagesDir)
  for (const f of files) {
    if (!f.endsWith('.md')) continue
    const fullPath = join(pagesDir, f)
    const content = await readFile(fullPath, 'utf8')
    let parsed
    try { parsed = parsePage(content) } catch { continue }
    const currentHashes = new Map<string, string>()
    for (const src of parsed.frontmatter.sources) {
      const rawPath = join(rawCodeDir, src.path)
      if (existsSync(rawPath)) {
        currentHashes.set(src.path, await sha256File(rawPath))
      }
    }
    const result = detectStaleSources(parsed.frontmatter, currentHashes)
    if (result.isStale !== parsed.frontmatter.stale) {
      const updated = serializePage(
        { ...parsed.frontmatter, stale: result.isStale },
        parsed.body
      )
      await writeFile(fullPath, updated)
    }
  }
}
