<script setup lang="ts">
// QaCitations — citation list for one Q&A turn answer. Pure dumb display:
// renders file:line + station chips, emits `navigate-to-station` on chip
// click. file:line is NOT clickable in P0 (file open in side panel is
// Phase 2 per qa-agent.md §十一).
//
// Spec: openspec/changes/qa-overlay-p0/specs/qa-overlay/spec.md
//   "<QaCitations> renders citation list with station emit"

import type { Citation } from '~/composables/useQaSession'

defineProps<{
  citations: Citation[]
}>()

defineEmits<{
  (e: 'navigate-to-station', stationId: string): void
}>()
</script>

<template>
  <div v-if="citations.length > 0" class="flex flex-col gap-2 mt-2">
    <div
      v-for="(citation, idx) in citations"
      :key="`${citation.file_path}-${citation.line_start}-${idx}`"
      data-testid="citation-row"
      class="font-mono text-[11px] text-text-dim flex flex-wrap items-baseline gap-2"
    >
      <span data-testid="citation-file-line" class="text-text-base">
        {{ citation.file_path }}:{{ citation.line_start }}-{{ citation.line_end }}
      </span>
      <span
        v-for="stationId in citation.related_stations"
        :key="stationId"
        :data-station-id="stationId"
        class="px-2 py-[2px] rounded border border-accent text-accent cursor-pointer hover:bg-accent/10"
        @click="$emit('navigate-to-station', stationId)"
      >
        📍 {{ stationId }}
      </span>
    </div>
  </div>
</template>
