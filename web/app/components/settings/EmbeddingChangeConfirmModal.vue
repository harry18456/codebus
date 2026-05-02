<script setup lang="ts">
// `<EmbeddingChangeConfirmModal>` — destructive confirmation gate for
// switching the `embed` role binding.
//
// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Requirement: Embedding switch goes through destructive confirm modal
//
// Visual / z-index distinct from `<InterventionConfirmModal>` per
// design Decision 6 invariant.

import { computed } from 'vue'

interface Props {
  open: boolean
  newProviderId: string | null
  currentChunkCount: number
}

const props = defineProps<Props>()
const emit = defineEmits<{ cancel: []; confirm: [] }>()

// Naive estimator — embedding throughput on consumer hardware lands
// roughly at ~50 chunks/sec for OpenAI text-embedding-3-small. We
// surface a friendly "minutes" label rather than a precise number
// because the rebuild also includes Qdrant upsert latency that
// varies wildly. A more accurate estimate lands in P1+ (D-033 §B).
const estimatedMinutes = computed(() => {
  const seconds = Math.max(1, props.currentChunkCount / 50)
  return Math.max(1, Math.round(seconds / 60))
})
</script>

<template>
  <div
    v-if="open"
    data-testid="embedding-confirm-dim"
    class="fixed inset-0 z-[70] grid place-items-center bg-rose-950/60"
    @click.self="emit('cancel')"
  >
    <aside
      data-testid="embedding-confirm-modal"
      class="max-w-[520px] w-full mx-4 p-6 rounded-lg bg-surface-1 border-2 border-rose-500 shadow-xl"
      @click.stop
    >
      <div
        class="font-mono text-[10.5px] tracking-[0.16em] uppercase text-rose-400 mb-2"
      >
        ⚠ destructive · embedding switch
      </div>
      <h3 class="text-[16px] font-semibold text-text-base mb-3">
        確定要重建知識庫嗎？
      </h3>
      <p class="text-[13.5px] text-text-dim leading-relaxed mb-2">
        切換 embedding provider 會把整個知識庫重新建立。目前狀態：
      </p>
      <ul class="text-[13px] text-text-base mb-3 ml-4 list-disc">
        <li data-testid="embedding-confirm-chunks">
          chunk 數：{{ currentChunkCount }}
        </li>
        <li data-testid="embedding-confirm-eta">
          預估重建時間：約 {{ estimatedMinutes }} 分鐘
        </li>
        <li>新 provider：{{ newProviderId }}</li>
      </ul>
      <p class="text-[12.5px] text-amber-300 mb-4">
        重建期間 Q&A / Generator / Scanner 會回
        <code>503 KB_REBUILD_IN_PROGRESS</code>，待重建完成後恢復。
      </p>
      <div class="flex justify-end gap-3">
        <button
          type="button"
          data-testid="embedding-confirm-cancel"
          class="px-3 py-1.5 rounded-md text-[13px] bg-surface-2 text-text-dim hover:bg-surface-3 hover:text-text-base"
          @click="emit('cancel')"
        >
          取消
        </button>
        <button
          type="button"
          data-testid="embedding-confirm-confirm"
          class="px-3 py-1.5 rounded-md text-[13px] bg-rose-500 text-white hover:bg-rose-600"
          @click="emit('confirm')"
        >
          重建並切換
        </button>
      </div>
    </aside>
  </div>
</template>
