<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, useTemplateRef, watch } from 'vue'

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
const contractError = ref<string | null>(null)

const ID_RE = /^s\d+-q\d+$/
const idValid = ID_RE.test(props.id)

const showRetry = computed(
  () => lastSubmitted.value !== null && !passed.value && contractError.value === null
)

let observer: MutationObserver | null = null
let parsedOnce = false

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

function refresh(): void {
  // H3 fix: re-parse whenever the mdc-rendered slot DOM mutates so we
  // do not race against async markdown rendering.
  const next = parseOptions()
  if (next.length === 0) return // wait for mdc
  options.value = next
  parsedOnce = true

  // H6 fix: validate that `correct` actually exists among parsed
  // option letters; otherwise the user can never pass and the page
  // looks broken with no diagnostic.
  const letters = new Set(next.map((o) => o.letter))
  if (!letters.has(props.correct)) {
    contractError.value = `Quiz id="${props.id}" 標 correct="${props.correct}" 但 options 只有 [${[...letters].join(', ')}] — Generator 端 markdown 契約異常`
    // eslint-disable-next-line no-console
    console.error(contractError.value)
  } else {
    contractError.value = null
  }
}

function handleSubmit(): void {
  if (selected.value === null) return
  if (passed.value) return
  if (contractError.value !== null) return
  const isCorrect = selected.value === props.correct
  lastSubmitted.value = selected.value
  if (isCorrect) {
    passed.value = true
  }
  const progress = useTutorialProgress()
  progress.setQuizAnswer(props.id, selected.value, isCorrect)
}

function restoreFromProgress(): void {
  // Spec scenario "Already-completed station revisitable via URL paste"
  // implies review-mode visuals: when the user re-enters a station
  // they have already passed, the Quiz should reflect their answer
  // instead of asking them to re-pick.
  const progress = useTutorialProgress()
  const existing = progress.state.value.quizzes[props.id]
  if (!existing) return
  selected.value = existing.answer
  lastSubmitted.value = existing.answer
  if (existing.correct) {
    passed.value = true
  }
}

onMounted(() => {
  if (!idValid) {
    // eslint-disable-next-line no-console
    console.warn(
      `<Quiz id="${props.id}"> does not match /^s\\d+-q\\d+$/ — rendering anyway`
    )
  }
  refresh()
  restoreFromProgress()
  if (slotRef.value) {
    observer = new MutationObserver(() => refresh())
    observer.observe(slotRef.value, { childList: true, subtree: true, characterData: true })
  }
  if (!parsedOnce) {
    // mdc has not produced <li> yet; the observer above will retry on
    // the next mutation. Give the user a single dev-mode breadcrumb
    // until that happens.
    // eslint-disable-next-line no-console
    console.warn(
      `<Quiz id="${props.id}"> waiting on mdc to render '- a) ...' / '- b) ...' options`
    )
  }
})

onBeforeUnmount(() => {
  if (observer) {
    observer.disconnect()
    observer = null
  }
})

watch(
  () => props.id,
  () => {
    parsedOnce = false
    selected.value = null
    lastSubmitted.value = null
    passed.value = false
    refresh()
  }
)
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

    <!-- Slot renders the question paragraph(s) for the user, while the
         `- a) ... / - b) ...` option list is hidden via scoped CSS so
         we can re-render it as radio buttons below. parseOptions still
         walks the slot DOM (display:none doesn't affect the DOM tree)
         and extracts the option letters + labels. -->
    <div ref="slotRef" class="quiz-body text-text-dim text-[14px] mb-3">
      <slot />
    </div>

    <div
      v-if="contractError"
      data-testid="quiz-contract-error"
      class="p-3 rounded-md text-[12.5px] bg-red/10 text-red font-mono leading-relaxed"
    >
      {{ contractError }}
    </div>

    <fieldset
      v-else
      class="space-y-2 text-[14px] text-text-dim"
      :disabled="passed"
    >
      <label
        v-for="opt in options"
        :key="opt.letter"
        class="flex items-start gap-3 p-2 rounded-md transition-colors border"
        :class="[
          passed && opt.letter === props.correct
            ? 'bg-green/10 border-green cursor-default'
            : passed
              ? 'border-transparent cursor-not-allowed opacity-60'
              : 'border-transparent cursor-pointer hover:bg-surface-2'
        ]"
      >
        <input
          v-model="selected"
          type="radio"
          :name="`quiz-${props.id}`"
          :value="opt.letter"
          class="mt-[5px] cursor-pointer accent-accent"
        />
        <span class="flex-1">
          <span
            class="font-mono mr-2"
            :class="
              passed && opt.letter === props.correct
                ? 'text-green'
                : 'text-text-mute'
            "
            >{{ opt.letter }})</span
          >
          <span
            :class="
              passed && opt.letter === props.correct
                ? 'text-green'
                : 'text-text-dim'
            "
            >{{ opt.label }}</span
          >
        </span>
        <span
          v-if="passed && opt.letter === props.correct"
          class="text-green font-mono text-[12.5px]"
          aria-label="正確答案"
        >
          ✓
        </span>
      </label>
    </fieldset>

    <div v-if="!contractError" class="mt-3 flex items-center gap-3">
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

<style scoped>
/* Hide the markdown-rendered option list (`- a) ... / - b) ...`)
   without removing it from the DOM — parseOptions walks the same
   slot ref to extract option letters + labels and renders them as
   radio buttons in the visible fieldset above. */
.quiz-body :deep(ul),
.quiz-body :deep(ol) {
  display: none;
}
.quiz-body :deep(p) {
  margin: 0 0 8px;
  line-height: 1.6;
  color: theme('colors.text.base');
}
.quiz-body :deep(code) {
  font-family: theme('fontFamily.mono');
  font-size: 13.5px;
  color: theme('colors.accent');
  padding: 1px 6px;
  background: theme('colors.surface.2');
  border-radius: 4px;
}
</style>
