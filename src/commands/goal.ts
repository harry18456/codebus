import { existsSync } from 'node:fs'
import { appendFile, readdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { vaultPaths } from '../core/vault/layout.js'
import { acquireLock, releaseLock } from '../core/vault/lock.js'
import { syncRepoToRaw } from '../infra/fs/raw-sync.js'
import { sha256File } from '../infra/fs/file-ops.js'
import { getSourceVersion } from '../infra/git/source-version.js'
import { autoCommit } from '../infra/git/nested-repo.js'
import { simpleGit } from 'simple-git'
import { parsePage, serializePage } from '../core/wiki/frontmatter.js'
import { detectStaleSources } from '../core/wiki/stale-detect.js'
import { utcTodayISO } from '../core/wiki/date.js'
import { lintWiki, type LintResult } from '../core/wiki/lint.js'
import { runInit } from './init.js'
import type { LLMProvider, StreamEvent } from '../infra/llm/types.js'

export interface RunGoalOptions {
  repoRoot: string
  goal: string
  provider: LLMProvider
  onEvent?: (e: StreamEvent) => void
}

export interface RunGoalResult {
  // True if the run produced new wiki content (nested-git HEAD advanced).
  // False when agent ran successfully but didn't write/edit anything —
  // typically when agent self-judged the goal as not wiki-shaped (e.g.
  // "create test.md") or refused for schema reasons. Caller uses this
  // to show an honest completion banner instead of the misleading
  // "wiki 已生成" / "Obsidian 開" hint.
  wikiChanged: boolean
  // Lint result captured AFTER enrich/stale-detect, BEFORE autoCommit.
  // Soft mode: lint never blocks commit; caller decides how to surface
  // the result (banner one-liner / full report). null when lint failed
  // hard (e.g. wiki/ doesn't exist); caller treats as "no issues to
  // report".
  lint: LintResult | null
}

// Check whether the wiki/ subtree has any uncommitted changes (new pages,
// edited pages, deleted pages). Used to distinguish "agent produced wiki
// content" from "agent ran but only goals.jsonl / raw sync changed".
// Comparing nested-git HEAD SHAs would be wrong: autoCommit also picks up
// the goals.jsonl append (always happens) and would falsely report change
// even when wiki/ is untouched.
async function hasWikiChanges(vaultRoot: string): Promise<boolean> {
  const out = await simpleGit(vaultRoot).raw(['status', '--porcelain', 'wiki/'])
  return out.trim().length > 0
}

export async function runGoal(opts: RunGoalOptions): Promise<RunGoalResult> {
  const p = vaultPaths(opts.repoRoot)

  if (!existsSync(p.root)) await runInit(opts.repoRoot)

  const lock = await acquireLock(p.lock)
  let wikiChanged = false
  let lint: LintResult | null = null
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
    // Soft auto-lint: never blocks commit, just captures result for caller
    // to surface. Phase 2 may add hard mode (--strict) and LLM correction
    // loop — both reuse this same lintWiki call; only the response differs.
    try {
      lint = await lintWiki(p.root)
    } catch { /* lint failure must not break ingest — best effort */ }
    wikiChanged = await hasWikiChanges(p.root)
    await autoCommit(p.root, `wiki: ${opts.goal}`)
  } finally {
    await releaseLock(lock)
  }
  return { wikiChanged, lint }
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
