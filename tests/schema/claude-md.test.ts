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
    // Must explicitly forbid creating goal-guide / log / index for noop
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('no "no-op record" goal-guide')
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('No `wiki/log.md` append')
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('No `wiki/index.md` modification')
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
