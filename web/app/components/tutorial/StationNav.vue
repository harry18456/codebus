<script setup lang="ts">
// Left rail listing every station with locked / unlocked / current /
// completed visual state. The page controls navigation; this
// component only emits navigate(stationId) when the user clicks an
// unlocked or completed entry. Locked entries are not interactive.

import { computed } from 'vue'

import type { RouteJson } from '~/composables/useStationRoute'

const props = defineProps<{
  route: RouteJson
  currentStationId: string | null
  unlockedStationIds: Set<string>
  completedStationIds: string[]
}>()

const emit = defineEmits<{
  (e: 'navigate', stationId: string): void
  (e: 'navigate-to-moc'): void
}>()

interface StationViewModel {
  station_id: string
  index: number
  title: string
  duration: number
  state: 'completed' | 'current' | 'unlocked' | 'locked' | 'degraded'
  reachable: boolean
}

const viewModel = computed<StationViewModel[]>(() =>
  props.route.stations.map((s) => {
    const completed = props.completedStationIds.includes(s.station_id)
    const unlocked = props.unlockedStationIds.has(s.station_id)
    const current = props.currentStationId === s.station_id
    let state: StationViewModel['state']
    if (s.degraded) state = 'degraded'
    else if (current) state = 'current'
    else if (completed) state = 'completed'
    else if (unlocked) state = 'unlocked'
    else state = 'locked'
    return {
      station_id: s.station_id,
      index: s.index,
      title: s.title,
      duration: s.duration,
      state,
      reachable: completed || unlocked || current
    }
  })
)

const completedCount = computed(() => props.completedStationIds.length)
const totalCount = computed(() => props.route.stations.length)

function handleClick(vm: StationViewModel): void {
  if (!vm.reachable) return
  emit('navigate', vm.station_id)
}

function badgeClass(state: StationViewModel['state']): string {
  switch (state) {
    case 'completed':
      return 'bg-green/25 text-green'
    case 'current':
      return 'bg-accent text-surface-0'
    case 'unlocked':
      return 'bg-surface-3 text-text-dim'
    case 'degraded':
      return 'bg-orange/25 text-orange'
    case 'locked':
    default:
      return 'bg-surface-2 text-text-mute'
  }
}
</script>

<template>
  <nav
    class="h-full overflow-y-auto px-2 py-4 border-r border-border-soft bg-surface-1 text-[12.5px]"
  >
    <button
      type="button"
      class="mx-2 mb-2 px-3 py-2 w-[calc(100%-1rem)] text-left rounded-md bg-surface-2 hover:bg-surface-3 transition-colors flex items-center gap-2 text-text-dim hover:text-text-base"
      data-testid="nav-to-moc"
      @click="emit('navigate-to-moc')"
    >
      <span class="font-mono text-[12.5px]">←</span>
      <span class="text-[12.5px]">教材目錄</span>
    </button>
    <div class="px-3 pb-3 mb-2 border-b border-border-soft">
      <div
        class="font-mono text-[9.5px] tracking-[0.16em] uppercase text-text-mute mb-1"
      >
        Stations
      </div>
      <div class="font-mono text-[10.5px] text-text-dim">
        {{ completedCount }} / {{ totalCount }} 完成
      </div>
    </div>

    <ul class="flex flex-col gap-px">
      <li
        v-for="vm in viewModel"
        :key="vm.station_id"
        class="rounded-md px-3 py-2 grid grid-cols-[26px_1fr] gap-2 items-center"
        :class="[
          vm.reachable
            ? 'cursor-pointer hover:bg-surface-2 transition-colors'
            : 'cursor-not-allowed opacity-60',
          vm.state === 'current' ? 'bg-surface-2' : ''
        ]"
        :data-station-state="vm.state"
        @click="handleClick(vm)"
      >
        <span
          class="w-[22px] h-[22px] rounded-full font-mono text-[10.5px] font-semibold grid place-items-center"
          :class="badgeClass(vm.state)"
        >
          <template v-if="vm.state === 'completed'">✓</template>
          <template v-else-if="vm.state === 'locked'">🔒</template>
          <template v-else-if="vm.state === 'degraded'">⚠</template>
          <template v-else>{{ vm.index }}</template>
        </span>
        <span class="min-w-0">
          <span class="block text-text-dim leading-tight truncate">{{ vm.title }}</span>
          <span class="block font-mono text-[9.5px] text-text-mute mt-1">
            {{ vm.duration }} 分鐘 · {{ vm.station_id }}
          </span>
        </span>
      </li>
    </ul>
  </nav>
</template>
