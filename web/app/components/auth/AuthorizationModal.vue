<script setup lang="ts">
import { computed } from 'vue'

import {
  type AuthStatusResponse,
  type GrantRequest,
  type GrantResponse,
  type GrantScenario,
  useSidecar
} from '../../composables/useSidecar'
import { useAuthorization } from '../../composables/useAuthorization'

interface AuthorizationModalProps {
  activeScenario: GrantScenario
  workspacePath: string
  fileCount: number
  dominantLanguages: string[]
  sanitizeKindCounts: Record<string, number>
  llmProvider: string
  llmModel: string
  outboundEndpoint: string
  sanitizerRulesVersion: string
  newKinds?: string[]
}

const props = withDefaults(defineProps<AuthorizationModalProps>(), {
  newKinds: () => []
})

const emit = defineEmits<{
  (e: 'denied'): void
  (e: 'granted', payload: GrantResponse): void
  (e: 'error', error: Error): void
}>()

const sidecar = useSidecar()
const flow = useAuthorization({
  scenario: props.activeScenario,
  llmProvider: props.llmProvider,
  newKinds: props.newKinds
})

const providerAckKey = computed(() => `outbound_to_${props.llmProvider}`)

const baseAckLabels: Record<string, string> = {
  raw_stays_local: '我了解原值留在本機，不會離開這台機器',
  no_kb_persist: '我了解清理後內容進 KB，原值不寫 KB'
}

function ackLabelFor(key: string): string {
  if (key in baseAckLabels) return baseAckLabels[key] as string
  if (key === providerAckKey.value) {
    return `我同意 CodeBus 將已清理內容送往 ${props.llmProvider}`
  }
  return key
}

function workspaceSourcePayload(): Record<string, unknown> {
  return { path: props.workspacePath }
}

function buildGrantRequest(): GrantRequest {
  return {
    workspace_type: 'folder',
    workspace_source: workspaceSourcePayload(),
    scenario: props.activeScenario,
    scope: {
      llm_provider: props.llmProvider,
      llm_model: props.llmModel,
      outbound_endpoint: props.outboundEndpoint
    },
    sanitizer_rules_version: props.sanitizerRulesVersion,
    user_ack: flow.buildUserAck()
  }
}

const sanitizeKindEntries = computed(() =>
  Object.entries(props.sanitizeKindCounts).filter(([, count]) => count > 0)
)

async function onCancel(): Promise<void> {
  try {
    await sidecar.deny({
      workspace_type: 'folder',
      workspace_source: workspaceSourcePayload(),
      scenario: props.activeScenario,
      reason: 'user_cancelled'
    })
  } catch (err) {
    emit('error', err as Error)
  } finally {
    emit('denied')
  }
}

async function onSubmit(): Promise<void> {
  if (!flow.submitEnabled.value) return
  try {
    const response = await sidecar.grant(buildGrantRequest())
    emit('granted', response)
  } catch (err) {
    emit('error', err as Error)
  }
}

defineExpose({
  flow,
  buildGrantRequest
})

const _typecheckUnused: AuthStatusResponse | null = null
void _typecheckUnused
</script>

