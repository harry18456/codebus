import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from 'node:fs'
import { execSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { runQuery } from '../../src/commands/query.js'
import type { LLMProvider, InvokeOptions, StreamEvent } from '../../src/infra/llm/types.js'

class FakeProvider implements LLMProvider {
  receivedMode: string | null = null
  receivedCwd: string | null = null
  async *invoke(opts: InvokeOptions): AsyncIterable<StreamEvent> {
    this.receivedMode = opts.mode
    this.receivedCwd = opts.cwd
    yield { kind: 'thought', text: 'searching wiki...' }
    yield { kind: 'done' }
  }
  cancel(): void {}
}

describe('runQuery', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-query-'))
    execSync('git init -q -b main', { cwd: dir })
    execSync('git config user.email "t@t.com"', { cwd: dir })
    execSync('git config user.name "T"', { cwd: dir })
    writeFileSync(join(dir, 'a.txt'), 'x')
    execSync('git add . && git commit -q -m init', { cwd: dir })
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('throws if .codebus/wiki/pages/ is missing (need --goal first)', async () => {
    await expect(
      runQuery({ repoRoot: dir, query: 'q', provider: new FakeProvider() })
    ).rejects.toThrow(/請先用 --goal/)
  })

  it('throws if .codebus/wiki/pages/ is empty', async () => {
    mkdirSync(join(dir, '.codebus', 'wiki', 'pages'), { recursive: true })
    await expect(
      runQuery({ repoRoot: dir, query: 'q', provider: new FakeProvider() })
    ).rejects.toThrow(/請先用 --goal/)
  })

  it('invokes provider with mode=query and cwd=.codebus/ when wiki has pages', async () => {
    mkdirSync(join(dir, '.codebus', 'wiki', 'pages'), { recursive: true })
    writeFileSync(join(dir, '.codebus', 'wiki', 'pages', 'a.md'), '# a')
    writeFileSync(join(dir, '.codebus', 'wiki', 'index.md'), '- [[a]]')
    writeFileSync(join(dir, '.codebus', 'CLAUDE.md'), 'schema')
    const provider = new FakeProvider()
    await runQuery({ repoRoot: dir, query: '結帳怎麼跑', provider })
    expect(provider.receivedMode).toBe('query')
    expect(provider.receivedCwd).toBe(join(dir, '.codebus'))
  })
})
