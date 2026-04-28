<script setup lang="ts">
// Single station page. URL key is the D-029 stable station id;
// `?ws_path=` carries the absolute workspace path (same query the index
// page consumes). The page calls `useTutorialProgress.canVisitStation`
// to gate access — already-completed stations are reachable in review
// mode regardless of the unlock-forward window.

import matter from 'gray-matter'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import StationContent from '~/components/tutorial/StationContent.vue'
import StationLayout, {
  type StationFrontmatter
} from '~/components/tutorial/StationLayout.vue'
import StationNav from '~/components/tutorial/StationNav.vue'
import { useStationRoute, type RouteJson } from '~/composables/useStationRoute'
import { useTutorialFiles } from '~/composables/useTutorialFiles'
import { useTutorialProgress } from '~/composables/useTutorialProgress'

const STATION_ID_RE = /^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$/

const route = useRoute()
const router = useRouter()
const files = useTutorialFiles()
const stationRoute = useStationRoute()
const progress = useTutorialProgress()

const loading = ref(true)
const errorMessage = ref<string | null>(null)
const taskId = ref<string | null>(null)
const routeJson = ref<RouteJson | null>(null)
const frontmatter = ref<StationFrontmatter | null>(null)
const body = ref('')

const workspaceId = computed(() => String(route.params.workspace_id ?? ''))
const stationId = computed(() => String(route.params.station_id ?? ''))
const workspaceRoot = computed(() => {
  const raw = route.query.ws_path
  if (typeof raw !== 'string' || raw.length === 0) return null
  return raw
})
const queryTask = computed(() => {
  const raw = route.query.task
  return typeof raw === 'string' && raw.length > 0 ? raw : null
})

const stationIdValid = computed(() => STATION_ID_RE.test(stationId.value))

const completedStationIds = computed(() => progress.state.value.completed_station_ids)
const unlockedStationIds = computed(() =>
  routeJson.value
    ? progress.unlockedStationIds(routeJson.value).value
    : new Set<string>()
)
const reachable = computed(() => {
  if (!routeJson.value) return false
  return progress.canVisitStation(stationId.value, routeJson.value).value
})
const isReviewMode = computed(() =>
  completedStationIds.value.includes(stationId.value)
)

