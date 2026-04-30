<script setup lang="ts">
// Confirm modal for the three Phase 6 intervention points (skip / regen
// / switch workspace). Subscribes to `useIntervention().pendingAction`
// and renders one of three copy variants. Mounted once at layout root
// (`layouts/default.vue`) — singleton modal per design Decision 4.

import { computed } from 'vue'

import { useIntervention } from '~/composables/useIntervention'

const intervention = useIntervention()

const action = computed(() => intervention.pendingAction.value)
const isOpen = computed(() => action.value !== null)

async function onConfirm(): Promise<void> {
  await intervention.confirm()
}

function onCancel(): void {
  intervention.cancel()
}
</script>

<template>
  <div
    v-if="isOpen"
    data-testid="intervention-modal-dim"
    class="fixed inset-0 z-[60] grid place-items-center bg-surface-0/80"
    @click.self="onCancel"
  >
    <aside
      data-testid="intervention-modal"
      class="max-w-[520px] w-full mx-4 p-6 rounded-lg bg-surface-1 border border-border-base shadow-xl"
      @click.stop
    >
        <template v-if="action?.kind === 'skip'">
          <div
            class="font-mono text-[10.5px] tracking-[0.16em] uppercase text-text-mute mb-2"
          >
            ↷ 介入點 · 跳過此站
          </div>
          <h3 class="text-[16px] font-semibold text-text-base mb-2">
            跳過「{{ action.payload.stationTitle }}」？
          </h3>
          <p class="text-[13.5px] text-text-dim leading-relaxed mb-4">
            跳過此站會解鎖下一站，但本站不會記為完成；隨時可重新進來學習。
          </p>
        </template>

        <template v-else-if="action?.kind === 'regen'">
          <div
            class="font-mono text-[10.5px] tracking-[0.16em] uppercase text-text-mute mb-2"
          >
            ↻ 介入點 · 重生此站
          </div>
          <h3 class="text-[16px] font-semibold text-text-base mb-2">
            重生「{{ action.payload.stationTitle }}」？
          </h3>
          <p class="text-[13.5px] text-text-dim leading-relaxed mb-4">
            重生會覆蓋本站 markdown 與 frontmatter，其他站與 MOC 不變。
            既有 Checkpoint / Quiz 進度會保留，但本站內容會由 LLM 重新撰寫。
          </p>
        </template>

        <template v-else-if="action?.kind === 'switch'">
          <div
            class="font-mono text-[10.5px] tracking-[0.16em] uppercase text-text-mute mb-2"
          >
            🔁 介入點 · 換資料夾
          </div>
          <h3 class="text-[16px] font-semibold text-text-base mb-2">
            切換到別的 workspace？
          </h3>
          <p class="text-[13.5px] text-text-dim leading-relaxed mb-4">
            進度按 workspace 路徑分開保存，回頭再選同一個資料夾就會繼續。
            新資料夾需要重新走 grant flow；之前授權過的會自動跳過 grant。
          </p>
        </template>

        <div class="flex justify-end gap-3">
          <button
            type="button"
            data-testid="intervention-cancel"
            class="px-3 py-1.5 rounded-md text-[13px] bg-surface-2 text-text-dim hover:bg-surface-3 hover:text-text-base"
            @click="onCancel"
          >
            取消
          </button>
          <button
            type="button"
            data-testid="intervention-confirm"
            class="px-3 py-1.5 rounded-md text-[13px] bg-accent text-surface-0 hover:opacity-90"
            @click="onConfirm"
          >
            確認
          </button>
        </div>
      </aside>
  </div>
</template>
