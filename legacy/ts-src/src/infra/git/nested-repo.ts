import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { simpleGit } from 'simple-git'

export async function initNestedRepo(vaultRoot: string): Promise<void> {
  if (existsSync(join(vaultRoot, '.git'))) return
  const git = simpleGit(vaultRoot)
  await git.init(['-b', 'main'])
  await git.addConfig('user.email', 'codebus@local')
  await git.addConfig('user.name', 'codebus')
}

export async function autoCommit(vaultRoot: string, message: string): Promise<string> {
  const git = simpleGit(vaultRoot)
  await git.add('-A')
  const status = await git.status()
  if (status.isClean()) {
    return (await git.revparse(['HEAD'])).trim()
  }
  const result = await git.commit(message)
  return result.commit
}