async function bootstrap(): Promise<void> {
  loading.value = true
  errorMessage.value = null
  routeJson.value = null
  frontmatter.value = null
  body.value = ''

  if (!stationIdValid.value) {
    errorMessage.value = `station_id 格式不符（應為 s{NN}-slug）：${stationId.value}`
    loading.value = false
    return
  }
  if (!workspaceRoot.value) {
    errorMessage.value =
      '缺少 workspace_root（?ws_path 查詢參數）。請從 MOC 連結或 grant 流程進入。'
    loading.value = false
    return
  }

  try {
    const resolution = await stationRoute.resolveTaskId(
      workspaceRoot.value,
      queryTask.value
    )
    if (resolution.task_id === null) {
      errorMessage.value = '此 workspace 尚無已產出的教材，請先回首頁觸發 generate。'
      loading.value = false
      return
    }
    taskId.value = resolution.task_id

    const routeRaw = await files.readTutorialFile(
      workspaceRoot.value,
      `codebus-tutorials/${resolution.task_id}/route.json`
    )
    routeJson.value = JSON.parse(routeRaw) as RouteJson

    const station = stationRoute.findStation(routeJson.value, stationId.value)
    if (!station) {
      errorMessage.value = `route.json 找不到 station_id=${stationId.value}`
      loading.value = false
      return
    }

    await progress.loadProgress(workspaceRoot.value, resolution.task_id)
    progress.setRoute(routeJson.value)
    progress.setCurrentStation(stationId.value)

    if (!progress.canVisitStation(stationId.value, routeJson.value).value) {
      // Locked branch: still load route.json so StationNav renders, but
      // skip body fetch; the template branches on `reachable`.
      loading.value = false
      return
    }

    const stationRaw = await files.readTutorialFile(
      workspaceRoot.value,
      `codebus-tutorials/${resolution.task_id}/${station.file_path}`
    )
    const parsed = matter(stationRaw)
    if (!parsed.data || !parsed.data.station_id || !parsed.data.title) {
      errorMessage.value = '本站 frontmatter 損毀（缺 station_id 或 title）'
      loading.value = false
      return
    }
    frontmatter.value = {
      station_id: String(parsed.data.station_id),
      station_index: Number(parsed.data.station_index ?? station.index),
      title: String(parsed.data.title),
      duration_minutes: parsed.data.duration_minutes
        ? Number(parsed.data.duration_minutes)
        : station.duration,
      workspace_type: parsed.data.workspace_type as string | undefined,
      repo_name: parsed.data.repo_name as string | undefined,
      task: parsed.data.task as string | undefined,
      generated_at: parsed.data.generated_at as string | undefined,
      related_stations:
        (parsed.data.related_stations as string[] | undefined) ??
        station.related_stations,
      required_checks:
        (parsed.data.required_checks as string[] | undefined) ??
        station.required_checks,
      degraded: Boolean(parsed.data.degraded ?? station.degraded ?? false),
      schema_version: parsed.data.schema_version as number | undefined
    }
    body.value = parsed.content
  } catch (err) {
    errorMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

function navigateToStation(targetId: string): void {
  if (!workspaceRoot.value) return
  void router.push({
    path: `/tutorial/${workspaceId.value}/${targetId}`,
    query: {
      ws_path: workspaceRoot.value,
      ...(taskId.value ? { task: taskId.value } : {})
    }
  })
}

function navigateToMoc(): void {
  if (!workspaceRoot.value) return
  void router.push({
    path: `/tutorial/${workspaceId.value}`,
    query: { ws_path: workspaceRoot.value }
  })
}

watch(
  [workspaceId, stationId, workspaceRoot, queryTask],
  () => {
    void bootstrap()
  },
  { immediate: true }
)
</script>

<template>
  <div class="grid grid-cols-[260px_1fr] h-full">
    <StationNav
      v-if="routeJson"
      :route="routeJson"
      :current-station-id="stationId"
      :unlocked-station-ids="unlockedStationIds"
      :completed-station-ids="completedStationIds"
      @navigate="navigateToStation"
    />
    <div v-else class="border-r border-border-soft bg-surface-1" />

    <section class="overflow-y-auto bg-surface-0">
      <div
        v-if="loading"
        class="h-full grid place-items-center text-text-mute font-mono text-[12px]"
      >
        載入中…
      </div>

      <div
        v-else-if="errorMessage"
        class="h-full grid place-items-center px-12"
      >
        <div
          class="max-w-[520px] p-6 rounded-lg bg-surface-1 border border-border-soft"
        >
          <h2 class="text-text-base font-semibold text-[16px] mb-2">無法開啟此站</h2>
          <p class="text-text-dim text-[13.5px] leading-relaxed mb-4 whitespace-pre-line">
            {{ errorMessage }}
          </p>
          <button
            type="button"
            class="px-3 py-1.5 rounded-md text-[13px] bg-surface-3 text-text-base hover:bg-surface-2"
            @click="navigateToMoc"
          >
            回 MOC 首頁
          </button>
        </div>
      </div>

      <div
        v-else-if="!reachable"
        data-testid="locked-view"
        class="h-full grid place-items-center px-12"
      >
        <div
          class="max-w-[520px] p-6 rounded-lg bg-surface-1 border border-border-soft"
        >
          <div
            class="font-mono text-[10.5px] tracking-[0.16em] uppercase text-text-mute mb-2"
          >
            🔒 站點未解鎖
          </div>
          <h2 class="text-text-base font-semibold text-[16px] mb-2">
            這站尚未開放
          </h2>
          <p class="text-text-dim text-[13.5px] leading-relaxed mb-4">
            前面站還沒走完，先回 MOC 看路線並完成前面的 Checkpoint / Quiz。
          </p>
          <button
            type="button"
            class="px-3 py-1.5 rounded-md text-[13px] bg-surface-3 text-text-base hover:bg-surface-2"
            @click="navigateToMoc"
          >
            回 MOC 首頁
          </button>
        </div>
      </div>

      <template v-else-if="frontmatter && routeJson">
        <div
          v-if="isReviewMode"
          class="px-12 pt-6 max-w-[760px] mx-auto"
        >
          <span
            data-testid="review-badge"
            class="inline-block px-2 py-[2px] rounded font-mono text-[10.5px] bg-green/15 text-green tracking-wider"
          >
            REVIEW MODE · 已完成
          </span>
        </div>
        <StationLayout
          :frontmatter="frontmatter"
          :total-stations="routeJson.stations.length"
        >
          <StationContent :markdown="body" />
        </StationLayout>
      </template>
    </section>
  </div>
</template>
