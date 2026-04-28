<script setup lang="ts">
import { computed, onMounted, ref, useTemplateRef, watch } from 'vue'

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

function findCheckboxes(): HTMLInputElement[] {
  const el = slotRef.value
  if (!el) return []
  return Array.from(el.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'))
}

function recompute(): void {
  const boxes = findCheckboxes()
  total.value = boxes.length
  checkedCount.value = boxes.filter((b) => b.checked).length
}

function handleChange(_event: Event, index: number): void {
  recompute()
  const progress = useTutorialProgress()
  // Spec: progress.checkpoints[id] is one { done, ts } record for the
  // Checkpoint as a whole. Flip done=true once all items pass; flip back
  // when the user later unchecks below the threshold.
  progress.setCheckpoint(props.id, index, passed.value)
}

onMounted(() => {
  if (!idValid) {
    // eslint-disable-next-line no-console
    console.warn(
      `<Checkpoint id="${props.id}"> does not match /^(station-\\d+-check|s\\d+-check-\\d+)$/ — rendering anyway`
    )
  }
  const boxes = findCheckboxes()
  total.value = boxes.length
  if (boxes.length === 0) {
    // eslint-disable-next-line no-console
    console.warn(`<Checkpoint id="${props.id}"> has no checkbox items`)
    return
  }
  boxes.forEach((box, index) => {
    box.disabled = false
    box.addEventListener('change', (e) => handleChange(e, index))
  })
  recompute()
})

watch(
  () => props.id,
  () => {
    recompute()
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
