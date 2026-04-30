// Defensive source-grep test for the single-writer invariant on
// `progress.json`. Spec scenario "Single-writer invariant enforced by
// source grep" — `useTutorialProgress` MUST be the only caller of
// `writeProgressFile` (the public method on `useTutorialFiles`).
//
// Grep boundary: `web/app/`. Only `useTutorialProgress.ts` is allowed
// to invoke `.writeProgressFile(...)`. The IPC call site
// (`invoke('write_progress_file', ...)`) lives in `useTutorialFiles.ts`
// as the wrapper; everything else MUST go through `useTutorialProgress`.

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

const APP_DIR = join(__dirname, '..', '..', 'app')

function* walk(dir: string): Generator<string> {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry)
    const st = statSync(full)
    if (st.isDirectory()) {
      yield* walk(full)
    } else if (
      entry.endsWith('.ts') ||
      entry.endsWith('.vue') ||
      entry.endsWith('.tsx')
    ) {
      yield full
    }
  }
}

const PROGRESS_TS = join(APP_DIR, 'composables', 'useTutorialProgress.ts')
const FILES_TS = join(APP_DIR, 'composables', 'useTutorialFiles.ts')

const WRITE_PROGRESS_CALL_RE = /\.writeProgressFile\s*\(/g
const WRITE_PROGRESS_INVOKE_RE = /invoke\(\s*['"]write_progress_file['"]/g

describe('progress.json single-writer invariant', () => {
  it('only useTutorialProgress.ts calls .writeProgressFile(...)', () => {
    const violations: string[] = []
    for (const file of walk(APP_DIR)) {
      if (file === PROGRESS_TS) continue
      const text = readFileSync(file, 'utf-8')
      if (WRITE_PROGRESS_CALL_RE.test(text)) {
        violations.push(file)
      }
      WRITE_PROGRESS_CALL_RE.lastIndex = 0
    }
    expect(violations).toEqual([])
  })

  it('only useTutorialFiles.ts invokes Tauri write_progress_file IPC', () => {
    const violations: string[] = []
    for (const file of walk(APP_DIR)) {
      if (file === FILES_TS) continue
      const text = readFileSync(file, 'utf-8')
      if (WRITE_PROGRESS_INVOKE_RE.test(text)) {
        violations.push(file)
      }
      WRITE_PROGRESS_INVOKE_RE.lastIndex = 0
    }
    expect(violations).toEqual([])
  })

  it('useIntervention.ts does NOT directly invoke writeProgressFile', () => {
    const interventionTs = join(APP_DIR, 'composables', 'useIntervention.ts')
    const text = readFileSync(interventionTs, 'utf-8')
    expect(text).not.toMatch(/writeProgressFile/)
    expect(text).not.toMatch(/write_progress_file/)
  })

  it('markStationSkipped in useTutorialProgress.ts triggers the same write path (uses scheduleFlush)', () => {
    const text = readFileSync(PROGRESS_TS, 'utf-8')
    // markStationSkipped MUST exist as a function and reach scheduleFlush
    // (the canonical write trigger). This test is a structural guard:
    // future refactors must not introduce an alternate write path.
    expect(text).toMatch(/function markStationSkipped\b/)
    // Every state-mutating public method funnels through scheduleFlush;
    // markStationSkipped must do the same.
    const fnMatch = text.match(
      /function markStationSkipped[\s\S]+?\n\}/m
    )
    expect(fnMatch).not.toBeNull()
    expect(fnMatch?.[0] ?? '').toContain('scheduleFlush()')
  })
})
