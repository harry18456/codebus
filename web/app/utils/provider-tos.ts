// Backs SHALL clauses in
// openspec/changes/phase7-onboarding-polish/specs/provider-onboarding/spec.md
//   Requirement: Onboarding wizard exposes three sequential routes
//   Scenario: Providers page renders contextual ToS link per type
//
// Per-app single-source mapping from provider `type` to terms-of-service
// URL. Adding a new provider type means adding one entry here — `welcome.vue`
// and `done.vue` MUST stay provider-agnostic per the spec.

export type KnownProviderType = 'openai_chat' | 'openai_embedding'

export const PROVIDER_TYPE_TOS_URL: Record<KnownProviderType, string> = {
  openai_chat: 'https://openai.com/policies/terms-of-use/',
  openai_embedding: 'https://openai.com/policies/terms-of-use/'
}

export function getTosUrl(type: KnownProviderType): string | null {
  return PROVIDER_TYPE_TOS_URL[type] ?? null
}
