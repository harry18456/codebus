import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, existsSync, readFileSync, writeFileSync } from 'node:fs'
import { execSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { runInit } from '../../src/commands/init.js'

describe('runInit', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-init-'))
    execSync('git init -q -b main', { cwd: dir })
    execSync('git config user.email "t@t.com"', { cwd: dir })
    execSync('git config user.name "T"', { cwd: dir })
    writeFileSync(join(dir, 'README.md'), 'hi')
    execSync('git add . && git commit -q -m init', { cwd: dir })
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('creates .codebus/ with all subdirs and files', async () => {
    await runInit(dir)
    expect(existsSync(join(dir, '.codebus'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', '.git'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'CLAUDE.md'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'goals.jsonl'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'wiki', 'concepts'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'wiki', 'entities'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'wiki', 'modules'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'wiki', 'processes'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'wiki', 'synthesis'))).toBe(true)
    // wiki/goals/ removed in wiki-taxonomy-realign — init must NOT create
    // it (per-goal narrative now lives in log.md, not separate guide files).
    expect(existsSync(join(dir, '.codebus', 'wiki', 'goals'))).toBe(false)
    expect(existsSync(join(dir, '.codebus', 'raw'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'raw', 'code'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'output'))).toBe(true)
  })

  it('adds .codebus to source repo .gitignore (creating it if missing)', async () => {
    await runInit(dir)
    const gi = readFileSync(join(dir, '.gitignore'), 'utf8')
    expect(gi).toContain('.codebus')
  })

  it('does not duplicate .codebus entry if already in .gitignore', async () => {
    writeFileSync(join(dir, '.gitignore'), 'node_modules\n.codebus\n')
    await runInit(dir)
    const gi = readFileSync(join(dir, '.gitignore'), 'utf8')
    const matches = gi.match(/^\.codebus$/gm) ?? []
    expect(matches.length).toBe(1)
  })

  it('is idempotent — running twice does not error', async () => {
    await runInit(dir)
    await runInit(dir)
    expect(existsSync(join(dir, '.codebus'))).toBe(true)
  })

  it('skips .gitignore mutation when source repo is not git', async () => {
    const nonGit = mkdtempSync(join(tmpdir(), 'codebus-nongit-'))
    await runInit(nonGit)
    expect(existsSync(join(nonGit, '.gitignore'))).toBe(false)
    expect(existsSync(join(nonGit, '.codebus'))).toBe(true)
    rmSync(nonGit, { recursive: true, force: true })
  })

  it('preserves user-modified CLAUDE.md on re-init', async () => {
    await runInit(dir)
    writeFileSync(join(dir, '.codebus', 'CLAUDE.md'), 'user customization')
    await runInit(dir)
    const schema = readFileSync(join(dir, '.codebus', 'CLAUDE.md'), 'utf8')
    expect(schema).toBe('user customization')
  })

  it('writes .codebus/.gitignore with .lock + raw/code/ + .obsidian/', async () => {
    await runInit(dir)
    const gi = readFileSync(join(dir, '.codebus', '.gitignore'), 'utf8')
    expect(gi).toContain('.lock')
    expect(gi).toContain('raw/code/')
    expect(gi).toContain('**/.obsidian/')
  })

  it('appends missing required lines to existing .codebus/.gitignore', async () => {
    // Simulate a vault initialized by an earlier codebus version that
    // didn't include .obsidian/ in the ignore list.
    const codebusDir = join(dir, '.codebus')
    require('node:fs').mkdirSync(codebusDir)
    writeFileSync(join(codebusDir, '.gitignore'), '.lock\nraw/code/\n')
    await runInit(dir)
    const gi = readFileSync(join(codebusDir, '.gitignore'), 'utf8')
    expect(gi).toContain('.lock')
    expect(gi).toContain('raw/code/')
    expect(gi).toContain('**/.obsidian/')
    // Should not duplicate existing entries
    expect((gi.match(/^\.lock$/gm) ?? []).length).toBe(1)
    expect((gi.match(/^raw\/code\/$/gm) ?? []).length).toBe(1)
  })

  it('merge is idempotent (re-init does not duplicate entries)', async () => {
    await runInit(dir)
    await runInit(dir)
    const gi = readFileSync(join(dir, '.codebus', '.gitignore'), 'utf8')
    expect((gi.match(/^\*\*\/\.obsidian\/$/gm) ?? []).length).toBe(1)
  })
})
