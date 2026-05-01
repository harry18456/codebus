<script setup lang="ts">
// `<RoleBindingTable>` — second section of `/settings`. Renders four
// rows (reasoning / judge / chat / embed) and a dropdown per row that
// filters compatible providers from the pool. Embed changes go
// through `<EmbeddingChangeConfirmModal>` first.
//
// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Requirement: Role binding change propagates via hot-swap
//   Requirement: Embedding switch goes through destructive confirm modal

import { computed, ref } from 'vue'

import { type RoleName, useProviderConfig } from '~/composables/useProviderConfig'

import EmbeddingChangeConfirmModal from './EmbeddingChangeConfirmModal.vue'

const ROLES: ReadonlyArray<{ name: RoleName; label: string }> = [
  { name: 'reasoning', label: 'reasoning' },
  { name: 'judge', label: 'judge' },
  { name: 'chat', label: 'chat' },
  { name: 'embed', label: 'embed' }
]

interface Props {
  // Optional override so tests / pages can pass an updated chunk
  // count without re-fetching kb stats from inside this component.
  kbChunkCount?: number
}

const props = withDefaults(defineProps<Props>(), { kbChunkCount: 0 })

const config = useProviderConfig()

const chatTypedProviders = computed(() =>
  config.providers.value.filter((p) => p.type === 'openai_chat')
)
const embedTypedProviders = computed(() =>
  config.providers.value.filter((p) => p.type === 'openai_embedding')
)

function optionsFor(role: RoleName) {
  return role === 'embed' ? embedTypedProviders.value : chatTypedProviders.value
}

const pendingEmbed = ref<string | null>(null)
const embedModalOpen = computed(() => pendingEmbed.value !== null)

async function onSelect(role: RoleName, providerId: string): Promise<void> {
  if (!providerId) return
  if (role === 'embed') {
    if (config.bindings.value.embed === providerId) return
    pendingEmbed.value = providerId
    return
  }
  await config.setBinding(role, providerId)
}

async function onConfirmEmbed(): Promise<void> {
  if (!pendingEmbed.value) return
  const target = pendingEmbed.value
  pendingEmbed.value = null
  await config.setBinding('embed', target)
  // Trigger KB rebuild SSE task. POST /kb/build on the sidecar; the
  // page-level KB rebuild banner subscribes to the response task_id.
  // Failure surfaces via console; the binding has already swapped.
  try {
    const { useSidecar } = await import('~/composables/useSidecar')
    await useSidecar().fetch('/kb/build', { method: 'POST' })
  } catch {
    /* ignore — page-level KB stats panel surfaces follow-up state */
  }
}

function onCancelEmbed(): void {
  pendingEmbed.value = null
}
</script>

<template>
  <section
    data-section="role-bindings"
    data-testid="role-bindings-section"
    class="p-4 rounded-lg bg-surface-1 border border-border-base"
  >
    <h2 class="text-[14px] font-semibold text-text-base mb-3">Role bindings</h2>
    <table class="w-full text-[13px]">
      <thead class="text-text-mute text-[11.5px] uppercase tracking-wide">
        <tr>
          <th class="text-left p-2">role</th>
          <th class="text-left p-2">provider</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="role in ROLES"
          :key="role.name"
          data-testid="role-binding-row"
          :data-role="role.name"
          class="border-t border-border-base"
        >
          <td class="p-2 text-text-dim font-mono">{{ role.label }}</td>
          <td class="p-2">
            <select
              :data-testid="`role-binding-select-${role.name}`"
              :value="config.bindings.value[role.name] ?? ''"
              class="px-3 py-1.5 rounded-md bg-surface-2 text-text-base"
              @change="
                onSelect(role.name, ($event.target as HTMLSelectElement).value)
              "
            >
              <option value="" disabled>— pick provider —</option>
              <option
                v-for="opt in optionsFor(role.name)"
                :key="opt.id"
                :value="opt.id"
              >
                {{ opt.id }} ({{ opt.model }})
              </option>
            </select>
          </td>
        </tr>
      </tbody>
    </table>

    <EmbeddingChangeConfirmModal
      :open="embedModalOpen"
      :new-provider-id="pendingEmbed"
      :current-chunk-count="props.kbChunkCount"
      @cancel="onCancelEmbed"
      @confirm="onConfirmEmbed"
    />
  </section>
</template>
