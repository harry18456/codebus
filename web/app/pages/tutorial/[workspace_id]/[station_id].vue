<script setup lang="ts">
// Single station page. URL key is the D-029 stable station id;
// `?ws_path=` carries the absolute workspace path (same query the index
// page consumes). The page calls `useTutorialProgress.canVisitStation`
// to gate access — already-completed stations are reachable in review
// mode regardless of the unlock-forward window.

import { computed, provide, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import AuditPanel, {
  type AuditRow,
  type AuditTab
} from '~/components/audit/AuditPanel.vue'
import LlmCallInspector from '~/components/audit/LlmCallInspector.vue'
import SanitizerAuditInspector from '~/components/audit/SanitizerAuditInspector.vue'
import RegenStationButton from '~/components/intervention/RegenStationButton.vue'
import SkipStationButton from '~/components/intervention/SkipStationButton.vue'
import StationContent from '~/components/tutorial/StationContent.vue'
import StationLayout, {
  type StationFrontmatter
} from '~/components/tutorial/StationLayout.vue'
import StationNav from '~/components/tutorial/StationNav.vue'
import {
  useAuditJsonl,
  type LlmCallEntry,
  type UseAuditJsonlApi
} from '~/composables/useAuditJsonl'
import { parseFrontmatter } from '~/composables/parseFrontmatter'
import {
  useSanitizeAudit,
  type SanitizeAuditEntry,
  type UseSanitizeAuditApi
} from '~/composables/useSanitizeAudit'
import { useSidecar } from '~/composables/useSidecar'
import { useSseTask } from '~/composables/useSseTask'
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

// `qa-overlay-p0`: mdc-auto-imported `<QAEntry>` reads `currentStationId`
// via `inject` to decide which station id to attach to the Q&A turn. The
// page is the only level that knows the routing context, so this provide
// is the canonical injection point — composables MUST NOT read `useRoute()`
// to derive station id.
provide('currentStationId', stationId)

const completedStationIds = computed(() => progress.state.value.completed_station_ids)
const skippedStationIds = computed(() => progress.state.value.skipped_station_ids)
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

const currentStationIndex = computed<number>(() => {
  if (!routeJson.value) return -1
  return routeJson.value.stations.findIndex((s) => s.station_id === stationId.value)
})

const prevStation = computed(() => {
  const idx = currentStationIndex.value
  if (idx <= 0 || !routeJson.value) return null
  return routeJson.value.stations[idx - 1] ?? null
})

const nextStation = computed(() => {
  const idx = currentStationIndex.value
  if (idx < 0 || !routeJson.value) return null
  return routeJson.value.stations[idx + 1] ?? null
})

const nextReachable = computed(() => {
  if (!nextStation.value || !routeJson.value) return false
  return progress.canVisitStation(nextStation.value.station_id, routeJson.value).value
})

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
    const parsed = parseFrontmatter(stationRaw)
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

// ---- Per-station regen wiring (intervention point 2) ----
//
// When the user confirms the regen modal, the page issues
// `POST /generate` with `target_stations=[stationId]`, attaches
// useSseTask to the returned task_id, and on `done` re-reads the
// station markdown so `<StationContent>` re-renders the fresh body.
const regenStatus = ref<'idle' | 'pending' | 'running' | 'done' | 'error'>(
  'idle'
)
const regenError = ref<string | null>(null)
const regenSseStop = ref<(() => void) | null>(null)

async function startRegen(stationIdToRegen: string): Promise<void> {
  if (!workspaceRoot.value || !taskId.value || !routeJson.value) return
  regenStatus.value = 'pending'
  regenError.value = null
  if (regenSseStop.value) regenSseStop.value()

  const sidecar = useSidecar()
  let res: Response
  try {
    res = await sidecar.fetch('/generate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        workspace_root: workspaceRoot.value,
        task: frontmatter.value?.task ?? '',
        stations: [],
        target_stations: [stationIdToRegen]
      })
    })
  } catch (err) {
    regenStatus.value = 'error'
    regenError.value = err instanceof Error ? err.message : String(err)
    return
  }
  if (!res.ok) {
    regenStatus.value = 'error'
    regenError.value = `POST /generate failed (${res.status})`
    return
  }
  const body = (await res.json()) as { task_id?: string }
  if (typeof body.task_id !== 'string') {
    regenStatus.value = 'error'
    regenError.value = 'POST /generate response missing task_id'
    return
  }
  regenStatus.value = 'running'
  const sse = useSseTask(body.task_id)

  const stopWatch = watch(
    () => sse.events.value.length,
    () => {
      const last = sse.events.value[sse.events.value.length - 1]
      if (!last) return
      if (last.type === 'done') {
        regenStatus.value = 'done'
        // Re-read station markdown so <StationContent> picks up new body.
        void reloadStationMarkdown(stationIdToRegen)
        sse.close()
        stopWatch()
      } else if (last.type === 'error') {
        regenStatus.value = 'error'
        const data = last.data as { code?: string; message?: string }
        regenError.value = data?.message ?? 'regen error'
        sse.close()
        stopWatch()
      }
    },
    { immediate: false }
  )
  regenSseStop.value = () => {
    sse.close()
    stopWatch()
  }
}

