<script setup lang="ts">
// `/` — entry route. Mounts AppShell once the sidecar reports both
// LLM lanes ready; otherwise redirects to `/onboarding/welcome` so
// the user finishes provider setup first.
//
// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-onboarding/spec.md
//   Requirement: Index page redirects to onboarding when LLM dependencies are not configured

import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import AppShell from '~/components/AppShell.vue'
import { useSidecar } from '~/composables/useSidecar'

const router = useRouter()
const checked = ref(false)

async function checkAndRedirect(): Promise<void> {
  try {
    const { fetch } = useSidecar()
    const res = await fetch('/healthz')
    if (!res.ok) {
      checked.value = true
      return
    }
    const body = (await res.json()) as {
      dependency?: Record<string, string>
    }
    const dep = body.dependency
    if (!dep) {
      checked.value = true
      return
    }
    if (
      dep.llm_chat === 'not-configured' ||
      dep.llm_embed === 'not-configured'
    ) {
      router.replace('/onboarding/welcome')
      return
    }
  } catch {
    // sidecar unreachable — let AppShell render so the user sees the
    // existing degraded UI instead of being shoved into onboarding.
  }
  checked.value = true
}

onMounted(checkAndRedirect)
</script>

<template>
  <div data-testid="index-page-root">
    <AppShell v-if="checked" />
    <div
      v-else
      data-testid="index-page-loading"
      class="grid place-items-center min-h-screen text-text-mute text-[12.5px]"
    >
      checking sidecar…
    </div>
  </div>
</template>
