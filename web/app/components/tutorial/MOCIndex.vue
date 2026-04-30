<script setup lang="ts">
// MOC (table-of-contents) renderer for the tutorial index page. The
// markdown body comes from `tutorial.md` (frontmatter stripped); the
// station list with badges is rendered from route.json so the unlock
// state is always live, regardless of whether the LLM-authored MOC
// markdown listed the stations correctly.

import { computed, ref } from 'vue'

import { useTutorialProgress } from '~/composables/useTutorialProgress'
import type { RouteJson, RouteStation } from '~/composables/useStationRoute'

const props = withDefaults(
  defineProps<{
    mocMarkdown: string
    workspaceId: string
    route: RouteJson
    unlockedStationIds: Set<string>
    completedStationIds: string[]
    skippedStationIds?: string[]
  }>(),
  { skippedStationIds: () => [] }
)

const emit = defineEmits<{
  (e: 'navigate', stationId: string): void
}>()

interface MOCEntry {
  station: RouteStation
  state: 'completed' | 'current' | 'unlocked' | 'locked' | 'degraded' | 'skipped'
  reachable: boolean
  href: string
}

const entries = computed<MOCEntry[]>(() =>
  props.route.stations.map((s) => {
    const completed = props.completedStationIds.includes(s.station_id)
    const skipped = props.skippedStationIds.includes(s.station_id)
    const unlocked = props.unlockedStationIds.has(s.station_id)
    let state: MOCEntry['state']
    if (s.degraded) state = 'degraded'
    else if (completed) state = 'completed'
    else if (skipped) state = 'skipped'
    else if (unlocked) state = 'unlocked'
    else state = 'locked'
    return {
      station: s,
      state,
      reachable: completed || skipped || unlocked,
      // URL uses stable station id only (D-T11: task_id stays out of
      // the URL hierarchy; the index page resolves it implicitly).
      href: `/tutorial/${props.workspaceId}/${s.station_id}`
    }
  })
)

function badgeClass(state: MOCEntry['state']): string {
  switch (state) {
    case 'completed':
      return 'bg-green/25 text-green'
    case 'unlocked':
      return 'bg-accent/20 text-accent'
    case 'degraded':
      return 'bg-orange/25 text-orange'
    case 'skipped':
      return 'bg-surface-3 text-text-mute'
    case 'locked':
    default:
      return 'bg-surface-2 text-text-mute'
  }
}

function badgeLabel(state: MOCEntry['state']): string {
  switch (state) {
    case 'completed':
      return '已完成'
    case 'unlocked':
      return '可開始'
    case 'degraded':
      return '產出失敗'
    case 'skipped':
      return '已跳過'
    case 'locked':
    default:
      return '未解鎖'
  }
}

const showResetConfirm = ref(false)
const resetting = ref(false)
const resetError = ref<string | null>(null)

const completedCount = computed(() => props.completedStationIds.length)
const totalCount = computed(() => props.route.stations.length)

async function confirmReset(): Promise<void> {
  resetting.value = true
  resetError.value = null
  try {
    await useTutorialProgress().resetProgress()
    showResetConfirm.value = false
  } catch (err) {
    resetError.value = err instanceof Error ? err.message : String(err)
  } finally {
    resetting.value = false
  }
}

function handleClick(entry: MOCEntry, event: MouseEvent): void {
  if (!entry.reachable) {
    event.preventDefault()
    return
  }
  // Let middle-click / cmd-click open in a new view (router will handle
  // it on next navigation). For plain click, emit so the page can use
  // its own router instance.
  if (event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey) {
    event.preventDefault()
    emit('navigate', entry.station.station_id)
  }
}
</script>

