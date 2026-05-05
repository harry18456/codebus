import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, existsSync, readFileSync, writeFileSync } from 'node:fs'
import { execSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

describe('e2e: init', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-e2e-'))
    execSync('git init -q -b main', { cwd: dir })
    execSync('git config user.email "t@t.com"', { cwd: dir })
    execSync('git config user.name "T"', { cwd: dir })
    writeFileSync(join(dir, 'README.md'), 'hi')
    execSync('git add . && git commit -q -m init', { cwd: dir })
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('runs `codebus --repo <dir>` end-to-end and creates .codebus vault', () => {
    execSync(`npx tsx src/cli.ts --repo "${dir}"`, { stdio: 'pipe' })
    expect(existsSync(join(dir, '.codebus'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', '.git'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'CLAUDE.md'))).toBe(true)
    const gi = readFileSync(join(dir, '.gitignore'), 'utf8')
    expect(gi).toContain('.codebus')
  })
})
