<script setup lang="ts">
// `<PiiModeToggle>` — third section of `/settings`. Two radio
// options: `rule` (default, no extra config) and `llm` (disabled in
// P0 because no LLM PII provider is registered yet).
//
// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Requirement: Settings page renders three sections (PII section)

import { useProviderConfig } from '~/composables/useProviderConfig'

const config = useProviderConfig()

async function onSelect(mode: 'rule' | 'llm'): Promise<void> {
  if (mode === 'llm') return // disabled in P0
  if (config.piiMode.value === mode) return
  await config.setPiiMode('rule', null)
}
</script>

<template>
  <section
    data-section="pii-mode"
    data-testid="pii-mode-section"
    class="p-4 rounded-lg bg-surface-1 border border-border-base"
  >
    <h2 class="text-[14px] font-semibold text-text-base mb-3">PII 偵測模式</h2>
    <p class="text-[12.5px] text-text-mute mb-3">
      決定三段 Sanitizer（Pass 1 / 2 / 3）使用哪種 PII 偵測後端。Rule
      模式是預設值，無需任何 LLM 即可運作；LLM 模式預計於後續版本支援。
    </p>
    <div class="flex flex-col gap-2">
      <label class="flex items-center gap-2 text-[13px] text-text-base">
        <input
          type="radio"
          name="pii-mode"
          value="rule"
          data-testid="pii-mode-rule"
          :checked="config.piiMode.value === 'rule'"
          @change="onSelect('rule')"
        />
        rule（預設）
      </label>
      <label
        class="flex items-center gap-2 text-[13px] text-text-mute opacity-50 cursor-not-allowed"
        style="pointer-events: none"
      >
        <input
          type="radio"
          name="pii-mode"
          value="llm"
          data-testid="pii-mode-llm"
          disabled
          :checked="config.piiMode.value === 'llm'"
        />
        llm（P1+ 規劃中）
      </label>
    </div>
  </section>
</template>
