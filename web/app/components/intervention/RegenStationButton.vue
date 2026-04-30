<script setup lang="ts">
// "↻ 重生此站" affordance for the station page header chrome.
//
// Renders for any station regardless of degraded state — the user may
// also want to regenerate a successful-but-unsatisfying station. When
// degraded, the button visually emphasizes (matching the existing "本站
// 產出失敗，請重跑" warning so the recovery affordance is obvious).
//
// On click, opens the InterventionConfirmModal with the regen kind.
// The actual sidecar call (POST /generate with target_stations) lives
// in the page integration so the test surface stays focused on the
// emit + payload contract.

import { useIntervention } from '~/composables/useIntervention'

const props = defineProps<{
  stationId: string
  stationTitle: string
  taskId: string
  workspaceRoot: string
  degraded?: boolean
}>()

const emit = defineEmits<{
  (e: 'requested-regen', stationId: string): void
}>()

const intervention = useIntervention()

function onClick(): void {
  intervention.requestRegen({
    stationId: props.stationId,
    stationTitle: props.stationTitle,
    taskId: props.taskId,
    workspaceRoot: props.workspaceRoot,
    onConfirm: () => {
      emit('requested-regen', props.stationId)
    }
  })
}
</script>

<template>
  <button
    type="button"
    data-testid="regen-station-button"
    class="px-2.5 py-1 rounded-md border text-[12px] font-mono"
    :class="
      degraded
        ? 'border-orange text-orange bg-orange/10 hover:bg-orange/20'
        : 'border-border-base text-text-dim bg-surface-1 hover:border-accent hover:text-accent'
    "
    :title="`重生此站（${stationTitle}）`"
    @click="onClick"
  >
    ↻ 重生此站
  </button>
</template>
