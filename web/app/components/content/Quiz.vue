<script setup lang="ts">
import { computed, onMounted, ref, useTemplateRef } from 'vue'

import { useTutorialProgress } from '~/composables/useTutorialProgress'

const props = defineProps<{
  id: string
  correct: 'a' | 'b' | 'c' | 'd'
}>()

const slotRef = useTemplateRef<HTMLDivElement>('slotRef')
const options = ref<{ letter: string; label: string }[]>([])
const selected = ref<string | null>(null)
const lastSubmitted = ref<string | null>(null)
const passed = ref(false)

const ID_RE = /^s\d+-q\d+$/
const idValid = ID_RE.test(props.id)

const showRetry = computed(
  () => lastSubmitted.value !== null && !passed.value
)

function parseOptions(): { letter: string; label: string }[] {
  const el = slotRef.value
  if (!el) return []
  const items = Array.from(el.querySelectorAll('li'))
  const parsed: { letter: string; label: string }[] = []
  for (const item of items) {
    const text = (item.textContent ?? '').trim()
    const match = text.match(/^([a-d])\)\s*(.+)$/)
    if (match && match[1] && match[2]) {
      parsed.push({ letter: match[1], label: match[2] })
    }
  }
  return parsed
}

function handleSubmit(): void {
  if (selected.value === null) return
  if (passed.value) return
  const isCorrect = selected.value === props.correct
  lastSubmitted.value = selected.value
  if (isCorrect) {
    passed.value = true
  }
  const progress = useTutorialProgress()
  progress.setQuizAnswer(props.id, selected.value, isCorrect)
}

onMounted(() => {
  if (!idValid) {
    // eslint-disable-next-line no-console
    console.warn(
      `<Quiz id="${props.id}"> does not match /^s\\d+-q\\d+$/ — rendering anyway`
    )
  }
  options.value = parseOptions()
  if (options.value.length === 0) {
    // eslint-disable-next-line no-console
    console.warn(
      `<Quiz id="${props.id}"> has no parseable options (expected '- a) ...' / '- b) ...' format)`
    )
  }
})
</script>

<template>
  <div class="my-6 p-4 rounded-lg bg-surface-1 border border-border-soft">
    <div class="flex items-center justify-between mb-2 font-mono text-[10.5px]">
      <span class="tracking-[0.14em] uppercase text-text-mute">Quiz</span>
      <span
        v-if="passed"
        data-testid="quiz-passed"
        class="px-2 py-[2px] rounded-full text-[11px] bg-green/20 text-green"
      >
        ✓ 答對
      </span>
    </div>

    <!-- Hidden slot ref used to parse markdown options at mount time -->
    <div ref="slotRef" class="hidden">
      <slot />
    </div>

    <fieldset class="space-y-2 text-[14px] text-text-dim" :disabled="passed">
      <label
        v-for="opt in options"
        :key="opt.letter"
        class="flex items-start gap-3 cursor-pointer p-2 rounded-md hover:bg-surface-2 transition-colors"
        :class="{ 'cursor-not-allowed': passed }"
      >
        <input
          v-model="selected"
          type="radio"
          :name="`quiz-${props.id}`"
          :value="opt.letter"
          class="mt-[5px] cursor-pointer accent-accent"
        />
        <span>
          <span class="font-mono text-text-mute mr-2">{{ opt.letter }})</span>
          {{ opt.label }}
        </span>
      </label>
    </fieldset>

    <div class="mt-3 flex items-center gap-3">
      <button
        type="button"
        class="px-3 py-1.5 rounded-md text-[12.5px] bg-accent text-surface-0 font-medium disabled:opacity-50 disabled:cursor-not-allowed"
        :disabled="selected === null || passed"
        @click="handleSubmit"
      >
        提交
      </button>
      <span
        v-if="showRetry"
        data-testid="quiz-retry"
        class="text-[12px] text-red font-mono"
      >
        再試一次
      </span>
    </div>
  </div>
</template>
