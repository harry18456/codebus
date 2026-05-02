<script setup lang="ts">
// `<FolderPickerButton>` — entry-page CTA that invokes the Tauri
// dialog plugin's native folder picker.
//
// Backs SHALL clauses in
// openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//   Requirement: Folder picker invocation flow
//     Scenario: User cancels folder picker
//
// User cancellation (the OS picker returns `null` / `undefined`) leaves
// the onramp state unchanged — the parent `pages/index.vue` only
// reacts to the `picked` event, so a no-op suppression here is enough.

const emit = defineEmits<{
  (event: 'picked', absolutePath: string): void
}>()

async function onClick(): Promise<void> {
  // Dynamic import keeps `@tauri-apps/plugin-dialog` out of the
  // initial chunk and lets the test environment swap in a mock without
  // pulling the real Tauri runtime.
  const mod = await import('@tauri-apps/plugin-dialog')
  const result = await mod.open({ directory: true, multiple: false })
  if (typeof result === 'string' && result.length > 0) {
    emit('picked', result)
  }
}
</script>

<template>
  <button
    type="button"
    data-testid="onramp-folder-picker"
    class="rounded-md bg-accent text-surface-0 px-4 py-2 text-sm font-medium transition hover:opacity-90"
    @click="onClick"
  >
    + 開新 codebase
  </button>
</template>
