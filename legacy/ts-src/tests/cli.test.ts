import { describe, it, expect } from 'vitest'
import { execSync } from 'node:child_process'

describe('cli', () => {
  it('--version prints version', () => {
    const out = execSync('npx tsx src/cli.ts --version').toString()
    expect(out).toContain('0.1.0')
  })

  it('--help mentions all 3 main flags', () => {
    const out = execSync('npx tsx src/cli.ts --help').toString()
    expect(out).toContain('--repo')
    expect(out).toContain('--goal')
    expect(out).toContain('--query')
  })
})
