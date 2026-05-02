// Backs SHALL clauses in
// openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//   Requirement: Folder picker invocation flow
//     Scenario: Selected path produces deterministic workspace_id
//
// Mirrors `codebus_agent.auth.service.workspace_id_for_path`. The
// canonical form is `posix-slash + lowercase`; the digest is SHA-256
// truncated to the first 12 hex characters and prefixed with `ws_`.
//
// Frontend / sidecar parity is enforced by two paired tests:
//   - web/tests/utils/workspace-id.spec.ts
//   - sidecar/tests/auth/test_workspace_id_parity.py
// Both reference the same fixture table; if you bump the algorithm,
// regenerate both tables in lockstep.

const WORKSPACE_ID_PREFIX = 'ws_'
const WORKSPACE_ID_HEX_LEN = 12

function canonicalize(absolutePath: string): string {
  return absolutePath.replace(/\\/g, '/').toLowerCase()
}

function bytesToHex(bytes: Uint8Array): string {
  let out = ''
  for (let i = 0; i < bytes.length; i += 1) {
    const b = bytes[i] ?? 0
    out += b.toString(16).padStart(2, '0')
  }
  return out
}

export async function deriveWorkspaceId(absolutePath: string): Promise<string> {
  const canonical = canonicalize(absolutePath)
  const encoded = new TextEncoder().encode(canonical)
  const digest = await crypto.subtle.digest('SHA-256', encoded)
  const hex = bytesToHex(new Uint8Array(digest))
  return `${WORKSPACE_ID_PREFIX}${hex.slice(0, WORKSPACE_ID_HEX_LEN)}`
}
