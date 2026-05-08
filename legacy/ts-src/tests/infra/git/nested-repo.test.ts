import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { initNestedRepo, autoCommit } from '../../../src/infra/git/nested-repo.js'

describe('nested-repo', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-nested-'))
    mkdirSync(join(dir, '.codebus'))
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('initializes nested git repo at .codebus/.git', async () => {
    await initNestedRepo(join(dir, '.codebus'))
    expect(existsSync(join(dir, '.codebus', '.git'))).toBe(true)
  })

  it('autoCommit stages all files and commits with given message', async () => {
    await initNestedRepo(join(dir, '.codebus'))
    writeFileSync(join(dir, '.codebus', 'README.md'), 'hi')
    const sha = await autoCommit(join(dir, '.codebus'), 'wiki: test')
    expect(sha).toMatch(/^[0-9a-f]{40}$/)
  })

  it('autoCommit returns existing HEAD when working tree is clean', async () => {
    await initNestedRepo(join(dir, '.codebus'))
    writeFileSync(join(dir, '.codebus', 'a.md'), 'a')
    const sha1 = await autoCommit(join(dir, '.codebus'), 'first')
    const sha2 = await autoCommit(join(dir, '.codebus'), 'no-op')
    expect(sha2).toBe(sha1)
  })

  it('initNestedRepo is idempotent (no-op when .git exists)', async () => {
    await initNestedRepo(join(dir, '.codebus'))
    await initNestedRepo(join(dir, '.codebus'))
    expect(existsSync(join(dir, '.codebus', '.git'))).toBe(true)
  })
})
