<script setup lang="ts">
// `/` — entry route. The page is two-staged:
//   1. Mount-time `/healthz` poll redirects to `/onboarding/welcome`
//      whenever an LLM dependency lane reports `not-configured`
//      (preserved from the `provider-settings-and-onboarding` change —
//      provider-onboarding spec Requirement: Index page redirects to
//      onboarding when LLM dependencies are not configured).
//   2. Once both LLM lanes are `ready`, the page renders the workspace
//      onramp surface: `<FolderPickerButton>` (CTA) + `<WorkspaceOnrampCard>`
//      (state-driven phase view). Clicking the picker funnels the
//      selected absolute path into `useWorkspaceOnramp().start(path)`,
//      which drives the 4-step sidecar pipeline (scan → kb-build →
//      explore → generate) per the onramp spec.

import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import FolderPickerButton from '~/components/workspace-onramp/FolderPickerButton.vue'
import WorkspaceOnrampCard from '~/components/workspace-onramp/WorkspaceOnrampCard.vue'
import { useSidecar } from '~/composables/useSidecar'
import { useWorkspaceOnramp } from '~/composables/useWorkspaceOnramp'

const router = useRouter()
const checked = ref(false)

const onramp = useWorkspaceOnramp()

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
    // sidecar unreachable — let the onramp surface render so the user
    // sees a degraded UI instead of being shoved into onboarding.
  }
  checked.value = true
}

async function onPicked(path: string): Promise<void> {
  await onramp.start(path)
}

onMounted(checkAndRedirect)
</script>

<template>
  <div data-testid="index-page-root">
    <section
      v-if="checked"
      class="flex flex-col items-center justify-center gap-6 min-h-screen px-6 py-16 bg-surface-0 text-text-base"
    >
      <h1 class="text-3xl font-semibold tracking-tight">CodeBus</h1>
      <p class="text-[13px] text-text-mute">
        把陌生 codebase 一鍵變成可走訪的 tutorial。
      </p>
      <FolderPickerButton @picked="onPicked" />
      <div class="w-full max-w-xl">
        <WorkspaceOnrampCard />
      </div>
    </section>
    <div
      v-else
      data-testid="index-page-loading"
      class="grid place-items-center min-h-screen text-text-mute text-[12.5px]"
    >
      checking sidecar…
    </div>
  </div>
</template>
