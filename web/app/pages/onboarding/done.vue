<script setup lang="ts">
// `/onboarding/done` — third and final step. Confirmation copy + a
// single Start button that routes to `/`.
//
// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-onboarding/spec.md
//   Scenario: Done page Start button routes to entry page

import { useRouter } from 'vue-router'

definePageMeta({ layout: false })

const router = useRouter()

function onStart(): void {
  router.push('/')
}
</script>

<template>
  <main
    data-testid="onboarding-done"
    class="grid place-items-center min-h-screen p-8"
  >
    <div class="max-w-[480px] flex flex-col gap-4 text-center">
      <h1 class="text-[24px] font-semibold text-text-base">All set</h1>
      <p class="text-[14px] text-text-dim">
        Your providers are configured. CodeBus is ready to turn an
        unfamiliar codebase into a guided tutorial.
      </p>
      <button
        type="button"
        data-testid="onboarding-done-start"
        class="self-center px-4 py-2 rounded-md bg-blue-500 text-white text-[14px]"
        @click="onStart"
      >
        Start
      </button>
    </div>
  </main>
</template>
