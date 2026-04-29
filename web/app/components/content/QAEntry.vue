<script setup lang="ts">
// QAEntry — mdc-auto-imported markdown trigger button. Click invokes
// `useQaSession().start(...)` to open the Q&A drawer overlay with the
// pre-filled prompt. The button itself does NOT fetch the sidecar — the
// composable is the sole network boundary, so the frontend-shell invariant
// "QAEntry MUST NOT itself fetch any sidecar endpoint; it is a navigation
// trigger only" stays satisfied (the imperative call is a navigation
// trigger into useQaSession).
//
// Spec: openspec/changes/qa-overlay-p0/specs/qa-overlay/spec.md
//   "<QAEntry> mdc element invokes useQaSession imperatively"
//
// `currentStationId` flows in via `inject` from the page-level provide in
// the R-01 station route — see design Decision (currentStationId via
// page-level provide rather than prop drill).

import { inject, isRef, type Ref } from 'vue'
import { useQaSession } from '~/composables/useQaSession'

const props = defineProps<{
  prompt: string
}>()

const session = useQaSession()
// Tests provide a plain string; the R-01 page provides a Ref<string> so the
// injection stays reactive across SPA route param changes (route reuses the
// same page instance). Accept either shape.
const injected = inject<string | Ref<string> | null>('currentStationId', null)

function resolveStationId(): string | null {
  if (injected === null || injected === undefined) return null
  if (isRef(injected)) {
    const v = injected.value
    return typeof v === 'string' && v.length > 0 ? v : null
  }
  return typeof injected === 'string' && injected.length > 0 ? injected : null
}

function handleClick(): void {
  // Open drawer first so the user immediately sees the question land in
  // the visible turn list; start() then drives the SSE pipeline.
  session.openDrawer()
  void session.start(props.prompt, resolveStationId())
}
</script>

<template>
  <button
    type="button"
    class="my-4 inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-accent text-surface-0 font-medium text-[13.5px] hover:opacity-90 transition-opacity"
    @click="handleClick"
  >
    <svg
      class="w-4 h-4"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    </svg>
    <span><slot>問 Q&amp;A Agent</slot></span>
  </button>
</template>
