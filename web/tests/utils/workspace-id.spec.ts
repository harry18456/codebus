// Backs SHALL clauses in
// openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//   Requirement: Folder picker invocation flow
//     Scenario: Selected path produces deterministic workspace_id
//
// Parity fixture: the (input, expected) pairs in `PATH_FIXTURES` MUST
// match the expected values asserted in the sidecar parity test
// `sidecar/tests/auth/test_workspace_id_parity.py`. If you bump the
// canonicalization algorithm in either side, regenerate the table by
// running:
//   python -c "import hashlib; \
//     p = lambda s: 'ws_' + hashlib.sha256(s.replace('\\\\','/').lower().encode()).hexdigest()[:12]"

import { describe, expect, it } from 'vitest'
import { deriveWorkspaceId } from '~/utils/workspace-id'

const PATH_FIXTURES: ReadonlyArray<readonly [string, string]> = [
  ['/abs/path', 'ws_6d80187b4541'],
  ['C:\\Users\\harry\\Code\\demo', 'ws_b3e6cc56defb'],
  ['c:/users/harry/code/demo', 'ws_b3e6cc56defb'],
  ['C:/Users/Harry/Code/Demo', 'ws_b3e6cc56defb'],
  ['/home/alice/projects/foo-bar', 'ws_bb0b84426459']
]

describe('deriveWorkspaceId', () => {
  it.each(PATH_FIXTURES)(
    'returns the canonical id for %s',
    async (input, expected) => {
      const id = await deriveWorkspaceId(input)
      expect(id).toBe(expected)
    }
  )

  it('matches the ws_<12 hex> shape exactly', async () => {
    const id = await deriveWorkspaceId('/abs/path')
    expect(id).toMatch(/^ws_[0-9a-f]{12}$/)
    expect(id).toHaveLength(15)
  })

  it('is stable across multiple calls with the same path', async () => {
    const a = await deriveWorkspaceId('/abs/path')
    const b = await deriveWorkspaceId('/abs/path')
    expect(a).toBe(b)
  })

  it('treats Windows backslashes the same as posix slashes', async () => {
    const back = await deriveWorkspaceId('C:\\Users\\harry\\Code\\demo')
    const fwd = await deriveWorkspaceId('c:/users/harry/code/demo')
    expect(back).toBe(fwd)
  })

  it('treats mixed-case Windows paths as the same workspace', async () => {
    const upper = await deriveWorkspaceId('C:/Users/Harry/Code/Demo')
    const lower = await deriveWorkspaceId('c:/users/harry/code/demo')
    expect(upper).toBe(lower)
  })
})
