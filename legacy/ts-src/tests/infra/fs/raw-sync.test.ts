import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync, existsSync, readFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { syncRepoToRaw } from '../../../src/infra/fs/raw-sync.js'

describe('syncRepoToRaw', () => {
  let repo: string
  let rawCode: string

  beforeEach(() => {
    repo = mkdtempSync(join(tmpdir(), 'codebus-repo-'))
    rawCode = join(repo, '.codebus', 'raw', 'code')
    mkdirSync(join(repo, 'src'), { recursive: true })
    writeFileSync(join(repo, 'src', 'app.ts'), 'console.log("hi")')
    mkdirSync(join(repo, 'node_modules', 'lodash'), { recursive: true })
    writeFileSync(join(repo, 'node_modules', 'lodash', 'index.js'), '// big')
    mkdirSync(join(repo, '.git'))
    writeFileSync(join(repo, '.git', 'HEAD'), 'ref: refs/heads/main')
    mkdirSync(join(repo, '.codebus'))
    writeFileSync(join(repo, '.codebus', 'goals.jsonl'), '{}')
    writeFileSync(join(repo, '.gitignore'), 'node_modules\n')
    writeFileSync(join(repo, '.env'), 'SECRET=xxx')
  })
  afterEach(() => { rmSync(repo, { recursive: true, force: true }) })

  it('copies repo content into raw/code, excluding .codebus/, .git/, .env, and gitignored', async () => {
    await syncRepoToRaw(repo, rawCode)
    expect(existsSync(join(rawCode, 'src', 'app.ts'))).toBe(true)
    expect(readFileSync(join(rawCode, 'src', 'app.ts'), 'utf8')).toBe('console.log("hi")')
    expect(existsSync(join(rawCode, 'node_modules'))).toBe(false)
    expect(existsSync(join(rawCode, '.git'))).toBe(false)
    expect(existsSync(join(rawCode, '.codebus'))).toBe(false)
    expect(existsSync(join(rawCode, '.env'))).toBe(false)
  })

  it('clears existing raw/code before re-syncing (does not touch raw/ siblings)', async () => {
    mkdirSync(rawCode, { recursive: true })
    writeFileSync(join(rawCode, 'stale.txt'), 'old')
    mkdirSync(join(repo, '.codebus', 'raw', 'docs'), { recursive: true })
    writeFileSync(join(repo, '.codebus', 'raw', 'docs', 'spec.md'), 'user-managed')
    await syncRepoToRaw(repo, rawCode)
    expect(existsSync(join(rawCode, 'stale.txt'))).toBe(false)
    expect(existsSync(join(repo, '.codebus', 'raw', 'docs', 'spec.md'))).toBe(true)
  })
})
