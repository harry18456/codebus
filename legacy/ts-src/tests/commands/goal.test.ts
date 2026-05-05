import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync, readFileSync, existsSync, mkdirSync } from 'node:fs'
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

// Simulates an agent that ACTUALLY writes a wiki page (real Claude would
// emit Write tool_use events whose effect is files on disk; FakeProvider
// can't trigger Claude tools, so we write directly to mimic the side
// effect for the wikiChanged=true case).
class WritingFakeProvider implements LLMProvider {
  async *invoke(opts: InvokeOptions): AsyncIterable<StreamEvent> {
    const pagesDir = join(opts.cwd, 'wiki', 'concepts')
    mkdirSync(pagesDir, { recursive: true })
    writeFileSync(
      join(pagesDir, 'fake.md'),
      `---\ntitle: Fake\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-04'\nupdated: '2026-05-04'\nrelated: []\nstale: false\n---\nbody`
    )
    yield { kind: 'thought', text: 'wrote a page' }
    yield { kind: 'done' }
  }
  cancel(): void {}
}

// Replaces the entire wiki/ directory with a regular file. Forces
// lintWiki to throw at the §4 wiki/ root readdir — existsSync(wikiRoot)
// returns true (it's a file), then readdir throws ENOTDIR. enrich and
// stale-detect iterate the 5 type folders inside wiki/; existsSync of
// wiki/<type>/ returns false once wiki/ is a file, so both phases skip
// without error. The fault is isolated to lintWiki, matching the spec's
// "lint throws during goal execution" scenario.
//
// (Pre wiki-taxonomy-realign this sabotaged wiki/goals/ instead. That
// directory was removed in the realign, so a different lintWiki throw
// vector is needed.)
class SabotageGoalsProvider implements LLMProvider {
  async *invoke(opts: InvokeOptions): AsyncIterable<StreamEvent> {
    const wikiDir = join(opts.cwd, 'wiki')
    rmSync(wikiDir, { recursive: true, force: true })
    writeFileSync(wikiDir, 'sabotaged')
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

  it('returns wikiChanged=false when agent does not write any wiki content', async () => {
    const result = await runGoal({
      repoRoot: dir,
      goal: 'agent will refuse this',
      provider: new FakeProvider()
    })
    expect(result.wikiChanged).toBe(false)
  })

  it('returns wikiChanged=true when agent writes a wiki page', async () => {
    const result = await runGoal({
      repoRoot: dir,
      goal: '建立 fake page',
      provider: new WritingFakeProvider()
    })
    expect(result.wikiChanged).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'wiki', 'concepts', 'fake.md'))).toBe(true)
  })

  it('returns lint: null and still auto-commits when lintWiki throws', async () => {
    // Soft-mode contract: lint failure must not abort ingest. SabotageGoalsProvider
    // breaks wiki/goals/ to force lintWiki to throw — runGoal must swallow,
    // return lint: null, and continue to auto-commit.
    const result = await runGoal({
      repoRoot: dir,
      goal: 'sabotage lint',
      provider: new SabotageGoalsProvider()
    })
    expect(result.lint).toBe(null)
    // goals.jsonl was appended (proves the run reached the body) AND
    // auto-commit ran (file is in the nested-git working tree).
    const goalsJsonl = readFileSync(join(dir, '.codebus', 'goals.jsonl'), 'utf8')
    expect(goalsJsonl).toContain('sabotage lint')
  })
})
