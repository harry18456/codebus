<script setup lang="ts">
// ConsoleTimeline — iterate stepBuckets in step ascending order. Vue's `:key`
// is bound to bucket.step so late-arriving events upsert into the same DOM
// node instead of unmount/remount. Bucket-fill in useExplorerStream is the
// authoritative groupBy; this component never recomputes step grouping.
// Spec: openspec/changes/agent-console-p0/specs/agent-console/spec.md
//   "ConsoleTimeline iterates stepBuckets in step ascending order"

import { computed } from 'vue'
import StepCard from '~/components/console/StepCard.vue'
import type { StepBucket } from '~/composables/useExplorerStream'

const props = defineProps<{ stepBuckets: Map<number, StepBucket> }>()

const orderedBuckets = computed<StepBucket[]>(() =>
  Array.from(props.stepBuckets.values()).sort((a, b) => a.step - b.step)
)
</script>

<template>
  <div class="flex flex-col gap-3.5">
    <div
      v-if="orderedBuckets.length === 0"
      data-testid="timeline-placeholder"
      class="px-4 py-10 text-center text-text-mute font-mono text-[11.5px]"
    >
      等候 Explorer 開始決策…
    </div>
    <StepCard
      v-for="bucket in orderedBuckets"
      :key="bucket.step"
      :bucket="bucket"
    />
  </div>
</template>
