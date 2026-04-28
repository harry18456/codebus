<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import AuthorizationModal from '../../components/auth/AuthorizationModal.vue'
import {
  type AuthStatusResponse,
  type GrantResponse,
  type GrantScenario,
  useSidecar
} from '../../composables/useSidecar'

definePageMeta({
  layout: 'default'
})

interface ScanFile {
  path: string
  language?: string | null
  sanitize_stats?: Record<string, number>
}

interface ScanResultLite {
  files?: ScanFile[]
}

const sidecar = useSidecar()
const route = useRoute()
const router = useRouter()

const isLoading = ref(true)
const errorMessage = ref<string | null>(null)
const workspacePath = ref('')
const sanitizeKindCounts = ref<Record<string, number>>({})
const fileCount = ref(0)
const dominantLanguages = ref<string[]>([])
const lastGrant = ref<AuthStatusResponse['last_grant']>(null)
const sanitizerRulesVersion = ref('unknown')

// Default scope for first cut. The Settings page (Phase 6 step 28+) will
// expose provider/model selection; for now the modal pins one provider.
const llmProvider = 'anthropic'
const llmModel = 'claude-haiku-4.5'
const outboundEndpoint = 'api.anthropic.com'

const activeScenario = computed<GrantScenario>(() => {
  if (!lastGrant.value) return 'first_run'
  const userAck = (lastGrant.value.user_ack as string[] | undefined) ?? []
  const ackedKinds = new Set(
    userAck
      .filter((flag) => flag.startsWith('new_kind:'))
      .map((flag) => flag.slice('new_kind:'.length))
  )
  const detectedKinds = Object.keys(sanitizeKindCounts.value).filter(
    (k) => (sanitizeKindCounts.value[k] ?? 0) > 0
  )
  const newKinds = detectedKinds.filter((k) => !ackedKinds.has(k))
  return newKinds.length > 0 ? 'scope_upgrade_new_kind' : 'scope_reconfirm'
})

const newKinds = computed(() => {
  if (activeScenario.value !== 'scope_upgrade_new_kind') return []
  const userAck =
    (lastGrant.value?.user_ack as string[] | undefined) ?? []
  const acked = new Set(
    userAck
      .filter((flag) => flag.startsWith('new_kind:'))
      .map((flag) => flag.slice('new_kind:'.length))
  )
  return Object.keys(sanitizeKindCounts.value)
    .filter((k) => (sanitizeKindCounts.value[k] ?? 0) > 0)
    .filter((k) => !acked.has(k))
})

function aggregateSanitizeStats(files: ScanFile[]): Record<string, number> {
  const out: Record<string, number> = {}
  for (const f of files) {
    const stats = f.sanitize_stats ?? {}
    for (const [kind, count] of Object.entries(stats)) {
      out[kind] = (out[kind] ?? 0) + count
    }
  }
  return out
}

function pickDominantLanguages(files: ScanFile[], top = 3): string[] {
  const counts: Record<string, number> = {}
  for (const f of files) {
    if (typeof f.language === 'string' && f.language.length > 0) {
      counts[f.language] = (counts[f.language] ?? 0) + 1
    }
  }
  return Object.entries(counts)
    .sort((a, b) => b[1] - a[1])
    .slice(0, top)
    .map(([lang]) => lang)
}

async function loadScanAndStatus(path: string): Promise<void> {
  errorMessage.value = null
  isLoading.value = true
  try {
    const scanRes = await sidecar.fetch('/scan', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        workspace_type: 'folder',
        workspace_root: path
      })
    })
    if (!scanRes.ok) {
      throw new Error(`scan failed: ${scanRes.status}`)
    }
    const scan = (await scanRes.json()) as ScanResultLite
    const files = scan.files ?? []
    fileCount.value = files.length
    sanitizeKindCounts.value = aggregateSanitizeStats(files)
    dominantLanguages.value = pickDominantLanguages(files)

    const workspaceId = await sidecar.workspaceIdForPath(path)
    const statusResp = await sidecar.status(workspaceId)
    lastGrant.value = statusResp.last_grant
    sanitizerRulesVersion.value = statusResp.current_rules_version
  } catch (err) {
    errorMessage.value = (err as Error).message
  } finally {
    isLoading.value = false
  }
}

onMounted(async () => {
  const queryPath = route.query.path
  if (typeof queryPath !== 'string' || !queryPath) {
    errorMessage.value = 'workspace path 缺失：請從首頁挑選資料夾再進來'
    isLoading.value = false
    return
  }
  workspacePath.value = queryPath
  await loadScanAndStatus(queryPath)
})

function onDenied(): void {
  void router.push('/')
}

function onGranted(payload: GrantResponse): void {
  // R-01 station-board MOC index. The page resolves task_id implicitly
  // (D-T11): empty workspace shows generate CTA (D-T13); single / multi
  // task picks the latest. ws_path query carries the absolute workspace
  // path so Tauri commands can read codebus-tutorials/ filesystem entries
  // — the workspace_id alone is a one-way SHA-256 prefix and cannot be
  // reversed to a path.
  // Nuxt 4 file routing maps `pages/tutorial/[workspace_id]/index.vue`
  // to URL `/tutorial/:workspace_id` (no `/index` segment) — the
  // trailing `/index` would route into `[station_id].vue` with
  // station_id="index" and trip the regex guard.
  void router.push({
    path: `/tutorial/${payload.workspace_id}`,
    query: { ws_path: workspacePath.value }
  })
}

function onModalError(err: Error): void {
  errorMessage.value = err.message
}
</script>

<template>
  <main class="min-h-screen bg-surface-0 text-text-base">
    <div v-if="isLoading" class="grid place-items-center min-h-screen">
      <p class="text-text-mute text-sm">載入 workspace 中…</p>
    </div>

    <div v-else-if="errorMessage" class="grid place-items-center min-h-screen">
      <div class="max-w-md px-6 py-5 rounded-lg bg-surface-1 border border-border-soft">
        <h2 class="text-text-base font-semibold">載入失敗</h2>
        <p class="mt-2 text-text-dim text-sm">{{ errorMessage }}</p>
        <button
          class="mt-4 px-3 py-1.5 rounded-md text-sm bg-surface-3 text-text-base hover:bg-surface-2"
          @click="onDenied"
        >
          回首頁
        </button>
      </div>
    </div>

    <AuthorizationModal
      v-else
      :active-scenario="activeScenario"
      :workspace-path="workspacePath"
      :file-count="fileCount"
      :dominant-languages="dominantLanguages"
      :sanitize-kind-counts="sanitizeKindCounts"
      :llm-provider="llmProvider"
      :llm-model="llmModel"
      :outbound-endpoint="outboundEndpoint"
      :sanitizer-rules-version="sanitizerRulesVersion"
      :new-kinds="newKinds"
      @denied="onDenied"
      @granted="onGranted"
      @error="onModalError"
    />
  </main>
</template>
