<script setup lang="ts">
// StepCard — renders one ReAct step's three beats (THINK / ACT / JUDGE) in
// fixed visual order. Each section is conditionally rendered; missing fields
// (judge not yet arrived, no actions) are simply hidden, not stubbed.
// Spec: openspec/changes/agent-console-p0/specs/agent-console/spec.md
//   "StepCard renders ReAct three beats in arrival order"

import { computed } from 'vue'
import type { StepBucket } from '~/composables/useExplorerStream'

const props = defineProps<{ bucket: StepBucket }>()

const OBS_TRUNCATION_THRESHOLD = 500
const EM_DASH = '—'

const stepLabel = computed(() => String(props.bucket.step).padStart(2, '0'))

const hasThink = computed(() => props.bucket.thought !== undefined)
const hasAct = computed(() => props.bucket.actions.length > 0)
const hasJudge = computed(() => props.bucket.judge !== undefined)

const judgeRelevance = computed(() =>
  props.bucket.judge ? props.bucket.judge.relevance.toFixed(2) : ''
)

function formatArgs(args: Record<string, unknown>): string {
  const entries = Object.entries(args)
  if (entries.length === 0) return ''
  return entries
    .map(([k, v]) => `${k}=${typeof v === 'string' ? `"${v}"` : JSON.stringify(v)}`)
    .join(' ')
}

function formatTokens(tokens: number): string {
  // Per spec: tokens_used === 0 means "no per-tool attribution available"
  // (P0 placeholder), NOT zero cost. Render em dash to avoid implying $0.
  if (tokens <= 0) return EM_DASH
  return `${tokens.toLocaleString('en-US')} tokens`
}

function isObservationTruncated(obs: string): boolean {
  return obs.length >= OBS_TRUNCATION_THRESHOLD
}
</script>

<template>
  <article
    class="bg-surface-1 border border-border-soft rounded-[10px] overflow-hidden"
    :data-step="props.bucket.step"
  >
    <header
      class="flex items-center gap-2 px-3 py-2 bg-surface-2 border-b border-border-soft"
    >
      <span
        class="font-mono text-[10.5px] text-text-mute px-2 py-[2px] rounded bg-surface-1"
      >
        step {{ stepLabel }}
      </span>
    </header>

    <section
      v-if="hasThink"
      data-testid="step-think"
      class="px-4 py-3 border-b border-border-soft"
    >
      <div class="flex items-center gap-2 mb-2">
        <span
          class="font-mono text-[10px] tracking-[0.14em] uppercase px-2 py-[2px] rounded bg-accent/20 text-accent"
        >
          THINK
        </span>
      </div>
      <p class="text-[13.5px] leading-[1.6] text-text-base whitespace-pre-wrap">
        {{ props.bucket.thought?.text }}
      </p>
      <ul
        v-if="props.bucket.thought && props.bucket.thought.actions.length > 0"
        class="mt-2 flex flex-col gap-1"
      >
        <li
          v-for="(call, i) in props.bucket.thought.actions"
          :key="i"
          class="font-mono text-[11.5px] text-text-dim"
        >
          <span class="text-purple">{{ call.tool }}</span>
          <span class="text-text-mute"> {{ formatArgs(call.args) }}</span>
        </li>
      </ul>
    </section>

    <section
      v-if="hasAct"
      data-testid="step-act"
      class="px-4 py-3 border-b border-border-soft"
    >
      <div class="flex items-center gap-2 mb-2">
        <span
          class="font-mono text-[10px] tracking-[0.14em] uppercase px-2 py-[2px] rounded bg-purple/20 text-purple"
        >
          ACT
        </span>
      </div>
      <ul class="flex flex-col gap-2">
        <li
          v-for="(entry, i) in props.bucket.actions"
          :key="i"
          data-testid="action-row"
          :data-state="entry.isError ? 'error' : 'ok'"
          :class="[
            'rounded-[7px] border bg-surface-2 overflow-hidden',
            entry.isError
              ? 'border-red/60 ring-1 ring-red/30'
              : 'border-border-soft'
          ]"
        >
          <div
            class="grid grid-cols-[auto_1fr_auto] gap-2 items-center px-3 py-2 font-mono text-[11.5px]"
          >
            <span class="text-purple">{{ entry.tool }}</span>
            <span class="text-text-dim truncate">{{ entry.observation }}</span>
            <span
              data-testid="action-tokens"
              :class="[
                'text-[10px] px-2 py-[2px] rounded',
                entry.tokens_used > 0
                  ? 'bg-surface-1 text-text-mute'
                  : 'bg-surface-1 text-text-mute'
              ]"
            >
              {{ formatTokens(entry.tokens_used) }}
            </span>
          </div>
          <div
            v-if="isObservationTruncated(entry.observation)"
            data-testid="obs-truncated"
            class="px-3 pb-2 font-mono text-[10.5px] text-text-mute"
          >
            …
          </div>
        </li>
      </ul>
    </section>

    <section
      v-if="hasJudge"
      data-testid="step-judge"
      class="px-4 py-3"
    >
      <div class="flex items-center gap-2 mb-2">
        <span
          class="font-mono text-[10px] tracking-[0.14em] uppercase px-2 py-[2px] rounded bg-green/20 text-green"
        >
          JUDGE
        </span>
      </div>
      <div class="grid grid-cols-[auto_1fr] gap-3 items-center">
        <div class="font-mono text-[22px] font-semibold text-green leading-none">
          {{ judgeRelevance }}
          <span class="text-[12px] text-text-mute font-normal"> / 1.0</span>
        </div>
        <p class="text-[12.5px] leading-[1.55] text-text-dim">
          {{ props.bucket.judge?.reason }}
        </p>
      </div>
    </section>
  </article>
</template>
