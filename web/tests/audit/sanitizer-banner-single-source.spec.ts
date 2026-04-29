import { describe, expect, it } from 'vitest'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { resolve, join } from 'node:path'

// Source-grep invariant: the literal string `raw values are not retained per D-015`
// MUST appear in exactly one file under `web/app/`. All other consumers
// (pages/audit/sanitizer.vue, AuditPanel.vue, SanitizerAuditInspector.vue
// itself) reach the same text by importing `SANITIZER_AUDIT_BANNER`.
//
// Per spec scenario `Banner string lives in a single constant`:
// > the only matching file MUST be the inspector's TypeScript module
// > exporting `SANITIZER_AUDIT_BANNER`

const NEEDLE = 'raw values are not retained per D-015'

function* walk(dir: string): Generator<string> {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name)
    const st = statSync(full)
    if (st.isDirectory()) {
      yield* walk(full)
    } else if (st.isFile()) {
      yield full
    }
  }
}

describe('SANITIZER_AUDIT_BANNER literal lives in a single file', () => {
  it('exactly one file under web/app/ contains the banner literal', () => {
    const root = resolve(process.cwd(), 'app')
    const matches: string[] = []
    for (const file of walk(root)) {
      const text = readFileSync(file, 'utf-8')
      if (text.includes(NEEDLE)) matches.push(file)
    }
    expect(matches).toHaveLength(1)
    // Sanity: the matching file path ends with the canonical constant module.
    const expected = 'sanitizerAuditBanner.ts'
    expect(matches[0]?.replace(/\\/g, '/')).toContain(expected)
  })
})
