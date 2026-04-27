<script setup lang="ts">
import { ref, computed } from 'vue'
import { useSidecar } from '~/composables/useSidecar'

interface HealthBody {
  status: string
}

const { fetch: sidecarFetch, ready, baseUrl } = useSidecar()

const pingState = ref<'idle' | 'pending' | 'ok' | 'error'>('idle')
const pingDetail = ref<string>('')

const port = computed<string>(() => {
  if (!baseUrl.value) {
    return ''
  }
  try {
    return new URL(baseUrl.value).port
  } catch {
    return ''
  }
})

async function onPing(): Promise<void> {
  pingState.value = 'pending'
  pingDetail.value = ''
  try {
    if (!ready.value) {
      throw new Error('Sidecar handshake not complete (Tauri IPC unavailable in this context)')
    }
    const res = await sidecarFetch('/healthz')
    if (!res.ok) {
      throw new Error(`/healthz returned HTTP ${res.status}`)
    }
    const body = (await res.json()) as HealthBody
    pingState.value = 'ok'
    pingDetail.value = `status=${body.status} port=${port.value}`
  } catch (err) {
    pingState.value = 'error'
    pingDetail.value = err instanceof Error ? err.message : String(err)
  }
}
</script>

<template>
  <section class="flex flex-col items-center justify-center gap-6 min-h-full px-6 py-16 bg-surface-0 text-text-base">
    <h1 class="text-4xl font-semibold tracking-tight">CodeBus</h1>
    <p class="text-sm text-text-dim">Phase 6 shell baseline · sidecar handshake smoke test</p>
    <button
      type="button"
      class="rounded-md bg-accent text-surface-0 px-4 py-2 text-sm font-medium transition hover:opacity-90 disabled:opacity-50"
      :disabled="pingState === 'pending'"
      @click="onPing"
    >
      {{ pingState === 'pending' ? 'Pinging…' : 'Ping sidecar' }}
    </button>
    <pre
      v-if="pingDetail"
      class="text-xs text-text-dim bg-surface-1 rounded px-3 py-2 max-w-xl whitespace-pre-wrap font-mono"
      :data-state="pingState"
    >{{ pingDetail }}</pre>
  </section>
</template>
