<script setup lang="ts">
// "↷ 跳過此站" affordance for the station page header chrome.
//
// Three render states:
// - completed: button does NOT render (skip is meaningless once done)
// - skipped: button renders but is inert (click no-op, tooltip "本站已跳過")
// - never-visited / current: button renders interactive; click opens
//   the InterventionConfirmModal via useIntervention().requestSkip(...).
//
// On confirm, calls useTutorialProgress().markStationSkipped(stationId)
// then navigates to the next unlocked station (or MOC if last). The
// component takes `route` + `workspaceId` so it can derive the next
// station and `router.push` without prop-drilling navigation through.

import { computed } from 'vue'
import { useRouter } from 'vue-router'

import { useIntervention } from '~/composables/useIntervention'
import { useTutorialProgress } from '~/composables/useTutorialProgress'
import type { RouteJson } from '~/composables/useStationRoute'

const props = defineProps<{
  stationId: string
  stationTitle: string
  // Route + workspace context required to navigate forward after skip.
  // Optional so the component can mount in tests without page wiring;
  // when absent, click still opens the modal but `onConfirm` will only
  // mutate progress (no navigation).
  route?: RouteJson | null
  workspaceId?: string | null
  workspaceRoot?: string | null
  taskId?: string | null
}>()

const intervention = useIntervention()
const progress = useTutorialProgress()
const router = useRouter()

const isCompleted = computed(() =>
  progress.state.value.completed_station_ids.includes(props.stationId)
)
const isSkipped = computed(() =>
  progress.state.value.skipped_station_ids.includes(props.stationId)
)

function navigateAfterSkip(): void {
  if (!props.route || !props.workspaceId || !props.workspaceRoot) return
  const idx = props.route.stations.findIndex(
    (s) => s.station_id === props.stationId
  )
  const next = idx >= 0 ? props.route.stations[idx + 1] : undefined
  const baseQuery: Record<string, string> = {
    ws_path: props.workspaceRoot
  }
  if (props.taskId) baseQuery.task = props.taskId
  if (next) {
    void router.push({
      path: `/tutorial/${props.workspaceId}/${next.station_id}`,
      query: baseQuery
    })
  } else {
    void router.push({
      path: `/tutorial/${props.workspaceId}`,
      query: { ws_path: props.workspaceRoot }
    })
  }
}

function onClick(): void {
  if (isCompleted.value || isSkipped.value) return
  intervention.requestSkip({
    stationId: props.stationId,
    stationTitle: props.stationTitle,
    onConfirm: () => {
      progress.markStationSkipped(props.stationId)
      navigateAfterSkip()
    }
  })
}
</script>

<template>
  <button
    v-if="!isCompleted"
    type="button"
    data-testid="skip-station-button"
    class="px-2.5 py-1 rounded-md border text-[12px] font-mono"
    :class="
      isSkipped
        ? 'border-border-soft text-text-mute bg-surface-2 cursor-not-allowed'
        : 'border-border-base text-text-dim bg-surface-1 hover:border-orange hover:text-orange'
    "
    :title="isSkipped ? '本站已跳過' : `跳過此站（${stationTitle}）`"
    @click="onClick"
  >
    ↷ 跳過此站
  </button>
</template>
