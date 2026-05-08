import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { execSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { getSourceVersion } from '../../../src/infra/git/source-version.js'

describe('getSourceVersion', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-srcver-'))
    execSync('git init -q -b main', { cwd: dir })
    execSync('git config user.email "t@t.com"', { cwd: dir })
    execSync('git config user.name "T"', { cwd: dir })
    writeFileSync(join(dir, 'a.txt'), 'hello')
    execSync('git add . && git commit -q -m init', { cwd: dir })
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('returns commit hash and clean=true on clean repo', async () => {
    const v = await getSourceVersion(dir)
    expect(v.commit).toMatch(/^[0-9a-f]{40}$/)
    expect(v.uncommitted).toBe(false)
  })

  it('returns uncommitted=true when working tree has changes', async () => {
    writeFileSync(join(dir, 'a.txt'), 'changed')
    const v = await getSourceVersion(dir)
    expect(v.uncommitted).toBe(true)
  })

  it('returns commit=null when path is not a git repo', async () => {
    const nonGit = mkdtempSync(join(tmpdir(), 'codebus-nongit-'))
    const v = await getSourceVersion(nonGit)
    expect(v.commit).toBe(null)
    expect(v.uncommitted).toBe(false)
    rmSync(nonGit, { recursive: true, force: true })
  })
})