async function reloadStationMarkdown(stationIdToRegen: string): Promise<void> {
  if (!workspaceRoot.value || !taskId.value || !routeJson.value) return
  const station = stationRoute.findStation(routeJson.value, stationIdToRegen)
  if (!station) return
  try {
    const stationRaw = await files.readTutorialFile(
      workspaceRoot.value,
      `codebus-tutorials/${taskId.value}/${station.file_path}`
    )
    const parsed = parseFrontmatter(stationRaw)
    if (!parsed.data || !parsed.data.station_id || !parsed.data.title) return
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
  } catch {
    // Best-effort: if read fails after regen, leave old content visible.
  }
}

function onRegenRequested(stationIdToRegen: string): void {
  // RegenStationButton's `requested-regen` emit fires from the
  // intervention modal's onConfirm closure; the page handles the
  // sidecar-side wiring (POST /generate + SSE + re-read).
  void startRegen(stationIdToRegen)
}

// ---- Audit panel + per-tab inspector overlays (R-01 station chrome) ----
//
// The station page hosts the audit rail via the layout's `audit` slot
// override below; the Sanitizer + LLM overlays mount at page root so
// either can fly over the workspace surface.
const sanitizeAudit = shallowRef<UseSanitizeAuditApi | null>(null)
const llmAudit = shallowRef<UseAuditJsonlApi<LlmCallEntry> | null>(null)
const auditTab = ref<AuditTab>('sanitize')
const llmInspectorIndex = ref<number | null>(null)
const sanitizeInspectorIndex = ref<number | null>(null)

watch(
  workspaceRoot,
  (path) => {
    if (path) {
      sanitizeAudit.value = useSanitizeAudit(path)
      llmAudit.value = useAuditJsonl<LlmCallEntry>(path, 'llm')
    } else {
      sanitizeAudit.value = null
      llmAudit.value = null
    }
  },
  { immediate: true }
)

const sanitizeRowsAsAuditRows = computed<AuditRow[]>(() => {
  const entries = sanitizeAudit.value?.entries.value ?? []
  return entries
    .slice()
    .reverse()
    .map((e) => {
      const tsRaw = typeof e.ts === 'string' ? e.ts : ''
      const ts = tsRaw.includes('T')
        ? (tsRaw.split('T')[1]?.slice(0, 8) ?? tsRaw)
        : tsRaw || '—'
      return {
        ts,
        body: e.rule_id,
        rule_id: e.rule_id,
        kind: e.kind,
        placeholder_index: e.placeholder_index,
        pass: e.pass
      }
    })
})

const llmRowsAsAuditRows = computed<AuditRow[]>(() => {
  const entries = llmAudit.value?.entries.value ?? []
  return entries
    .slice()
    .reverse()
    .map((e) => {
      const tsRaw = typeof e.timestamp === 'string' ? e.timestamp : ''
      const ts = tsRaw.includes('T')
        ? (tsRaw.split('T')[1]?.slice(0, 8) ?? tsRaw)
        : tsRaw || '—'
      const prompt = e.prompt_tokens ?? 0
      const completion = e.completion_tokens ?? 0
      return {
        ts,
        body: `${e.role} · ${e.module ?? '—'} · ${e.model} · ${prompt + completion}t`,
        badge: e.sanitizer_pass2_applied ? 'sanitize' : undefined,
        badgeKind: e.sanitizer_pass2_applied ? ('purple' as const) : undefined
      }
    })
})

const tabRows = computed<AuditRow[]>(() => {
  if (auditTab.value === 'sanitize') return sanitizeRowsAsAuditRows.value
  if (auditTab.value === 'llm') return llmRowsAsAuditRows.value
  return []
})
const tabCounts = computed(() => ({
  sanitize: sanitizeAudit.value?.entries.value.length ?? 0,
  llm: llmAudit.value?.entries.value.length ?? 0
}))

