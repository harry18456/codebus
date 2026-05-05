import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync, mkdirSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { sha256File, listFilesRecursive } from '../../../src/infra/fs/file-ops.js'

describe('sha256File', () => {
  let dir: string
  beforeEach(() => { dir = mkdtempSync(join(tmpdir(), 'codebus-fs-')) })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('computes sha256 of file content', async () => {
    const f = join(dir, 'a.txt')
    writeFileSync(f, 'hello')
    const hash = await sha256File(f)
    expect(hash).toBe('2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824')
  })
})

describe('listFilesRecursive', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-fs-'))
    writeFileSync(join(dir, 'a.txt'), '')
    const sub = join(dir, 'sub')
    mkdirSync(sub)
    writeFileSync(join(sub, 'b.txt'), '')
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('lists all files recursively (paths relative to root, forward slashes)', async () => {
    const files = await listFilesRecursive(dir)
    expect(files.sort()).toEqual(['a.txt', 'sub/b.txt'])
  })
})
