<script setup lang="ts">
import { useRouter } from 'vue-router'

const props = defineProps<{
  prompt: string
}>()

const router = useRouter()

function handleClick(): void {
  // P0 placeholder route — the Q&A page wiring lands in step 30
  // (`module-8-qa-p0` UI followup). QAEntry MUST NOT itself fetch any
  // sidecar endpoint; it is a navigation trigger only.
  void router.push(`/qa?prompt=${encodeURIComponent(props.prompt)}`)
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
