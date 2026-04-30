import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
import { describe, expect, expectTypeOf, it } from 'vitest'
import type { ActionEntry } from '~/types/agent-action'

// Defensive guards for change fix-action-entry-import-collision.
//
// 1. test_action_entry_single_source — source-grep ensures `export interface
//    ActionEntry` lives in exactly one file: web/app/types/agent-action.ts.
//    Both useQaSession.ts and useExplorerStream.ts must consume it via import,
//    not redeclare it. Locks the Nuxt duplicate-export warning fix.
// 2. test_action_entry_shape_invariant — vitest type-test API freezes the
//    four-field shape so future edits cannot widen/narrow ActionEntry without
//    going through a new change (Non-Goals "不變更 schema").

const repoRoot = resolve(fileURLToPath(import.meta.url), '../../..')

const CANONICAL = resolve(repoRoot, 'app/types/agent-action.ts')
const QA_SESSION = resolve(repoRoot, 'app/composables/useQaSession.ts')
const EXPLORER_STREAM = resolve(repoRoot, 'app/composables/useExplorerStream.ts')

const EXPORT_PATTERN = /export\s+interface\s+ActionEntry\b/

describe('agent-action canonical type module', () => {
  it('test_action_entry_single_source: only the canonical file declares ActionEntry', () => {
    const canonicalSrc = readFileSync(CANONICAL, 'utf-8')
    expect(EXPORT_PATTERN.test(canonicalSrc)).toBe(true)

    const qaSrc = readFileSync(QA_SESSION, 'utf-8')
    expect(EXPORT_PATTERN.test(qaSrc)).toBe(false)

    const explorerSrc = readFileSync(EXPLORER_STREAM, 'utf-8')
    expect(EXPORT_PATTERN.test(explorerSrc)).toBe(false)
  })

  it('test_action_entry_shape_invariant: ActionEntry equals the agreed four-field shape', () => {
    // Runtime guard: canonical file must exist with the four agreed fields.
    // Pairs with the typecheck-time expectTypeOf assertion below — without
    // this readFileSync the test would silently pass even when the canonical
    // module is missing (type-only imports are erased at runtime).
    const canonicalSrc = readFileSync(CANONICAL, 'utf-8')
    expect(canonicalSrc).toMatch(/export\s+interface\s+ActionEntry\s*\{/)
    expect(canonicalSrc).toMatch(/tool:\s*string/)
    expect(canonicalSrc).toMatch(/observation:\s*string/)
    expect(canonicalSrc).toMatch(/tokens_used:\s*number/)
    expect(canonicalSrc).toMatch(/isError:\s*boolean/)

    expectTypeOf<ActionEntry>().toEqualTypeOf<{
      tool: string
      observation: string
      tokens_used: number
      isError: boolean
    }>()
  })
})
