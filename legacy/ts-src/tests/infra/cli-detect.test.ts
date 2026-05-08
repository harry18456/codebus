import { describe, it, expect } from 'vitest'
import { checkClaudeCliAvailable, type ExecSyncFn } from '../../src/infra/cli-detect.js'

describe('checkClaudeCliAvailable', () => {
  it('returns ok when claude --version prints a semver-ish line', () => {
    const fakeExec: ExecSyncFn = () => Buffer.from('2.1.126 (Claude Code)\n')
    expect(checkClaudeCliAvailable(fakeExec)).toEqual({ ok: true })
  })

  it('returns ok for a minor-version-only output (e.g. 3.0)', () => {
    const fakeExec: ExecSyncFn = () => Buffer.from('3.0\n')
    expect(checkClaudeCliAvailable(fakeExec)).toEqual({ ok: true })
  })

  it('returns not-ok with install hint when execSync throws (claude not on PATH)', () => {
    const fakeExec: ExecSyncFn = () => { throw new Error('command not found') }
    const result = checkClaudeCliAvailable(fakeExec)
    expect(result.ok).toBe(false)
    expect(result.reason).toContain('找不到 claude CLI')
    expect(result.hint).toContain('npm install -g @anthropic-ai/claude-code')
  })

  it('returns not-ok with version-mismatch hint when output is not semver-like', () => {
    const fakeExec: ExecSyncFn = () => Buffer.from('garbage output\n')
    const result = checkClaudeCliAvailable(fakeExec)
    expect(result.ok).toBe(false)
    expect(result.reason).toContain('預期外')
    expect(result.hint).toContain('升級到 2.x')
  })

  it('handles string output (not just Buffer)', () => {
    const fakeExec: ExecSyncFn = () => '2.1.126'
    expect(checkClaudeCliAvailable(fakeExec).ok).toBe(true)
  })
})
