import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { loadGlobalConfig } from '../../src/infra/global-config.js'

describe('loadGlobalConfig', () => {
  let home: string
  beforeEach(() => {
    home = mkdtempSync(join(tmpdir(), 'codebus-home-'))
    vi.stubEnv('HOME', home)
    vi.stubEnv('USERPROFILE', home)
  })
  afterEach(() => {
    vi.unstubAllEnvs()
    rmSync(home, { recursive: true, force: true })
  })

  it('returns empty config when ~/.codebus/config.yaml does not exist', async () => {
    const cfg = await loadGlobalConfig()
    expect(cfg).toEqual({})
  })

  it('parses valid emoji setting', async () => {
    mkdirSync(join(home, '.codebus'))
    writeFileSync(join(home, '.codebus', 'config.yaml'), 'emoji: off\n')
    const cfg = await loadGlobalConfig()
    expect(cfg.emoji).toBe('off')
  })

  it('returns empty + warns on invalid yaml', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    mkdirSync(join(home, '.codebus'))
    writeFileSync(join(home, '.codebus', 'config.yaml'), '{{{ broken yaml')
    const cfg = await loadGlobalConfig()
    expect(cfg).toEqual({})
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })

  it('silently ignores unknown fields (forward-compat for phase 2)', async () => {
    mkdirSync(join(home, '.codebus'))
    writeFileSync(
      join(home, '.codebus', 'config.yaml'),
      'emoji: on\ndefault_provider: anthropic-sdk\napi_keys:\n  anthropic: sk-xxx\n'
    )
    const cfg = await loadGlobalConfig()
    expect(cfg.emoji).toBe('on')
    expect((cfg as Record<string, unknown>).default_provider).toBeUndefined()
  })

  it('rejects unknown emoji value with warning', async () => {
    mkdirSync(join(home, '.codebus'))
    writeFileSync(join(home, '.codebus', 'config.yaml'), 'emoji: notvalid\n')
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    const cfg = await loadGlobalConfig()
    expect(cfg.emoji).toBeUndefined()
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })
})
