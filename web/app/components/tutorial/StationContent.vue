<script setup lang="ts">
// Station markdown body renderer with D-T12 sub-page navigation.
// Splits the markdown on `^### ` headings into chunks; only one chunk
// renders at a time. ArrowDown / PageDown advances; ArrowUp / PageUp
// retreats. URLs stay station-level — chunk index is local component
// state. When the page changes station (markdown prop changes), index
// resets to 0.

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps<{
  markdown: string
}>()

const chunks = computed<string[]>(() => splitChunks(props.markdown))
const chunkIndex = ref(0)
const total = computed(() => chunks.value.length)
const currentChunk = computed(() => chunks.value[chunkIndex.value] ?? '')

function splitChunks(md: string): string[] {
  if (!md) return ['']
  const parts: string[] = []
  const lines = md.split('\n')
  let buffer: string[] = []
  let inFence = false
  let fenceMarker = ''
  for (const line of lines) {
    // H5 fix: track ``` / ~~~ fence state so '### ' inside a code
    // block does NOT open a new chunk (would corrupt the fenced
    // block).
    const fenceMatch = line.match(/^\s{0,3}(`{3,}|~{3,})/)
    if (fenceMatch) {
      const marker = fenceMatch[1]!
      if (!inFence) {
        inFence = true
        fenceMarker = marker
      } else if (line.trimStart().startsWith(fenceMarker)) {
        inFence = false
        fenceMarker = ''
      }
    }
    if (!inFence && /^###\s+/.test(line) && buffer.length > 0) {
      parts.push(buffer.join('\n'))
      buffer = [line]
    } else {
      buffer.push(line)
    }
  }
  if (buffer.length > 0) parts.push(buffer.join('\n'))
  // Drop empty/whitespace-only chunks. The common cause is markdown
  // that starts with a `### ` heading (after frontmatter strip): the
  // pre-heading buffer accumulates as an empty chunk 0 and shows up
  // as a blank "第 1 / N 頁" before any real content.
  const cleaned = parts.filter((p) => p.trim() !== '')
  return cleaned.length === 0 ? [''] : cleaned
}

function advance(): void {
  if (chunkIndex.value < total.value - 1) chunkIndex.value += 1
}

function retreat(): void {
  if (chunkIndex.value > 0) chunkIndex.value -= 1
}

function isFocusInEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  const tag = target.tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true
  if (target.isContentEditable) return true
  return false
}

function handleKeydown(event: KeyboardEvent): void {
  // Spec: do not preventDefault when focus is on an editable control;
  // the inner control should still receive its own arrow handling.
  if (isFocusInEditable(event.target)) return
  if (event.key === 'ArrowDown' || event.key === 'PageDown') {
    advance()
    event.preventDefault()
  } else if (event.key === 'ArrowUp' || event.key === 'PageUp') {
    retreat()
    event.preventDefault()
  }
}

onMounted(() => {
  if (typeof window === 'undefined') return
  window.addEventListener('keydown', handleKeydown)
})

onBeforeUnmount(() => {
  if (typeof window === 'undefined') return
  window.removeEventListener('keydown', handleKeydown)
})

// Spec: cross-station navigation resets chunk index. Watching markdown
// prop change covers the both-pages-mounted-via-keepalive case as well.
watch(
  () => props.markdown,
  () => {
    chunkIndex.value = 0
  }
)
</script>

<template>
  <section class="station-content">
    <div class="station-content-body">
      <MDC :value="currentChunk" />
    </div>
    <footer
      class="mt-8 pt-4 border-t border-border-soft flex items-center justify-between font-mono text-[10.5px] text-text-mute"
    >
      <button
        type="button"
        class="px-3 py-1.5 rounded-md bg-surface-2 text-text-dim hover:bg-surface-3 disabled:opacity-40 disabled:cursor-not-allowed"
        :disabled="chunkIndex === 0"
        @click="retreat"
      >
        ← 上一頁
      </button>
      <span data-testid="chunk-indicator">第 {{ chunkIndex + 1 }} / {{ total }} 頁</span>
      <button
        type="button"
        class="px-3 py-1.5 rounded-md bg-surface-2 text-text-dim hover:bg-surface-3 disabled:opacity-40 disabled:cursor-not-allowed"
        :disabled="chunkIndex >= total - 1"
        @click="advance"
      >
        下一頁 →
      </button>
    </footer>
  </section>
</template>

<style scoped>
.station-content-body :deep(h2) {
  font-size: 22px;
  font-weight: 600;
  letter-spacing: -0.015em;
  margin: 32px 0 12px;
  color: theme('colors.text.base');
}
.station-content-body :deep(h3) {
  font-size: 17px;
  font-weight: 600;
  margin: 24px 0 10px;
  color: theme('colors.text.base');
}
.station-content-body :deep(p) {
  margin: 0 0 16px;
  color: theme('colors.text.dim');
  line-height: 1.75;
}
.station-content-body :deep(code) {
  font-family: theme('fontFamily.mono');
  font-size: 13.5px;
  color: theme('colors.accent');
  padding: 1px 6px;
  background: theme('colors.surface.2');
  border-radius: 4px;
}
.station-content-body :deep(pre) {
  margin: 0 0 20px;
  padding: 14px 18px;
  font-family: theme('fontFamily.mono');
  font-size: 12.5px;
  line-height: 1.7;
  background: theme('colors.surface.1');
  border: 1px solid theme('colors.border.base');
  border-radius: 8px;
  overflow-x: auto;
}
.station-content-body :deep(pre code) {
  background: none;
  padding: 0;
  color: theme('colors.text.base');
}
.station-content-body :deep(ul) {
  padding-left: 22px;
  color: theme('colors.text.dim');
  margin: 0 0 16px;
}
.station-content-body :deep(ul li) {
  margin-bottom: 6px;
}
.station-content-body :deep(blockquote) {
  margin: 0 0 16px;
  padding: 12px 18px;
  border-left: 3px solid theme('colors.accent');
  color: theme('colors.text.dim');
  font-size: 14px;
  border-radius: 0 6px 6px 0;
}
.station-content-body :deep(strong) {
  color: theme('colors.text.base');
  font-weight: 600;
}
.station-content-body :deep(a) {
  color: theme('colors.accent');
  text-decoration: underline;
  text-underline-offset: 2px;
}
</style>
