import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { checkRepoIsNotVault } from '../../../src/core/vault/sanity-check.js'

// Build a directory that passes looksLikeVault (CLAUDE.md / wiki / raw /
// goals.jsonl all present). Caller picks whether the basename is .codebus.
function makeVault(parent: string, name = '.codebus'): string {
  const root = join(parent, name)
  mkdirSync(join(root, 'wiki'), { recursive: true })
  mkdirSync(join(root, 'raw'), { recursive: true })
  writeFileSync(join(root, 'CLAUDE.md'), 'schema')
  writeFileSync(join(root, 'goals.jsonl'), '')
  return root
}

describe('checkRepoIsNotVault', () => {
  let tmp: string
  let fakeHome: string

  beforeEach(() => {
    tmp = mkdtempSync(join(tmpdir(), 'codebus-sanity-'))
    fakeHome = mkdtempSync(join(tmpdir(), 'codebus-home-'))
    vi.stubEnv('HOME', fakeHome)
    vi.stubEnv('USERPROFILE', fakeHome)
  })
  afterEach(() => {
    vi.unstubAllEnvs()
    rmSync(tmp, { recursive: true, force: true })
    rmSync(fakeHome, { recursive: true, force: true })
  })

  it('accepts a plain folder that has no vault markers', () => {
    const repo = join(tmp, 'normal-repo')
    mkdirSync(repo)
    writeFileSync(join(repo, 'README.md'), 'hi')
    expect(checkRepoIsNotVault(repo).ok).toBe(true)
  })

  it('rejects when --repo basename is .codebus', () => {
    const vault = makeVault(tmp)  // tmp/.codebus/ with markers
    const result = checkRepoIsNotVault(vault)
    expect(result.ok).toBe(false)
    expect(result.reason).toContain('codebus vault')
    expect(result.hint).toContain(tmp)
  })

  it('rejects when --repo basename is .codebus even without markers (defensive)', () => {
    // basename check fires before marker check so an empty .codebus dir
    // is also rejected — protects against half-init or freshly-mkdir'd vault
    const empty = join(tmp, '.codebus')
    mkdirSync(empty)
    expect(checkRepoIsNotVault(empty).ok).toBe(false)
  })

  it('rejects when path has all vault markers but different name', () => {
    const fakeNamedVault = makeVault(tmp, 'pretend-vault')
    const result = checkRepoIsNotVault(fakeNamedVault)
    expect(result.ok).toBe(false)
    expect(result.reason).toContain('vault')
  })

  it('rejects when --repo points INSIDE a vault (wiki subdir)', () => {
    const vault = makeVault(tmp)
    const wikiInside = join(vault, 'wiki')
    const result = checkRepoIsNotVault(wikiInside)
    expect(result.ok).toBe(false)
    expect(result.reason).toContain('inside a codebus vault')
    expect(result.hint).toContain(tmp)
  })

  it('rejects when --repo points at ~/.codebus user-global config dir', () => {
    mkdirSync(join(fakeHome, '.codebus'))
    const result = checkRepoIsNotVault(join(fakeHome, '.codebus'))
    expect(result.ok).toBe(false)
    expect(result.reason).toContain('user-global')
    expect(result.hint).toContain('config.yaml')
  })

  it('accepts a folder named .codebus.backup (similar but not vault)', () => {
    const looksClose = join(tmp, '.codebus.backup')
    mkdirSync(looksClose)
    expect(checkRepoIsNotVault(looksClose).ok).toBe(true)
  })

  it('accepts source repo that has .codebus/ as sibling subdir', () => {
    // Common case: user runs `codebus --repo /repo/` where /repo/.codebus/
    // already exists from prior init. The repo itself is not a vault.
    const repo = join(tmp, 'project')
    mkdirSync(repo)
    makeVault(repo)  // creates project/.codebus
    expect(checkRepoIsNotVault(repo).ok).toBe(true)
  })

  it('accepts a folder coincidentally named .codebus inside a non-vault tree', () => {
    // Edge: any folder called .codebus is treated as vault (defensive
    // basename rule); confirm we are NOT walking up from a non-.codebus
    // path and falsely claiming an ancestor is a vault.
    const sub = join(tmp, 'src', 'utils')
    mkdirSync(sub, { recursive: true })
    expect(checkRepoIsNotVault(sub).ok).toBe(true)
  })
})
