<script setup lang="ts">
// CoverageBanner — spec: agent-console-p0. Renders 0..2 banners.
// Steps takes priority over tokens when both budget kinds are latched
// (per spec rule 3: "Steps takes priority when both kinds latched").
import { computed } from 'vue'
import type {
  BudgetBannerState,
  BudgetWarningEvent,
  CoverageBannerEvent
} from '~/composables/useExplorerStream'

const props = defineProps<{
  coverage: CoverageBannerEvent | null
  budget: BudgetBannerState
}>()

// Steps takes priority over tokens when both kinds are latched: pick at most
// one budget kind to display.
const activeBudget = computed<BudgetWarningEvent | null>(() => {
  if (props.budget.steps) return props.budget.steps
  if (props.budget.tokens) return props.budget.tokens
  return null
})

const coverageLabel = computed<string>(() => {
  const cov = props.coverage
  if (!cov) return ''
  switch (cov.skip_reason) {
    case 'no_gaps':
      return 'Coverage check found no gaps'
    case 'budget_exhausted':
      return 'Budget exhausted before coverage check could recurse'
    case 'max_depth_reached':
      return 'Reached max coverage recursion depth'
    case null:
      return `Coverage check found ${cov.gaps.length} gap(s) — exploring further`
  }
})

const coverageTone = computed<string>(() => {
  // will_recurse=true (skip_reason=null) is informational; use accent.
  // Skip reasons indicate coverage halted; use yellow warn tone.
  if (props.coverage && props.coverage.skip_reason === null) {
    return 'border-accent/40 bg-accent/10'
  }
  return 'border-yellow/30 bg-yellow/10'
})

const coverageLabelTone = computed<string>(() => {
  if (props.coverage && props.coverage.skip_reason === null) {
    return 'text-accent'
  }
  return 'text-yellow'
})

const budgetCopy = computed<string>(() => {
  const b = activeBudget.value
  if (!b) return ''
  const pct = Math.round(b.pct * 100)
  const unit = b.kind === 'steps' ? 'step' : 'token'
  return `已用 ${pct}% 的 ${unit} (${b.current}/${b.budget})`
})
</script>

<template>
  <div v-if="props.coverage || activeBudget" class="flex flex-col gap-2">
    <div
      v-if="props.coverage"
      data-banner="coverage"
      :class="['rounded-md border px-3 py-2 flex flex-col gap-1', coverageTone]"
    >
      <div
        :class="[
          'font-mono uppercase tracking-[0.1em] text-[10px]',
          coverageLabelTone
        ]"
      >
        coverage
      </div>
      <div class="text-text-dim text-[11.5px]">{{ coverageLabel }}</div>
    </div>

    <div
      v-if="activeBudget"
      :data-kind="activeBudget.kind"
      class="rounded-md border border-yellow/30 bg-yellow/10 px-3 py-2 flex flex-col gap-1"
    >
      <div
        class="text-yellow font-mono uppercase tracking-[0.1em] text-[10px]"
      >
        budget · {{ activeBudget.kind }}
      </div>
      <div class="text-text-dim text-[11.5px]">{{ budgetCopy }}</div>
    </div>
  </div>
</template>
