import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { acquireLock, releaseLock } from '../../../src/core/vault/lock.js'

describe('lock', () => {
  let dir: string
  beforeEach(() => { dir = mkdtempSync(join(tmpdir(), 'codebus-lock-')) })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('acquires lock by writing pid file', async () => {
    const lockPath = join(dir, '.lock')
    const handle = await acquireLock(lockPath)
    expect(existsSync(lockPath)).toBe(true)
    await releaseLock(handle)
    expect(existsSync(lockPath)).toBe(false)
  })

  it('throws when lock already held', async () => {
    const lockPath = join(dir, '.lock')
    const h1 = await acquireLock(lockPath)
    await expect(acquireLock(lockPath)).rejects.toThrow(/already held/)
    await releaseLock(h1)
  })

  it('release is idempotent', async () => {
    const lockPath = join(dir, '.lock')
    const h = await acquireLock(lockPath)
    await releaseLock(h)
    await releaseLock(h)
    expect(existsSync(lockPath)).toBe(false)
  })
})
