<script setup lang="ts">
type Tab = 'learn' | 'reasoning' | 'audit'
type KillState = 'READY' | 'ARMED' | 'OFF'

interface Props {
  workspace: string
  task?: string
  tab?: Tab
  kill: KillState
  model?: string
  tokens?: string
  cost?: string
}

withDefaults(defineProps<Props>(), {
  task: '',
  tab: undefined,
  model: '',
  tokens: '',
  cost: ''
})

defineEmits<{
  (e: 'switch-workspace'): void
  (e: 'select-tab', tab: Tab): void
  (e: 'open-settings'): void
  (e: 'kill'): void
}>()

const dash = '—'
</script>

<template>
  <div
    class="h-11 flex items-center px-4 gap-[14px] border-b border-border-base bg-surface-1 flex-shrink-0"
  >
    <div class="flex items-center gap-2 font-semibold tracking-tight">
      <div
        class="w-[22px] h-[22px] rounded-md grid place-items-center text-[12px] text-surface-0 bg-gradient-to-br from-accent to-accent-2"
      >
        🚌
      </div>
      <span class="text-[13.5px]">CodeBus</span>
    </div>

    <button
      type="button"
      class="flex items-center gap-1.5 px-2.5 py-1 rounded-md font-mono text-[11.5px] text-text-dim bg-surface-2 border border-border-base ml-1.5 hover:border-surface-4 hover:text-text-base"
      :title="`Switch workspace (current: ${workspace})`"
      @click="$emit('switch-workspace')"
    >
      <svg class="w-[1em] h-[1em] inline-block align-middle" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M2 4a1 1 0 0 1 1-1h3l1.5 1.5H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V4Z" />
      </svg>
      <span>{{ workspace }}</span>
      <span class="text-text-mute">▾</span>
    </button>

    <div v-if="tab" class="flex gap-0.5 ml-2">
      <button
        v-for="t in (['learn', 'reasoning', 'audit'] as const)"
        :key="t"
        type="button"
        class="px-3 py-[5px] rounded-md text-[12.5px]"
        :class="t === tab ? 'bg-surface-2 text-text-base' : 'text-text-mute hover:text-text-dim'"
        @click="$emit('select-tab', t)"
      >
        {{ t === 'learn' ? 'Learn' : t === 'reasoning' ? 'Reasoning' : 'Audit' }}
      </button>
    </div>

    <div class="flex-1" />

    <div
      v-if="task"
      class="font-mono text-[10.5px] text-text-mute pr-1.5 mr-1.5 border-r border-border-base"
    >
      task={{ task }}
    </div>

    <div class="flex items-center px-1 border border-border-base rounded-md bg-surface-2 h-7 overflow-hidden">
      <div
        class="px-2.5 font-mono text-[11px] text-text-dim flex items-center gap-1.5 h-full border-r border-border-base"
        title="LLM provider"
      >
        <span class="w-1.5 h-1.5 rounded-full bg-accent" />
        {{ model || dash }}
      </div>
      <div
        class="px-2.5 font-mono text-[11px] text-text-dim flex items-center gap-1.5 h-full border-r border-border-base"
        title="Tokens this session"
      >
        tokens <span class="text-text-base font-medium">{{ tokens || dash }}</span>
      </div>
      <div
        class="px-2.5 font-mono text-[11px] text-text-dim flex items-center gap-1.5 h-full"
        title="Cost"
      >
        cost <span class="text-text-base font-medium">{{ cost || dash }}</span>
      </div>
    </div>

    <button
      type="button"
      class="flex items-center px-2 py-[5px] rounded-md bg-surface-2 border border-border-base text-text-dim hover:border-surface-4 hover:text-text-base"
      title="Settings"
      @click="$emit('open-settings')"
    >
      <svg class="w-[1em] h-[1em] inline-block align-middle" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
        <circle cx="8" cy="8" r="2.5" />
        <path d="M8 1v2M8 13v2M15 8h-2M3 8H1M12.95 3.05l-1.41 1.41M4.46 11.54l-1.41 1.41M12.95 12.95l-1.41-1.41M4.46 4.46L3.05 3.05" />
      </svg>
    </button>

    <button
      type="button"
      class="flex items-center gap-1.5 px-2.5 py-1 rounded-md border border-border-base text-text-dim font-mono text-[11px] bg-surface-2 hover:border-red hover:text-red"
      title="Kill switch — instantly disable LLM calls"
      @click="$emit('kill')"
    >
      <span class="w-[7px] h-[7px] rounded-full bg-red" />
      <span>{{ kill }}</span>
    </button>
  </div>
</template>
