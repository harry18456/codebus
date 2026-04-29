import { ref, type Ref } from 'vue'
import { useSidecar } from './useSidecar'

// useSanitizerRules — read-only fetch of the sanitizer rules registry from
// the sidecar's GET /sanitizer/rules endpoint. Per spec
// `Requirement: useSanitizerRules composable fetches rules registry from sidecar`:
//   * One fetch per session (module-level cache)
//   * lookup(rule_id) returns matching SanitizerRule | null
//   * Composable MUST NOT issue Tauri IPC of any kind — registry is HTTP only
//   * Composable MUST NOT request "full regex" via any side channel
//
// Forward-reference (P1+) — the future `sanitizer-audit-unlock` capability
// will own raw value retention. This composable stays metadata-only.

export interface SanitizerRule {
  rule_id: string
  kind: string
  description: string
  pattern_summary: string
  source: 'builtin' | 'user_yaml'
}

export interface SanitizerRulesSnapshot {
  rules_version: string
  rules: SanitizerRule[]
}

// Module-level cache. Stays in memory for the Nuxt session — bumping
// `rules_version` requires a sidecar restart per `docs/sanitizer.md §六`,
// so caching across components mounted within the same process is safe.
const snapshot: Ref<SanitizerRulesSnapshot | null> = ref(null)
let inFlight: Promise<void> | null = null

export interface UseSanitizerRulesApi {
  snapshot: Ref<SanitizerRulesSnapshot | null>
  loadOnce: () => Promise<void>
  lookup: (ruleId: string) => SanitizerRule | null
}

async function fetchSnapshot(): Promise<SanitizerRulesSnapshot> {
  const sidecar = useSidecar()
  const res = await sidecar.fetch('/sanitizer/rules')
  if (!res.ok) {
    throw new Error(
      `useSanitizerRules: GET /sanitizer/rules failed with status ${res.status}`
    )
  }
  const body = (await res.json()) as SanitizerRulesSnapshot
  return body
}

export function useSanitizerRules(): UseSanitizerRulesApi {
  async function loadOnce(): Promise<void> {
    if (snapshot.value !== null) return
    if (inFlight !== null) {
      await inFlight
      return
    }
    inFlight = (async () => {
      try {
        snapshot.value = await fetchSnapshot()
      } finally {
        inFlight = null
      }
    })()
    await inFlight
  }

  function lookup(ruleId: string): SanitizerRule | null {
    const snap = snapshot.value
    if (snap === null) return null
    return snap.rules.find((r) => r.rule_id === ruleId) ?? null
  }

  return { snapshot, loadOnce, lookup }
}
