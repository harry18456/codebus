<script setup lang="ts">
// `<ProviderPoolList>` — first section of `/settings`. Renders the
// `llm.providers[]` snapshot one row at a time with edit / delete
// affordances and an "Add provider" button that opens
// `<ProviderEditModal>`.
//
// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Requirement: Provider pool CRUD touches keyring and config

import { computed, ref } from 'vue'

import {
  type ProviderSpec,
  useProviderConfig
} from '~/composables/useProviderConfig'

import ProviderEditModal from './ProviderEditModal.vue'

const config = useProviderConfig()
const editModalOpen = ref(false)
const editing = ref<ProviderSpec | null>(null)
const blockedDelete = ref<{ id: string; roles: string[] } | null>(null)

const boundIds = computed(() => new Set(Object.values(config.bindings.value)))

function openAdd(): void {
  editing.value = null
  editModalOpen.value = true
}

function openEdit(provider: ProviderSpec): void {
  editing.value = provider
  editModalOpen.value = true
}

async function callKeyringDelete(providerId: string): Promise<void> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('keyring_delete', { providerId })
  } catch {
    // Best-effort: keyring delete failures don't block the provider
    // pool delete because the secret is already useless once the
    // provider is gone. The user is told via console only.
  }
}

async function onDelete(provider: ProviderSpec): Promise<void> {
  const boundRoles: string[] = []
  for (const [role, id] of Object.entries(config.bindings.value)) {
    if (id === provider.id) boundRoles.push(role)
  }
  if (boundRoles.length > 0) {
    blockedDelete.value = { id: provider.id, roles: boundRoles }
    return
  }
  await callKeyringDelete(provider.id)
  await config.deleteProvider(provider.id)
}
</script>

<template>
  <section
    data-section="provider-pool"
    data-testid="provider-pool-section"
    class="p-4 rounded-lg bg-surface-1 border border-border-base"
  >
    <header class="flex items-center justify-between mb-3">
      <h2 class="text-[14px] font-semibold text-text-base">Provider pool</h2>
      <button
        type="button"
        data-testid="provider-pool-add"
        class="px-3 py-1.5 rounded-md text-[12px] bg-blue-500 text-white hover:bg-blue-600"
        @click="openAdd"
      >
        + Add provider
      </button>
    </header>

    <p
      v-if="config.providers.value.length === 0"
      data-testid="provider-pool-empty"
      class="text-[13px] text-text-dim"
    >
      No providers configured.
    </p>

    <ul v-else class="flex flex-col gap-2">
      <li
        v-for="p in config.providers.value"
        :key="p.id"
        data-testid="provider-pool-row"
        :data-provider-id="p.id"
        class="flex items-center justify-between p-3 rounded-md bg-surface-2"
      >
        <div class="flex flex-col">
          <span class="text-[13px] font-medium text-text-base">{{ p.id }}</span>
          <span class="text-[11.5px] text-text-mute">
            {{ p.type }} · {{ p.model }}
          </span>
        </div>
        <div class="flex gap-2">
          <button
            type="button"
            data-testid="provider-pool-edit"
            class="px-2 py-1 rounded-md text-[12px] bg-surface-3 text-text-dim hover:text-text-base"
            @click="openEdit(p)"
          >
            edit
          </button>
          <button
            type="button"
            data-testid="provider-pool-delete"
            :title="boundIds.has(p.id) ? 'remove role binding first' : ''"
            :class="[
              'px-2 py-1 rounded-md text-[12px] bg-surface-3',
              boundIds.has(p.id)
                ? 'text-rose-300/40 cursor-not-allowed'
                : 'text-rose-300 hover:text-rose-200'
            ]"
            @click="onDelete(p)"
          >
            delete
          </button>
        </div>
      </li>
    </ul>

    <p
      v-if="blockedDelete"
      data-testid="provider-pool-delete-blocked"
      class="mt-3 text-[12.5px] text-amber-300"
    >
      Provider <code>{{ blockedDelete.id }}</code> is bound to:
      {{ blockedDelete.roles.join(', ') }}. Remove role binding first.
    </p>

    <ProviderEditModal
      :open="editModalOpen"
      :initial="editing"
      @close="editModalOpen = false"
    />
  </section>
</template>
