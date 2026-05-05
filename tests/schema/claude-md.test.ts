import { describe, it, expect } from 'vitest'
import { CODEBUS_SCHEMA_MARKDOWN } from '../../src/schema/claude-md.js'

describe('CODEBUS_SCHEMA_MARKDOWN', () => {
  it('contains SPDX license header', () => {
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('SPDX-License-Identifier: MIT')
  })

  it('contains all 12 schema sections', () => {
    const sections = [
      'Your Role', 'Workspace Layout', 'Wiki Structure',
      'Workflow per Goal', 'Page Conflict', 'Frontmatter Schema',
      'WikiLinks', 'Source', 'Stopping Criteria',
      'Failure Modes', 'Output Format', 'Workflow per Query'
    ]
    for (const s of sections) {
      expect(CODEBUS_SCHEMA_MARKDOWN).toContain(s)
    }
  })

  it('warns LLM about wikilink YAML quoting requirement', () => {
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('"[[')
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/quote|引號|MUST quote/i)
  })

  it('instructs agent to fill only sources[].path (not sha256/at_commit)', () => {
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/only fill.*path/i)
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/sha256.*auto-fill/i)
  })

  it('specifies UTC date convention', () => {
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('UTC YYYY-MM-DD')
  })

  it('contains §4.0 out-of-scope detection sub-section', () => {
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('Out-of-scope detection')
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('In-scope** if ANY of:')
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('Out-of-scope** otherwise')
  })

  it('contains §4.0.1 STOP rules forbidding no-op record creation', () => {
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('If out-of-scope: STOP')
    // Must explicitly forbid mutating log/index/type-folder pages for noop.
    // wiki/goals/ and wiki/overview.md were removed from STOP-rules in
    // wiki-taxonomy-realign — agent already won't create what no longer
    // has schema-mandated semantics.
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('No `wiki/log.md` append')
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('No `wiki/index.md` modification')
    expect(CODEBUS_SCHEMA_MARKDOWN).not.toContain('no "no-op record" goal-guide')
    expect(CODEBUS_SCHEMA_MARKDOWN).not.toContain('No `wiki/overview.md` update')
  })

  it('does NOT mention wiki/overview.md as a named root file', () => {
    // Post wiki-taxonomy-realign: overview is a synthesis page type, not
    // a wiki/ root special. Schema §3 should describe overview as a kind
    // of synthesis page, not as a strictly-named file at wiki/ root.
    expect(CODEBUS_SCHEMA_MARKDOWN).not.toMatch(/`wiki\/overview\.md`\s*—/)
    expect(CODEBUS_SCHEMA_MARKDOWN).not.toMatch(/rewrite each run/i)
  })

  it('does NOT mention wiki/goals/ as a schema-managed directory', () => {
    // Post wiki-taxonomy-realign: per-goal reading guides are gone; the
    // narrative is folded into the wiki/log.md entry. Schema must not
    // describe wiki/goals/<slug>.md as a step or named directory.
    expect(CODEBUS_SCHEMA_MARKDOWN).not.toMatch(/wiki\/goals\/<slug>\.md/)
    expect(CODEBUS_SCHEMA_MARKDOWN).not.toMatch(/per-goal reading guide/i)
    expect(CODEBUS_SCHEMA_MARKDOWN).not.toMatch(/Guide:\s*write\s*wiki\/goals/i)
  })

  it('describes the log step as carrying narrative coverage', () => {
    // Step 6 (Log) absorbs the goal-guide narrative: not just a single
    // line, but covered pages + reading suggestion + key takeaways.
    // Pattern is markdown-bold tolerant: matches `Log:` or `**Log**:`.
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/Log\**:[^\n]*chronological[^\n]*entry/i)
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/covered\s+pages/i)
  })

  it('treats type folders as organizational hint, not strict mandate', () => {
    // Post wiki-taxonomy-realign: 5 folders are still pre-created and
    // recommended, but schema must not present folder/type matching as
    // a hard contract; frontmatter `type` is the authoritative metadata.
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/organizational hint/i)
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/frontmatter\s+`?type`?\s+is\s+(?:the\s+)?authoritative/i)
  })

  it('explicitly references the wikiChanged=false / 🤷 banner contract', () => {
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('wikiChanged=false')
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('🤷')
  })

  it('teaches concept vs process tiebreaker (algorithms with steps → process)', () => {
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/algorithms with ordered steps/i)
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/Concept vs process tiebreaker/i)
  })

  it('teaches slug = file basename discipline (CJK title must not become slug)', () => {
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/Slug = file basename/i)
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/lower-case kebab-case ASCII/i)
  })
})
