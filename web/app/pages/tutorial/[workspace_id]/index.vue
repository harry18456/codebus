<script setup lang="ts">
// MOC index page. Drives D-T11 implicit-latest task resolution and
// renders empty CTA (D-T13) when no task directories exist. The
// workspace_root absolute path comes via the `?ws_path=<encoded>` query
// — grant.vue passes it on `granted` redirect; deep links from
// elsewhere need the same query for the page to read filesystem paths
// through the Tauri command.

import matter from 'gray-matter'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import MOCIndex from '~/components/tutorial/MOCIndex.vue'
import StationNav from '~/components/tutorial/StationNav.vue'
import { useStationRoute, type RouteJson } from '~/composables/useStationRoute'
import { useTutorialFiles } from '~/composables/useTutorialFiles'
import { useTutorialProgress } from '~/composables/useTutorialProgress'

const route = useRoute()
const router = useRouter()
const files = useTutorialFiles()
const stationRoute = useStationRoute()
const progress = useTutorialProgress()

const loading = ref(true)
const errorMessage = ref<string | null>(null)
const taskId = ref<string | null>(null)
const taskSource = ref<'query' | 'single' | 'latest' | 'empty' | null>(null)
const routeJson = ref<RouteJson | null>(null)
const mocBody = ref('')

const workspaceId = computed(() => String(route.params.workspace_id ?? ''))
const workspaceRoot = computed(() => {
  const raw = route.query.ws_path
  if (typeof raw !== 'string' || raw.length === 0) return null
  return raw
})
const queryTask = computed(() => {
  const raw = route.query.task
  return typeof raw === 'string' && raw.length > 0 ? raw : null
})

const completedStationIds = computed(() => progress.state.value.completed_station_ids)
const unlockedStationIds = computed(() =>
  routeJson.value
    ? progress.unlockedStationIds(routeJson.value).value
    : new Set<string>()
)

async function bootstrap(): Promise<void> {
  loading.value = true
  errorMessage.value = null
  routeJson.value = null
  mocBody.value = ''
  taskId.value = null
  taskSource.value = null

  if (!workspaceRoot.value) {
    errorMessage.value =
      '缺少 workspace_root（?ws_path 查詢參數）。請從 grant 流程進入，或在 URL 末加上 `?ws_path=<絕對路徑>`。'
    loading.value = false
    return
  }

  try {
    const resolution = await stationRoute.resolveTaskId(
      workspaceRoot.value,
      queryTask.value
    )
    taskId.value = resolution.task_id
    taskSource.value = resolution.source
    if (resolution.source === 'empty' || resolution.task_id === null) {
      loading.value = false
      return
    }
    const [routeRaw, mocRaw] = await Promise.all([
      files.readTutorialFile(
        workspaceRoot.value,
        `codebus-tutorials/${resolution.task_id}/route.json`
      ),
      files.readTutorialFile(
        workspaceRoot.value,
        `codebus-tutorials/${resolution.task_id}/tutorial.md`
      )
    ])
    routeJson.value = JSON.parse(routeRaw) as RouteJson
    const parsedMoc = matter(mocRaw)
    mocBody.value = parsedMoc.content
    await progress.loadProgress(workspaceRoot.value, resolution.task_id)
    progress.setRoute(routeJson.value)
  } catch (err) {
    errorMessage.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

function navigateToStation(stationId: string): void {
  if (!workspaceRoot.value) return
  void router.push({
    path: `/tutorial/${workspaceId.value}/${stationId}`,
    query: {
      ws_path: workspaceRoot.value,
      ...(taskId.value ? { task: taskId.value } : {})
    }
  })
}

watch(
  [workspaceId, workspaceRoot, queryTask],
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
      :current-station-id="null"
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
          <h2 class="text-text-base font-semibold text-[16px] mb-2">無法載入教材</h2>
          <p class="text-text-dim text-[13.5px] leading-relaxed whitespace-pre-line">
            {{ errorMessage }}
          </p>
        </div>
      </div>

      <div
        v-else-if="taskSource === 'empty'"
        data-testid="empty-cta"
        class="h-full grid place-items-center px-12"
      >
        <div
          class="max-w-[640px] p-6 rounded-lg bg-surface-1 border border-border-soft"
        >
          <div
            class="font-mono text-[10.5px] tracking-[0.16em] uppercase text-text-mute mb-2"
          >
            尚無教材
          </div>
          <h2 class="text-text-base font-semibold text-[18px] mb-3">
            此 workspace 尚無已產出的教材
          </h2>
          <p class="text-text-dim text-[13.5px] leading-relaxed mb-4">
            下一步：呼叫 <code class="font-mono text-accent">POST /generate</code>
            觸發產生（執行頁面落在後續 change 步驟 28+）。
          </p>
          <pre
            class="font-mono text-[11.5px] bg-surface-2 text-text-base p-3 rounded-md overflow-x-auto leading-relaxed"
          ><code>curl -X POST -H "Authorization: Bearer &lt;token&gt;" \
       http://127.0.0.1:&lt;port&gt;/generate \
       -d '{ "workspace_root": "&lt;path&gt;", "task": "&lt;question&gt;", "stations": [...] }'</code></pre>
        </div>
      </div>

      <MOCIndex
        v-else-if="routeJson"
        :moc-markdown="mocBody"
        :workspace-id="workspaceId"
        :route="routeJson"
        :unlocked-station-ids="unlockedStationIds"
        :completed-station-ids="completedStationIds"
        @navigate="navigateToStation"
      />
    </section>
  </div>
</template>
