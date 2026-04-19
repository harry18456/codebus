<script setup lang="ts">
const pingState = ref<'idle' | 'pending' | 'ok' | 'error'>('idle')
const pingDetail = ref<string>('')

async function onPing() {
  pingState.value = 'pending'
  pingDetail.value = ''
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const res = await invoke<{ status: string; port: number }>('sidecar_ping')
    pingState.value = 'ok'
    pingDetail.value = `status=${res.status} port=${res.port}`
  } catch (err) {
    pingState.value = 'error'
    pingDetail.value = err instanceof Error ? err.message : String(err)
  }
}
</script>

<template>
  <main class="min-h-screen flex flex-col items-center justify-center gap-6 bg-slate-950 text-slate-100">
    <h1 class="text-4xl font-semibold tracking-tight">CodeBus</h1>
    <p class="text-sm text-slate-400">M1 power-on — sidecar handshake smoke test</p>
    <button
      class="rounded-md bg-indigo-500 hover:bg-indigo-400 px-4 py-2 text-sm font-medium transition"
      :disabled="pingState === 'pending'"
      @click="onPing"
    >
      {{ pingState === 'pending' ? 'Pinging…' : 'Ping sidecar' }}
    </button>
    <pre
      v-if="pingDetail"
      class="text-xs text-slate-300 bg-slate-900 rounded px-3 py-2 max-w-xl whitespace-pre-wrap"
      :data-state="pingState"
    >{{ pingDetail }}</pre>
  </main>
</template>
