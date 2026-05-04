import { describe, it, expect } from 'vitest'
import { vaultPaths } from '../../../src/core/vault/layout.js'

describe('vaultPaths', () => {
  it('returns all .codebus/ paths under given repo root', () => {
    const p = vaultPaths('/tmp/myrepo')
    expect(p.root).toMatch(/[/\\]tmp[/\\]myrepo[/\\]\.codebus$/)
    expect(p.git).toMatch(/[/\\]\.codebus[/\\]\.git$/)
    expect(p.gitignore).toMatch(/[/\\]\.codebus[/\\]\.gitignore$/)
    expect(p.goalsJsonl).toMatch(/[/\\]\.codebus[/\\]goals\.jsonl$/)
    expect(p.schemaMd).toMatch(/[/\\]\.codebus[/\\]CLAUDE\.md$/)
    expect(p.raw).toMatch(/[/\\]\.codebus[/\\]raw$/)
    expect(p.rawCode).toMatch(/[/\\]\.codebus[/\\]raw[/\\]code$/)
    expect(p.wiki).toMatch(/[/\\]\.codebus[/\\]wiki$/)
    expect(p.wikiOverview).toMatch(/[/\\]wiki[/\\]overview\.md$/)
    expect(p.wikiIndex).toMatch(/[/\\]wiki[/\\]index\.md$/)
    expect(p.wikiLog).toMatch(/[/\\]wiki[/\\]log\.md$/)
    expect(p.wikiPages).toMatch(/[/\\]wiki[/\\]pages$/)
    expect(p.wikiGoals).toMatch(/[/\\]wiki[/\\]goals$/)
    expect(p.output).toMatch(/[/\\]\.codebus[/\\]output$/)
    expect(p.lock).toMatch(/[/\\]\.codebus[/\\]\.lock$/)
  })
})
