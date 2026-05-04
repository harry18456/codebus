import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync, readFileSync, existsSync } from 'node:fs'
import { execSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { runGoal } from '../../src/commands/goal.js'
import type { LLMProvider, InvokeOptions, StreamEvent } from '../../src/infra/llm/types.js'

class FakeProvider implements LLMProvider {
  receivedCwd: string | null = null
  async *invoke(opts: InvokeOptions): AsyncIterable<StreamEvent> {
    this.receivedCwd = opts.cwd
    yield { kind: 'thought', text: 'analyzing...' }
    yield { kind: 'done' }
  }
  cancel(): void {}
}

describe('runGoal', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-goal-'))
    execSync('git init -q -b main', { cwd: dir })
    execSync('git config user.email "t@t.com"', { cwd: dir })
    execSync('git config user.name "T"', { cwd: dir })
    writeFileSync(join(dir, 'app.ts'), 'console.log("hi")')
    // Pre-commit .gitignore with .codebus so runInit's gitignore mutation
    // is a no-op (otherwise the side-effect would dirty the working tree
    // and uncommitted=true even on a "clean" repo).
    writeFileSync(join(dir, '.gitignore'), '.codebus\n')
    execSync('git add . && git commit -q -m init', { cwd: dir })
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('runs init if needed, syncs raw, records goal, invokes provider, commits', async () => {
    const provider = new FakeProvider()
    const events: StreamEvent[] = []
    await runGoal({
      repoRoot: dir,
      goal: '了解 app.ts',
      provider,
      onEvent: (e) => events.push(e)
    })

    expect(existsSync(join(dir, '.codebus'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'raw', 'code', 'app.ts'))).toBe(true)
    const goalsJsonl = readFileSync(join(dir, '.codebus', 'goals.jsonl'), 'utf8')
    expect(goalsJsonl).toContain('了解 app.ts')
    expect(goalsJsonl).toContain('"uncommitted":false')
    expect(events.length).toBeGreaterThan(0)
    // Spike E sandbox: spawn cwd MUST be vault root, not source repo root.
    expect(provider.receivedCwd).toBe(join(dir, '.codebus'))
  })

  it('records uncommitted=true when working tree has changes', async () => {
    writeFileSync(join(dir, 'app.ts'), 'changed')
    await runGoal({ repoRoot: dir, goal: 'g', provider: new FakeProvider() })
    const goalsJsonl = readFileSync(join(dir, '.codebus', 'goals.jsonl'), 'utf8')
    expect(goalsJsonl).toContain('"uncommitted":true')
  })
})
