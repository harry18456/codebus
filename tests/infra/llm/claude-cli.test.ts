import { describe, it, expect } from 'vitest'
import { EventEmitter } from 'node:events'
import { Readable } from 'node:stream'
import { ClaudeCliProvider } from '../../../src/infra/llm/claude-cli.js'

describe('ClaudeCliProvider', () => {
  it('builds correct argv for ingest mode (acceptEdits + no --add-dir)', () => {
    const p = new ClaudeCliProvider({ binary: 'claude' })
    const argv = p.buildArgv({ mode: 'ingest', vaultRoot: '/tmp/.codebus' })
    expect(argv).toEqual([
      '-p',
      '--output-format', 'stream-json',
      '--input-format', 'stream-json',
      '--verbose',
      '--permission-mode', 'acceptEdits',
      '--disallowedTools', 'Bash,WebFetch,WebSearch'
    ])
    expect(argv).not.toContain('--add-dir')
  })

  it('builds correct argv for query mode (acceptEdits + Write/Edit hard-disabled)', () => {
    const p = new ClaudeCliProvider({ binary: 'claude' })
    const argv = p.buildArgv({ mode: 'query', vaultRoot: '/tmp/.codebus' })
    const dIdx = argv.indexOf('--disallowedTools')
    expect(argv[dIdx + 1]).toBe('Bash,WebFetch,WebSearch,Write,Edit')
    expect(argv).toContain('--permission-mode')
    expect(argv).not.toContain('--add-dir')
  })

  it('detects OAuth failure from non-zero exit + auth keyword in stderr', () => {
    const p = new ClaudeCliProvider({ binary: 'claude' })
    expect(p.classifyExit(1, 'unauthenticated: please run `claude` to login')).toMatchObject({
      kind: 'oauth-needed'
    })
    expect(p.classifyExit(1, 'token expired')).toMatchObject({ kind: 'oauth-needed' })
    expect(p.classifyExit(1, 'random failure')).toMatchObject({ kind: 'generic-error' })
    expect(p.classifyExit(0, '')).toMatchObject({ kind: 'success' })
  })

  it('passes opts.cwd to spawn (sandbox isolation per spec §3.2)', async () => {
    const seen: { cwd?: string } = {}
    const fakeChild = new EventEmitter() as any
    fakeChild.stdout = Readable.from([])             // empty readable
    fakeChild.stderr = new EventEmitter()
    fakeChild.stdin = { write: () => {}, end: () => {} }
    fakeChild.killed = false
    fakeChild.kill = () => {}

    const fakeSpawn: any = (_bin: string, _args: string[], opts: any) => {
      seen.cwd = opts?.cwd
      setImmediate(() => fakeChild.emit('exit', 0))
      return fakeChild
    }

    const p = new ClaudeCliProvider({ binary: 'claude', spawn: fakeSpawn })
    const it = p.invoke({
      systemPrompt: '', userMessage: '', mode: 'ingest',
      cwd: '/tmp/myrepo/.codebus', vaultRoot: '/tmp/myrepo/.codebus'
    })
    for await (const _ev of it) { /* drain */ }
    expect(seen.cwd).toBe('/tmp/myrepo/.codebus')
  })
})