<template>
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/55 px-4"
    role="dialog"
    aria-modal="true"
    aria-labelledby="authz-modal-title"
  >
    <div
      class="w-full max-w-[640px] rounded-xl border border-border-soft bg-surface-1 shadow-2xl"
    >
      <header
        class="px-6 pt-5 pb-4 border-b border-border-soft flex items-start gap-3"
      >
        <div
          class="w-9 h-9 rounded-lg bg-gradient-to-br from-accent to-accent-2 grid place-items-center text-surface-0 font-semibold"
        >
          ⚡
        </div>
        <div class="flex-1">
          <h2
            id="authz-modal-title"
            class="text-[15px] font-semibold tracking-tight text-text-base"
          >
            授權 CodeBus 開始 workspace
          </h2>
          <p class="mt-1 text-[12px] text-text-dim font-mono break-all">
            {{ workspacePath }}
          </p>
          <p class="mt-1 text-[11px] text-text-mute">
            <span>{{ fileCount }} files</span>
            <span v-if="dominantLanguages.length" class="ml-2">
              · {{ dominantLanguages.join(' / ') }}
            </span>
          </p>
        </div>
        <span
          class="text-[10.5px] uppercase tracking-wider px-2 py-0.5 rounded bg-surface-2 text-text-mute font-mono"
        >
          {{ activeScenario }}
        </span>
      </header>

      <section class="px-6 py-4 space-y-4">
        <!-- Sanitizer 類別預告 -->
        <div>
          <h3 class="text-[12px] font-semibold text-text-base">
            🛡️ Sanitizer 將在送 LLM 前替換以下類別
          </h3>
          <ul
            v-if="sanitizeKindEntries.length"
            class="mt-2 flex flex-wrap gap-2"
          >
            <li
              v-for="[kind, count] in sanitizeKindEntries"
              :key="kind"
              class="px-2.5 py-1 rounded-md bg-surface-2 border border-border-soft text-[11.5px] font-mono text-text-dim"
            >
              {{ kind }}<span class="ml-1 text-text-mute">({{ count }})</span>
            </li>
          </ul>
          <p v-else class="mt-2 text-[11.5px] text-text-mute">
            尚未偵測到敏感類別
          </p>
        </div>

        <!-- Hero line -->
        <p
          class="px-3 py-2.5 rounded-md bg-surface-2 border border-border-soft text-[13px] text-text-base font-medium"
        >
          🛡️ 原值留在本機 sidecar，不進 LLM、不寫進 KB
        </p>

        <!-- Provider 行 -->
        <div
          class="px-3 py-2.5 rounded-md bg-surface-2 border border-border-soft text-[12px] text-text-dim space-y-1"
        >
          <p>
            <span class="text-text-base font-medium">{{ llmProvider }}</span>
            ·
            <span class="font-mono">{{ llmModel }}</span>
          </p>
          <p class="text-[11px] text-text-mute font-mono">
            outbound HTTPS → {{ outboundEndpoint }}
          </p>
          <p class="text-[10.5px] text-text-mute font-mono">
            sanitizer rules: {{ sanitizerRulesVersion }}
          </p>
        </div>

        <!-- 三條基底 ack -->
        <fieldset class="space-y-2">
          <legend class="sr-only">同意條款</legend>
          <label
            v-for="key in flow.baseAckKeys"
            :key="key"
            class="flex items-start gap-2.5 px-3 py-2 rounded-md border border-border-soft bg-surface-2 hover:border-surface-4 cursor-pointer"
          >
            <input
              type="checkbox"
              :checked="flow.ackFlags.baseAcks[key]"
              :name="`base-ack-${key}`"
              class="mt-0.5 accent-accent"
              @change="(e) => flow.setAck(key, (e.target as HTMLInputElement).checked)"
            />
            <span class="text-[12.5px] text-text-base">
              {{ ackLabelFor(key) }}
            </span>
          </label>
        </fieldset>

        <!-- new_kind ack（scope_upgrade_new_kind 時才顯示）-->
        <fieldset
          v-if="activeScenario === 'scope_upgrade_new_kind' && (newKinds?.length ?? 0) > 0"
          class="space-y-2 pt-2 border-t border-border-soft"
        >
          <legend class="text-[12px] font-semibold text-text-base">
            ⚠️ 偵測到新類別
          </legend>
          <label
            v-for="kind in newKinds"
            :key="`new-${kind}`"
            class="flex items-start gap-2.5 px-3 py-2 rounded-md border border-accent-2/40 bg-surface-2 cursor-pointer"
          >
            <input
              type="checkbox"
              :checked="flow.ackFlags.newKindAcks[kind]"
              :name="`new-kind-ack-${kind}`"
              class="mt-0.5 accent-accent-2"
              @change="(e) => flow.setAck(kind, (e.target as HTMLInputElement).checked)"
            />
            <span class="text-[12.5px] text-text-base">
              我了解此 workspace 含
              <code
                class="px-1.5 py-0.5 mx-0.5 rounded bg-surface-3 text-accent-2 font-mono text-[11.5px]"
                >{{ kind }}</code
              >
              類內容，將被替換
            </span>
          </label>
        </fieldset>
      </section>

      <footer
        class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 bg-surface-1/60"
      >
        <button
          type="button"
          class="px-4 py-2 rounded-md text-[12.5px] text-text-dim bg-surface-3 hover:text-text-base"
          @click="onCancel"
        >
          先不啟用此 workspace
        </button>
        <button
          type="button"
          class="px-4 py-2 rounded-md text-[12.5px] font-medium text-surface-0 bg-gradient-to-b from-accent to-accent disabled:opacity-50 disabled:cursor-not-allowed"
          :disabled="!flow.submitEnabled.value"
          @click="onSubmit"
        >
          授權並開始
        </button>
      </footer>
    </div>
  </div>
</template>
