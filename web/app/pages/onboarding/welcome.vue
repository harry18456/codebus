<script setup lang="ts">
// `/onboarding/welcome` — first step of the three-step onboarding
// wizard. Pure copy + a single Next button (always enabled — no
// fields to fill).
//
// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-onboarding/spec.md
//   Requirement: Onboarding wizard exposes three sequential routes
//   Scenario: Welcome page next button always enabled

import { useRouter } from 'vue-router'

const router = useRouter()

definePageMeta({ layout: false })

function onNext(): void {
  router.push('/onboarding/providers')
}
</script>

<template>
  <main
    data-testid="onboarding-welcome"
    class="grid place-items-center min-h-screen p-8"
  >
    <div class="max-w-[560px] flex flex-col gap-4 text-center">
      <h1 class="text-[24px] font-semibold text-text-base">Welcome to CodeBus</h1>
      <p class="text-[14px] text-text-dim leading-relaxed">
        CodeBus turns an unfamiliar codebase into a guided tutorial.
        It needs an LLM provider to function — pick a chat model and
        an embedding model in the next step. Your API keys are stored
        in your operating system's native keychain (macOS Keychain,
        Windows Credential Manager, or Linux Secret Service) and
        never touch disk or audit logs.
      </p>
      <p class="text-[12.5px] text-text-mute">
        Read the
        <a
          href="https://openai.com/policies/terms-of-use/"
          target="_blank"
          rel="noopener"
          data-testid="onboarding-welcome-tos-link"
          class="underline"
          >OpenAI Terms of Service</a
        >
        before continuing.
      </p>
      <button
        type="button"
        data-testid="onboarding-welcome-next"
        class="self-center px-4 py-2 rounded-md bg-blue-500 text-white text-[14px]"
        @click="onNext"
      >
        Next
      </button>
    </div>
  </main>
</template>
