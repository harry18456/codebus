<script setup lang="ts">
// `/onboarding/welcome` — first step of the three-step onboarding
// wizard. Pure copy + a single Next button (always enabled — no
// fields to fill).
//
// Backs SHALL clauses in
// openspec/changes/phase7-onboarding-polish/specs/provider-onboarding/spec.md
//   Requirement: Onboarding wizard exposes three sequential routes
//   Scenario: Welcome page next button always enabled
//   Scenario: Welcome page contains no provider-specific ToS link

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
      <h1 class="text-[24px] font-semibold text-text-base">歡迎使用 CodeBus</h1>
      <p class="text-[14px] text-text-dim leading-relaxed">
        CodeBus 把陌生的 codebase 變成可走訪的教學。需要設定 LLM 提供者才能運作 ——
        下一步請挑一個 chat model 與一個 embedding model。你的 API key 會存放在作業系統原生的
        keychain（macOS Keychain、Windows Credential Manager 或 Linux Secret Service），
        不會落地磁碟，也不會寫入稽核紀錄。
      </p>
      <p class="text-[12.5px] text-text-mute">
        下一步可挑選你要使用的 LLM 提供者；在輸入 API key 前，請先閱讀該提供者的服務條款。
      </p>
      <button
        type="button"
        data-testid="onboarding-welcome-next"
        class="self-center px-4 py-2 rounded-md bg-blue-500 text-white text-[14px]"
        @click="onNext"
      >
        下一步
      </button>
    </div>
  </main>
</template>