const currentSanitizeRow = computed<SanitizeAuditEntry | null>(() => {
  if (sanitizeAudit.value === null || sanitizeInspectorIndex.value === null) {
    return null
  }
  return sanitizeAudit.value.entries.value[sanitizeInspectorIndex.value] ?? null
})

function selectAuditTab(tab: AuditTab): void {
  auditTab.value = tab
  if (tab !== 'llm') llmInspectorIndex.value = null
  if (tab !== 'sanitize') sanitizeInspectorIndex.value = null
}

function onAuditRowSelect(displayIndex: number): void {
  if (auditTab.value === 'sanitize' && sanitizeAudit.value) {
    const total = sanitizeAudit.value.entries.value.length
    sanitizeInspectorIndex.value = total - 1 - displayIndex
    return
  }
  if (auditTab.value === 'llm' && llmAudit.value) {
    const total = llmAudit.value.entries.value.length
    llmInspectorIndex.value = total - 1 - displayIndex
    return
  }
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
  <div class="grid grid-cols-[260px_1fr_360px] h-full">
    <StationNav
      v-if="routeJson"
      :route="routeJson"
      :current-station-id="stationId"
      :unlocked-station-ids="unlockedStationIds"
      :completed-station-ids="completedStationIds"
      :skipped-station-ids="skippedStationIds"
      @navigate="navigateToStation"
      @navigate-to-moc="navigateToMoc"
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
          <template #header-actions>
            <SkipStationButton
              :station-id="stationId"
              :station-title="frontmatter.title"
              :route="routeJson"
              :workspace-id="workspaceId"
              :workspace-root="workspaceRoot"
              :task-id="taskId"
            />
            <RegenStationButton
              v-if="taskId && workspaceRoot"
              :station-id="stationId"
              :station-title="frontmatter.title"
              :task-id="taskId"
              :workspace-root="workspaceRoot"
              :degraded="frontmatter.degraded"
              @requested-regen="onRegenRequested"
            />
          </template>
          <StationContent :markdown="body" />
          <footer
            class="mt-10 pt-6 border-t border-border-soft grid grid-cols-2 gap-4"
          >
            <button
              v-if="prevStation"
              type="button"
              class="text-left p-4 rounded-lg bg-surface-1 border border-border-soft hover:border-accent transition-colors"
              data-testid="pager-prev"
              @click="navigateToStation(prevStation.station_id)"
            >
              <div
                class="font-mono text-[9.5px] tracking-[0.14em] uppercase text-text-mute mb-1"
              >
                ← 上一站
              </div>
              <div class="text-[14px] text-text-base">
                {{ prevStation.title }}
              </div>
            </button>
            <div v-else />
            <button
              v-if="nextStation"
              type="button"
              class="text-right p-4 rounded-lg bg-surface-1 border border-border-soft transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              :class="
                nextReachable
                  ? 'hover:border-accent cursor-pointer'
                  : 'cursor-not-allowed'
              "
              :disabled="!nextReachable"
              data-testid="pager-next"
              @click="nextReachable && navigateToStation(nextStation.station_id)"
            >
              <div
                class="font-mono text-[9.5px] tracking-[0.14em] uppercase text-text-mute mb-1"
              >
                {{ nextReachable ? '下一站 →' : '🔒 完成本站才解鎖' }}
              </div>
              <div class="text-[14px] text-text-base">
                {{ nextStation.title }}
              </div>
            </button>
            <div v-else class="text-right p-4 text-text-mute font-mono text-[10.5px]">
              已是最後一站
            </div>
          </footer>
        </StationLayout>
      </template>
    </section>

    <aside class="border-l border-border-base flex flex-col bg-surface-1 min-h-0">
      <AuditPanel
        :active-tab="auditTab"
        :rows="tabRows"
        :counts="tabCounts"
        @select-tab="selectAuditTab"
        @select-row="onAuditRowSelect"
      />
    </aside>
  </div>

  <SanitizerAuditInspector
    v-if="currentSanitizeRow !== null"
    :row="currentSanitizeRow"
    @close="sanitizeInspectorIndex = null"
  />
  <LlmCallInspector
    :rows="llmAudit?.entries.value ?? []"
    :active-index="llmInspectorIndex"
    @close="llmInspectorIndex = null"
    @select-index="llmInspectorIndex = $event"
  />
</template>
