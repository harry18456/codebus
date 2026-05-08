import { readdir, mkdir, copyFile, rm, readFile, stat } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { join, relative, sep } from 'node:path'

const ALWAYS_SKIP_AT_ROOT = new Set(['.codebus', '.git', '.env'])
const MAX_FILE_BYTES = 5 * 1024 * 1024  // 5 MiB

interface IgnoreMatcher {
  match(relPath: string): boolean
}

async function loadGitignore(repoRoot: string): Promise<IgnoreMatcher> {
  const gi = join(repoRoot, '.gitignore')
  if (!existsSync(gi)) return { match: () => false }
  const text = await readFile(gi, 'utf8')
  const patterns = text
    .split('\n')
    .map((s) => s.trim())
    .filter((s) => s && !s.startsWith('#'))
    .map((pat) => (pat.endsWith('/') ? pat.slice(0, -1) : pat))
  return {
    match(rel: string): boolean {
      const segments = rel.split('/')
      return patterns.some((pat) =>
        segments.includes(pat) || rel === pat || rel.startsWith(pat + '/')
      )
    }
  }
}

export async function syncRepoToRaw(repoRoot: string, rawDir: string): Promise<void> {
  if (existsSync(rawDir)) await rm(rawDir, { recursive: true, force: true })
  await mkdir(rawDir, { recursive: true })

  const ignore = await loadGitignore(repoRoot)

  async function walk(srcDir: string, dstDir: string): Promise<void> {
    const entries = await readdir(srcDir, { withFileTypes: true })
    for (const e of entries) {
      const srcPath = join(srcDir, e.name)
      const rel = relative(repoRoot, srcPath).split(sep).join('/')
      if (srcDir === repoRoot && ALWAYS_SKIP_AT_ROOT.has(e.name)) continue
      if (ignore.match(rel)) continue
      const dstPath = join(dstDir, e.name)
      if (e.isDirectory()) {
        await mkdir(dstPath, { recursive: true })
        await walk(srcPath, dstPath)
      } else if (e.isFile()) {
        const { size } = await stat(srcPath)
        if (size > MAX_FILE_BYTES) continue
        await copyFile(srcPath, dstPath)
      }
    }
  }

  await walk(repoRoot, rawDir)
}
