import { describe, it, expect } from 'vitest'
import { EventEmitter } from 'node:events'
import { Readable } from 'node:stream'
import { ClaudeCliProvider } from '../../../src/infra/llm/claude-cli.js'

describe('ClaudeCliProvider', () => {
  it('builds correct argv for ingest mode (--tools restricts toolset, --allowedTools auto-approves)', () => {
    const p = new ClaudeCliProvider({ binary: 'claude' })
    const argv = p.buildArgv({ mode: 'ingest', vaultRoot: '/tmp/.codebus' })
    expect(argv).toEqual([
      '-p',
      '--output-format', 'stream-json',
      '--input-format', 'stream-json',
      '--verbose',
      '--permission-mode', 'acceptEdits',
      '--tools', 'Read,Glob,Grep,Write,Edit',
      '--allowedTools', 'Read,Glob,Grep,Write,Edit'
    ])
    expect(argv).not.toContain('--add-dir')
    expect(argv).not.toContain('--disallowedTools')
  })

  it('builds correct argv for query mode (read-only — no Write/Edit in either flag)', () => {
    const p = new ClaudeCliProvider({ binary: 'claude' })
    const argv = p.buildArgv({ mode: 'query', vaultRoot: '/tmp/.codebus' })
    const tIdx = argv.indexOf('--tools')
    const aIdx = argv.indexOf('--allowedTools')
    expect(argv[tIdx + 1]).toBe('Read,Glob,Grep')
    expect(argv[aIdx + 1]).toBe('Read,Glob,Grep')
    expect(argv).toContain('--permission-mode')
    expect(argv).not.toContain('--add-dir')
    expect(argv).not.toContain('--disallowedTools')
  })

  it('toolset excludes future-leak vectors by design (no Bash, no AskUserQuestion, no Task, no MCP)', () => {
    const p = new ClaudeCliProvider({ binary: 'claude' })
    const argv = p.buildArgv({ mode: 'ingest', vaultRoot: '/tmp/.codebus' })
    // --tools is the lever that actually restricts the toolset
    // (--allowedTools is auto-approval only).
    const tIdx = argv.indexOf('--tools')
    const tools = (argv[tIdx + 1] ?? '').split(',')
    for (const banned of ['Bash', 'WebFetch', 'WebSearch', 'AskUserQuestion', 'Task', 'NotebookEdit', 'TodoWrite', 'SlashCommand', 'BashOutput', 'KillBash']) {
      expect(tools).not.toContain(banned)
    }
  })

  it('argv has --tools strictly before --allowedTools (--tools is the gate, --allowedTools the auto-approver)', () => {
    const p = new ClaudeCliProvider({ binary: 'claude' })
    const argv = p.buildArgv({ mode: 'ingest', vaultRoot: '/tmp/.codebus' })
    expect(argv.indexOf('--tools')).toBeLessThan(argv.indexOf('--allowedTools'))
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
