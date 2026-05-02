// Boot-time push of OS-keychain API keys into sidecar memory.
//
// Backs SHALL clauses in
// openspec/changes/phase7-onboarding-polish/specs/keyring-integration/spec.md
//   Requirement: Tauri-to-sidecar startup key injection (idempotent lock relaxed)
//
// D-033 B archive shipped `push_startup_config_cmd` but only wired it to
// the onboarding submit flow. Boot for a returning user who already has
// keys in the OS keychain leaves `app.state.provider_keys` empty in the
// sidecar — `/healthz.dependency.llm_chat` reports `not-configured` and
// `onboarding-redirect.global.ts` middleware sends them straight back to
// `/onboarding/welcome`. This plugin closes that gap by running once at
// boot, after the sidecar handshake, and pushing the keys for every
// provider that already lives in the on-disk config.
//
// We use the function form `defineNuxtPlugin((nuxtApp) => { ... })` to
// stay aligned with `mdc-content-components.client.ts` (the only other
// Nuxt 4 plugin in this app). The unconditional first-line log lets a
// dev confirm the plugin actually got registered without having to
// trigger an error path.

import { useProviderConfig } from '~/composables/useProviderConfig'
import { useSidecar } from '~/composables/useSidecar'

export default defineNuxtPlugin(async (_nuxtApp) => {
  console.log('[sidecar-startup-config] plugin loaded')

  const sidecar = useSidecar()
  try {
    await sidecar.fetch('/healthz')
  } catch (e) {
    console.warn('[sidecar-startup-config] sidecar not reachable, skip:', e)
    return
  }
  if (!sidecar.ready.value) {
    console.warn('[sidecar-startup-config] sidecar handshake not ready, skip')
    return
  }

  const config = useProviderConfig()
  if (!config.loaded.value) {
    try {
      await config.loadConfig()
    } catch (e) {
      console.error('[sidecar-startup-config] loadConfig failed:', e)
      return
    }
  }

  const providerIds = config.providers.value.map((p) => p.id)
  if (providerIds.length === 0) {
    console.log('[sidecar-startup-config] no providers in pool, skip (first boot)')
    return
  }

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('push_startup_config_cmd', { providerIds })
    console.log(
      '[sidecar-startup-config] pushed',
      providerIds.length,
      'provider key(s) to sidecar'
    )
  } catch (e) {
    console.error('[sidecar-startup-config] push_startup_config_cmd failed:', e)
  }
})
