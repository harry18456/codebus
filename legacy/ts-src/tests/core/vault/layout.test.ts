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
    expect(p.wikiConcepts).toMatch(/[/\\]wiki[/\\]concepts$/)
    expect(p.wikiEntities).toMatch(/[/\\]wiki[/\\]entities$/)
    expect(p.wikiModules).toMatch(/[/\\]wiki[/\\]modules$/)
    expect(p.wikiProcesses).toMatch(/[/\\]wiki[/\\]processes$/)
    expect(p.wikiSynthesis).toMatch(/[/\\]wiki[/\\]synthesis$/)
    expect(p.wikiPageFolders).toEqual([
      p.wikiConcepts, p.wikiEntities, p.wikiModules, p.wikiProcesses, p.wikiSynthesis
    ])
    expect(p.wikiTypeFolderMap.concept).toBe(p.wikiConcepts)
    expect(p.wikiTypeFolderMap.entity).toBe(p.wikiEntities)
    expect(p.wikiTypeFolderMap.module).toBe(p.wikiModules)
    expect(p.wikiTypeFolderMap.process).toBe(p.wikiProcesses)
    expect(p.wikiTypeFolderMap.synthesis).toBe(p.wikiSynthesis)
    // wiki/goals/ removed in wiki-taxonomy-realign — VaultPaths no longer
    // exposes a wikiGoals field. Per-goal narrative now lives in log.md.
    expect((p as Record<string, unknown>).wikiGoals).toBeUndefined()
    expect(p.output).toMatch(/[/\\]\.codebus[/\\]output$/)
    expect(p.lock).toMatch(/[/\\]\.codebus[/\\]\.lock$/)
  })
})
