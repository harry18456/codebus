<script setup lang="ts">
// Three-pane workspace shell: topbar (44px) on top, main stage on the left,
// always-on audit rail on the right (360px). Mirrors the `.cb-app` /
// `.cb-split` / `.cb-stage` / `.cb-audit` primitives in
// `design/v1/shell.css`. Pages override the named slots; the fallback
// content keeps the shell coherent for pages that only render a stage.
//
// Components imported explicitly to bypass an auto-import miss observed
// during `cargo tauri dev` — without these the topbar slot fallback
// renders a "Failed to resolve component: TopBar" Vue warn and the
// 44px row stays blank.
import { onBeforeUnmount, onMounted } from 'vue'
import TopBar from '~/components/layout/TopBar.vue'
import AuditPanel from '~/components/audit/AuditPanel.vue'
import QAOverlay from '~/components/qa/QAOverlay.vue'
import { useQaSession } from '~/composables/useQaSession'

// `qa-overlay-p0`: layout owns the global keyboard shortcut for the Q&A
// drawer. Spec rationale — listener MUST live on `window` in the layout
// host so Cmd+K can OPEN the drawer (drawer-internal listener could not
// fire when the drawer is closed because the component is `v-if`-gated).
const qaSession = useQaSession()

function handleKeyDown(e: KeyboardEvent): void {
  if (e.key === 'Escape' && qaSession.open.value) {
    e.preventDefault()
    qaSession.close()
    return
  }
  if ((e.metaKey || e.ctrlKey) && e.key === 'k' && !qaSession.open.value) {
    e.preventDefault()
    qaSession.openDrawer()
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeyDown)
})
</script>

<template>
  <div class="grid grid-rows-[44px_1fr] h-screen overflow-hidden bg-surface-0 text-text-base font-sans">
    <slot name="topbar">
      <TopBar workspace="codebus" kill="READY" />
    </slot>
    <div class="grid grid-cols-[minmax(480px,1fr)_360px] overflow-hidden min-h-0">
      <main class="overflow-y-auto min-h-0 bg-surface-0">
        <slot />
      </main>
      <aside class="border-l border-border-base flex flex-col bg-surface-1 min-h-0">
        <slot name="audit">
          <AuditPanel active-tab="sanitize" />
        </slot>
      </aside>
    </div>
    <QAOverlay />
  </div>
</template>
