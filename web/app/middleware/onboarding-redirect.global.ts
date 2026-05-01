// Global Nuxt route middleware — redirects to `/onboarding/welcome`
// when `/healthz.dependency` reports any of `llm_chat` / `llm_embed`
// as `not-configured`.
//
// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-onboarding/spec.md
//   Requirement: Startup detection redirects to onboarding when any LLM dependency is not configured
//
// Skip rules:
//   - `/onboarding/*` routes never redirect (otherwise the wizard
//     can't be reached when the keyring is empty)
//   - `/healthz` is a diagnostic surface; reserved

import { useSidecar } from '~/composables/useSidecar'

interface HealthzDependency {
  llm_chat?: string
  llm_embed?: string
  pii?: string
  [key: string]: string | undefined
}

export interface HealthzResponse {
  status: string
  dependency?: HealthzDependency
}

export async function decideOnboardingRedirect(
  toPath: string,
  fetchHealthz: () => Promise<HealthzResponse | null>
): Promise<string | null> {
  if (toPath.startsWith('/onboarding')) return null
  if (toPath === '/healthz') return null

  const health = await fetchHealthz()
  if (!health || !health.dependency) return null

  const chat = health.dependency.llm_chat
  const embed = health.dependency.llm_embed
  if (chat === 'not-configured' || embed === 'not-configured') {
    return '/onboarding/welcome'
  }
  return null
}

async function defaultFetchHealthz(): Promise<HealthzResponse | null> {
  try {
    const { fetch } = useSidecar()
    const res = await fetch('/healthz')
    if (!res.ok) return null
    return (await res.json()) as HealthzResponse
  } catch {
    return null
  }
}

export default defineNuxtRouteMiddleware(async (to) => {
  const target = await decideOnboardingRedirect(to.path, defaultFetchHealthz)
  if (target) return navigateTo(target)
})
