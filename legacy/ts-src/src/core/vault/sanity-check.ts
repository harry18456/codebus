import { existsSync } from 'node:fs'
import { homedir } from 'node:os'
import { resolve, basename, join, dirname } from 'node:path'

// Files that together identify a folder as a codebus vault. All four
// must exist; user repos rarely have all of these by coincidence.
const VAULT_MARKERS = ['CLAUDE.md', 'wiki', 'raw', 'goals.jsonl']

function resolveHome(): string {
  return process.env.HOME ?? process.env.USERPROFILE ?? homedir()
}

function looksLikeVault(path: string): boolean {
  return VAULT_MARKERS.every((m) => existsSync(join(path, m)))
}

export interface VaultSanityResult {
  ok: boolean
  reason?: string
  hint?: string
}

// Catches user mistakes that would otherwise produce nested-vault chaos:
//   - --repo points at .codebus/ (basename or by structure)
//   - --repo points INSIDE a vault somewhere up the tree
//   - --repo points at the user-global ~/.codebus/ config dir
// Returns ok=true when path appears to be a real source repo.
export function checkRepoIsNotVault(repoRoot: string): VaultSanityResult {
  const resolved = resolve(repoRoot)

  // 1. user-global ~/.codebus/ config dir — never a source repo
  const globalDir = resolve(join(resolveHome(), '.codebus'))
  if (resolved === globalDir) {
    return {
      ok: false,
      reason: `--repo points at the user-global codebus config dir (${resolved}).`,
      hint: '~/.codebus/ holds your config.yaml, not source code. Pass --repo /path/to/your/source/repo.'
    }
  }

  // 2. repoRoot is itself a vault (named .codebus OR has marker structure)
  if (basename(resolved) === '.codebus' || looksLikeVault(resolved)) {
    return {
      ok: false,
      reason: `--repo points at a codebus vault (${resolved}), not a source repo.`,
      hint: `Vaults live AT the source repo's .codebus/ subdir. Pass --repo ${dirname(resolved)} (the parent).`
    }
  }

  // 3. repoRoot is INSIDE a vault somewhere up the tree (walk ancestors)
  let cur = dirname(resolved)
  while (cur !== dirname(cur)) {
    if (basename(cur) === '.codebus' && looksLikeVault(cur)) {
      return {
        ok: false,
        reason: `--repo (${resolved}) is inside a codebus vault at ${cur}.`,
        hint: `Pass --repo ${dirname(cur)} (the source repo containing the vault).`
      }
    }
    cur = dirname(cur)
  }

  return { ok: true }
}
