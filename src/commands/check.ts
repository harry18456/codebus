import { existsSync } from 'node:fs'
import { vaultPaths } from '../core/vault/layout.js'
import { lintWiki, type LintResult } from '../core/wiki/lint.js'

export interface RunCheckOptions {
  repoRoot: string
}

export async function runCheck(opts: RunCheckOptions): Promise<LintResult> {
  const p = vaultPaths(opts.repoRoot)
  if (!existsSync(p.root)) {
    throw new Error(
      `No codebus vault at ${p.root} — ` +
      `run \`codebus --repo ${opts.repoRoot}\` first to init, ` +
      `or \`codebus --repo ${opts.repoRoot} --goal "..."\` to ingest`
    )
  }
  return await lintWiki(p.root)
}
