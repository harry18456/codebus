<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, useTemplateRef, watch } from 'vue'

import { useTutorialProgress } from '~/composables/useTutorialProgress'

const props = defineProps<{
  id: string
}>()

const slotRef = useTemplateRef<HTMLDivElement>('slotRef')
const checkedCount = ref(0)
const total = ref(0)

const passed = computed(() => total.value > 0 && checkedCount.value === total.value)

const ID_RE = /^(station-\d+-check|s\d+-check-\d+)$/
const idValid = ID_RE.test(props.id)

// H3 + H4 fix: mdc renders the markdown slot asynchronously, so the
// initial onMounted often runs before any <input type=checkbox> exists.
// We watch the slot DOM with a MutationObserver, rebind on every
// change, and clean both observer + listeners on unmount.

interface BoundCheckbox {
  el: HTMLInputElement
  handler: (event: Event) => void
}

let observer: MutationObserver | null = null
let bound: BoundCheckbox[] = []
let warnedEmpty = false

function unbindAll(): void {
  for (const { el, handler } of bound) {
    el.removeEventListener('change', handler)
  }
  bound = []
}

function rebind(): void {
  const root = slotRef.value
  if (!root) return
  const live = Array.from(root.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'))

  // Drop listeners on detached or replaced elements.
  unbindAll()

  // Spec scenario "Already-completed station revisitable via URL paste"
  // implies review-mode visual restore: when progress.checkpoints[id]
  // says done=true (the user passed previously), tick every box on
  // mount so the user sees their completed state instead of being
  // asked to re-tick. This stays a one-way restore: the change
  // listeners fire on user interaction the same way as before.
  const progress = useTutorialProgress()
  const restored = progress.state.value.checkpoints[props.id]?.done === true

  for (const el of live) {
    el.disabled = false
    if (restored) el.checked = true
    const handler = (): void => {
      recompute()
      const progress2 = useTutorialProgress()
      progress2.setCheckpoint(props.id, indexOfCheckbox(el), passed.value)
    }
    el.addEventListener('change', handler)
    bound.push({ el, handler })
  }

  total.value = live.length
  recompute()

  if (live.length === 0 && !warnedEmpty) {
    warnedEmpty = true
    // eslint-disable-next-line no-console
    console.warn(`<Checkpoint id="${props.id}"> has no checkbox items yet`)
  } else if (live.length > 0) {
    warnedEmpty = false
  }
}

function indexOfCheckbox(el: HTMLInputElement): number {
  return bound.findIndex((b) => b.el === el)
}

function recompute(): void {
  total.value = bound.length
  checkedCount.value = bound.filter((b) => b.el.checked).length
}

onMounted(() => {
  if (!idValid) {
    // eslint-disable-next-line no-console
    console.warn(
      `<Checkpoint id="${props.id}"> does not match /^(station-\\d+-check|s\\d+-check-\\d+)$/ — rendering anyway`
    )
  }
  rebind()
  if (slotRef.value) {
    observer = new MutationObserver(() => rebind())
    observer.observe(slotRef.value, { childList: true, subtree: true })
  }
})

onBeforeUnmount(() => {
  if (observer) {
    observer.disconnect()
    observer = null
  }
  unbindAll()
})

watch(
  () => props.id,
  () => {
    warnedEmpty = false
    rebind()
  }
)
</script>

<template>
  <div class="my-6 p-4 rounded-lg bg-surface-1 border border-border-soft">
    <div class="flex items-center justify-between mb-2 font-mono text-[10.5px]">
      <span class="tracking-[0.14em] uppercase text-text-mute">Checkpoint</span>
      <span
        v-if="passed"
        data-testid="checkpoint-passed"
        class="px-2 py-[2px] rounded-full text-[11px] bg-green/20 text-green"
      >
        ✓ 通過
      </span>
    </div>
    <div ref="slotRef" class="checkpoint-body text-[14px]">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.checkpoint-body :deep(ul) {
  list-style: none;
  padding-left: 0;
  margin: 0;
}
.checkpoint-body :deep(li) {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 6px 0;
  color: theme('colors.text.dim');
  line-height: 1.55;
}
.checkpoint-body :deep(input[type='checkbox']) {
  margin-top: 5px;
  cursor: pointer;
  accent-color: theme('colors.accent');
}
</style>
