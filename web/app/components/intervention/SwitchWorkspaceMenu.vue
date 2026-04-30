<script setup lang="ts">
// Workspace identifier chip + dropdown menu in <TopBar>. Renders only on
// tutorial-level routes (per spec scenario "Workspace chip absent on
// entry and grant pages") and offers "🔁 換資料夾" which triggers the
// confirm modal via useIntervention().requestSwitchWorkspace().

import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useIntervention } from '~/composables/useIntervention'

const props = defineProps<{
  workspaceRoot: string
}>()

const route = useRoute()
const router = useRouter()
const intervention = useIntervention()

// Hide on entry (index) and grant pages — there is no current workspace
// on those routes and rendering the chip would mislead the user.
const HIDDEN_ROUTE_NAMES = new Set<string>(['index', 'workspace-grant'])

const visible = computed<boolean>(() => {
  const name = route?.name
  if (typeof name !== 'string') return true
  return !HIDDEN_ROUTE_NAMES.has(name)
})

const basename = computed<string>(() => {
  const raw = props.workspaceRoot
  if (!raw) return ''
  // Handle both POSIX and Windows path separators; strip trailing slashes.
  const cleaned = raw.replace(/[\\/]+$/, '')
  const parts = cleaned.split(/[\\/]/)
  return parts[parts.length - 1] || cleaned
})

const dropdownOpen = ref(false)

function toggleDropdown(): void {
  dropdownOpen.value = !dropdownOpen.value
}

function onSwitchSelected(): void {
  dropdownOpen.value = false
  intervention.requestSwitchWorkspace({
    onConfirm: () => {
      void router.push('/')
    }
  })
}
</script>

<template>
  <div v-if="visible" class="relative">
    <button
      type="button"
      data-testid="workspace-chip"
      class="flex items-center gap-1.5 px-2.5 py-1 rounded-md font-mono text-[11.5px] text-text-dim bg-surface-2 border border-border-base hover:border-surface-4 hover:text-text-base"
      :title="`Switch workspace (current: ${basename})`"
      @click="toggleDropdown"
    >
      <svg
        class="w-[1em] h-[1em] inline-block align-middle"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
      >
        <path d="M2 4a1 1 0 0 1 1-1h3l1.5 1.5H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V4Z" />
      </svg>
      <span>{{ basename }}</span>
      <span class="text-text-mute">▾</span>
    </button>
    <div
      v-if="dropdownOpen"
      data-testid="workspace-dropdown"
      class="absolute z-40 mt-1 left-0 min-w-[200px] rounded-md bg-surface-1 border border-border-base shadow-lg overflow-hidden"
    >
      <button
        type="button"
        data-testid="workspace-switch-action"
        class="w-full text-left px-3 py-2 text-[12.5px] text-text-dim hover:bg-surface-2 hover:text-text-base"
        @click="onSwitchSelected"
      >
        🔁 換資料夾
      </button>
    </div>
  </div>
</template>
