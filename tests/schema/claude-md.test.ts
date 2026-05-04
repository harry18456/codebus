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
})
