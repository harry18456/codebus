import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { runCheck } from '../../src/commands/check.js'

describe('runCheck', () => {
  let dir: string
  beforeEach(() => { dir = mkdtempSync(join(tmpdir(), 'codebus-check-')) })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('throws helpful error when .codebus/ does not exist', async () => {
    await expect(runCheck({ repoRoot: dir })).rejects.toThrow(/No codebus vault/)
  })

  it('returns lintResult on a valid vault', async () => {
    const vault = join(dir, '.codebus')
    for (const f of ['concepts', 'entities', 'modules', 'processes', 'synthesis']) {
      mkdirSync(join(vault, 'wiki', f), { recursive: true })
    }
    writeFileSync(join(vault, 'wiki', 'overview.md'), '# X')
    writeFileSync(join(vault, 'wiki', 'index.md'), '# X')
    writeFileSync(join(vault, 'wiki', 'log.md'), '# X')
    const result = await runCheck({ repoRoot: dir })
    expect(result.errorCount).toBe(0)
  })

  it('returns errorCount > 0 when vault has error-severity issues (drives exit code 1)', async () => {
    // cli.ts:104 maps `result.errorCount > 0 ? 1 : 0` to process exit code.
    // Surface that contract at the unit level — runCheck must report a
    // non-zero errorCount on a vault containing any error-severity issue.
    const vault = join(dir, '.codebus')
    for (const f of ['concepts', 'entities', 'modules', 'processes', 'synthesis']) {
      mkdirSync(join(vault, 'wiki', f), { recursive: true })
    }
    writeFileSync(join(vault, 'wiki', 'overview.md'), '# X')
    writeFileSync(join(vault, 'wiki', 'index.md'), '# X')
    writeFileSync(join(vault, 'wiki', 'log.md'), '# X')
    writeFileSync(join(vault, 'wiki', 'concepts', 'broken.md'), '---\ninvalid yaml [[\n---\nbody')
    const result = await runCheck({ repoRoot: dir })
    expect(result.errorCount).toBeGreaterThan(0)
  })
})
