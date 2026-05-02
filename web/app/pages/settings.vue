<script setup lang="ts">
// `/settings` — D-033 B Setting Page entry route. Three top-to-bottom
// sections per provider-settings spec: provider pool, role bindings,
// PII mode.
//
// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Requirement: Settings page renders three sections

import { onMounted } from 'vue'

import PiiModeToggle from '~/components/settings/PiiModeToggle.vue'
import ProviderPoolList from '~/components/settings/ProviderPoolList.vue'
import RoleBindingTable from '~/components/settings/RoleBindingTable.vue'
import { useProviderConfig } from '~/composables/useProviderConfig'

const config = useProviderConfig()

onMounted(async () => {
  if (!config.loaded.value) {
    try {
      await config.loadConfig()
    } catch {
      // sidecar may be cold during first paint — retry happens via the
      // SSE-driven re-fetch on `provider_config_changed`, or the user
      // can navigate away and back.
    }
  }
})
</script>

<template>
  <main
    data-testid="settings-page"
    class="flex flex-col gap-4 p-6 max-w-[920px] mx-auto"
  >
    <header class="mb-2">
      <h1 class="text-[20px] font-semibold text-text-base">設定</h1>
      <p class="text-[13px] text-text-dim">
        管理 LLM 提供者、角色綁定，以及 PII 偵測模式。
      </p>
    </header>

    <ProviderPoolList />
    <RoleBindingTable />
    <PiiModeToggle />
  </main>
</template>
