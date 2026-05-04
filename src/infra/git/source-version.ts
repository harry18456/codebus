import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { simpleGit } from 'simple-git'

export interface SourceVersion {
  commit: string | null
  uncommitted: boolean
}

export async function getSourceVersion(repoRoot: string): Promise<SourceVersion> {
  if (!existsSync(join(repoRoot, '.git'))) {
    return { commit: null, uncommitted: false }
  }
  const git = simpleGit(repoRoot)
  const commit = (await git.revparse(['HEAD'])).trim()
  const status = await git.status()
  return {
    commit,
    uncommitted: !status.isClean()
  }
}