<template>
  <article class="px-12 py-10 max-w-[760px] mx-auto text-text-base">
    <header class="mb-8">
      <div
        class="font-mono text-[10.5px] tracking-[0.16em] uppercase text-text-mute mb-2"
      >
        Tutorial · MOC
      </div>
      <h1 class="text-[32px] font-bold tracking-tight leading-tight">教材目錄</h1>
    </header>

    <section class="mb-10 text-[15px] leading-[1.75] moc-prose">
      <MDC :value="mocMarkdown" />
    </section>

    <section
      class="mb-6 p-4 rounded-lg bg-surface-1 border border-border-soft flex items-center justify-between"
    >
      <div>
        <div
          class="font-mono text-[10.5px] tracking-[0.14em] uppercase text-text-mute mb-1"
        >
          Progress
        </div>
        <div class="text-text-base text-[15px]">
          {{ completedCount }} / {{ totalCount }} 站完成
        </div>
      </div>
      <button
        type="button"
        class="px-3 py-1.5 rounded-md text-[12.5px] bg-surface-2 text-text-dim hover:bg-surface-3 hover:text-text-base transition-colors"
        data-testid="reset-progress"
        @click="showResetConfirm = true"
      >
        ↺ 重新開始
      </button>
    </section>

    <section>
      <h2
        class="font-mono text-[10.5px] tracking-[0.14em] uppercase text-text-mute mb-3"
      >
        Stations
      </h2>
      <ol class="flex flex-col gap-1">
        <li v-for="entry in entries" :key="entry.station.station_id">
          <a
            :href="entry.href"
            class="flex items-center gap-4 p-3 rounded-lg border border-border-soft bg-surface-1 hover:bg-surface-2 transition-colors"
            :class="{ 'cursor-not-allowed opacity-60': !entry.reachable }"
            :data-station-state="entry.state"
            @click="handleClick(entry, $event)"
          >
            <span
              class="w-[28px] h-[28px] rounded-full font-mono text-[11px] font-semibold grid place-items-center"
              :class="badgeClass(entry.state)"
            >
              {{ entry.station.index }}
            </span>
            <span class="flex-1 min-w-0">
              <span class="block text-[14.5px] text-text-base">{{ entry.station.title }}</span>
              <span class="block font-mono text-[10.5px] text-text-mute mt-1">
                {{ entry.station.duration }} 分鐘 · {{ entry.station.station_id }}
              </span>
            </span>
            <span
              class="px-2 py-[2px] rounded font-mono text-[10px] uppercase tracking-wider"
              :class="badgeClass(entry.state)"
            >
              {{ badgeLabel(entry.state) }}
            </span>
          </a>
        </li>
      </ol>
    </section>
  </article>

  <!-- Reset confirm overlay -->
  <div
    v-if="showResetConfirm"
    class="fixed inset-0 z-50 grid place-items-center bg-surface-0/80"
    data-testid="reset-confirm-overlay"
    @click.self="showResetConfirm = false"
  >
    <div
      class="max-w-[480px] w-full mx-4 p-6 rounded-lg bg-surface-1 border border-border-base"
    >
      <h3 class="text-[16px] font-semibold text-text-base mb-2">
        確定清除學習紀錄？
      </h3>
      <p class="text-[13.5px] text-text-dim leading-relaxed mb-4">
        會重置所有 Checkpoint 勾選、Quiz 答題與解鎖狀態，回到第一站。
        教材 markdown 不會被刪除。
      </p>
      <div
        v-if="resetError"
        class="mb-3 p-3 rounded-md text-[12.5px] bg-red/10 text-red font-mono"
      >
        {{ resetError }}
      </div>
      <div class="flex justify-end gap-3">
        <button
          type="button"
          class="px-3 py-1.5 rounded-md text-[13px] bg-surface-2 text-text-dim hover:bg-surface-3 hover:text-text-base"
          :disabled="resetting"
          @click="showResetConfirm = false"
        >
          取消
        </button>
        <button
          type="button"
          class="px-3 py-1.5 rounded-md text-[13px] bg-red text-surface-0 hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
          data-testid="reset-confirm"
          :disabled="resetting"
          @click="confirmReset"
        >
          {{ resetting ? '清除中…' : '清除紀錄' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.moc-prose :deep(h2) {
  font-size: 22px;
  font-weight: 600;
  letter-spacing: -0.015em;
  margin: 28px 0 12px;
  color: theme('colors.text.base');
}
.moc-prose :deep(h3) {
  font-size: 17px;
  font-weight: 600;
  margin: 20px 0 10px;
  color: theme('colors.text.base');
}
.moc-prose :deep(p) {
  margin: 0 0 16px;
  color: theme('colors.text.dim');
  line-height: 1.75;
}
.moc-prose :deep(ul) {
  padding-left: 22px;
  color: theme('colors.text.dim');
  margin: 0 0 16px;
}
.moc-prose :deep(a) {
  color: theme('colors.accent');
  text-decoration: underline;
  text-underline-offset: 2px;
}
</style>
