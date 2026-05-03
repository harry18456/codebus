<script setup lang="ts">
// `<WorkspaceOnrampCard>` — entry-page card that orchestrates the
// onramp UI. Pulls reactive state from the `useWorkspaceOnramp()`
// singleton and renders one of five phase branches:
//
//   idle                                    → picker prompt
//   scanning / indexing / exploring / …     → <OnrampProgress>
//   scan-complete                           → "+ 產生 tutorial" CTA
//   ready                                   → "進入 tutorial" NuxtLink
//   error                                   → errorMsg + retry CTA
//
// Backs SHALL clauses in
// openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//   Requirement: Entry page exposes folder-picker workspace onramp
//   Requirement: Workspace onramp drives scan, kb-build, explore, then generate via SSE

import { computed } from 'vue'

import { useWorkspaceOnramp } from '~/composables/useWorkspaceOnramp'
import OnrampProgress from './OnrampProgress.vue'

const onramp = useWorkspaceOnramp()

const inFlight = computed<boolean>(() =>
  ['scanning', 'indexing', 'exploring', 'generating'].includes(onramp.phase.value)
)

const pathTail = computed<string>(() => {
  const p = onramp.pickedPath.value
  if (!p) return ''
  // Posix slash both sides — the picker may yield Windows-style or
  // posix-style paths and we just want the leaf name to display.
  const normalized = p.replace(/\\/g, '/')
  const parts = normalized.split('/').filter(Boolean)
  return parts[parts.length - 1] ?? normalized
})

const tutorialHref = computed<string>(() => {
  const wid = onramp.workspaceId.value
  const path = onramp.pickedPath.value
  // The MOC page (pages/tutorial/[workspace_id]/index.vue) reads
  // tutorial files via `?ws_path=<absolute>`; without it the page
  // bails out and the user lands on an error UI. We have the path
  // in onramp state so propagate it through.
  if (!wid || !path) return '/'
  return `/tutorial/${wid}?ws_path=${encodeURIComponent(path)}`
})

async function onGenerate(): Promise<void> {
  await onramp.triggerGenerate()
}

async function onRetry(): Promise<void> {
  await onramp.retry()
}
</script>

<template>
  <section
    data-testid="workspace-onramp-card"
    class="flex flex-col gap-3 p-5 rounded-lg bg-surface-1 border border-border-base"
  >
    <div
      v-if="onramp.phase.value === 'idle'"
      data-testid="onramp-idle"
      class="flex flex-col gap-2 text-text-base"
    >
      <h2 class="text-base font-semibold">開始一個新的 codebase</h2>
      <p class="text-[13px] text-text-mute">
        選一個資料夾，CodeBus 會掃描並產生互動式 tutorial。
      </p>
    </div>

    <div
      v-else
      data-testid="onramp-active"
      class="flex flex-col gap-3"
    >
      <div class="flex items-baseline justify-between gap-2">
        <span
          class="text-[12.5px] text-text-base font-medium truncate"
          data-testid="onramp-path-tail"
        >
          {{ pathTail }}
        </span>
        <span
          class="text-[11.5px] text-text-mute font-mono"
          data-testid="onramp-workspace-id"
        >
          {{ onramp.workspaceId.value }}
        </span>
      </div>

      <OnrampProgress
        v-if="inFlight"
        :phase="onramp.phase.value"
        :events="onramp.progressEvents.value"
      />

      <button
        v-if="onramp.phase.value === 'scan-complete'"
        type="button"
        data-testid="onramp-generate-cta"
        class="rounded-md bg-accent text-surface-0 px-4 py-2 text-sm font-medium transition hover:opacity-90"
        @click="onGenerate"
      >
        + 產生 tutorial
      </button>

      <NuxtLink
        v-if="onramp.phase.value === 'ready'"
        data-testid="onramp-enter-tutorial"
        :to="tutorialHref"
        class="rounded-md bg-accent text-surface-0 px-4 py-2 text-sm font-medium text-center transition hover:opacity-90"
      >
        進入 tutorial
      </NuxtLink>

      <div
        v-if="onramp.phase.value === 'error'"
        class="flex flex-col gap-2"
      >
        <p
          class="text-[13px] text-text-base"
          data-testid="onramp-error-msg"
        >
          {{ onramp.errorMsg.value }}
        </p>
        <button
          type="button"
          data-testid="onramp-retry"
          class="rounded-md bg-surface-2 text-text-base px-4 py-2 text-sm font-medium border border-border-base transition hover:opacity-90"
          @click="onRetry"
        >
          重試
        </button>
      </div>
    </div>
  </section>
</template>
