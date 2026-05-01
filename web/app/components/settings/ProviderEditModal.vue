<script setup lang="ts">
// `<ProviderEditModal>` — add / edit a single provider entry.
//
// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Requirement: Provider pool CRUD touches keyring and config
//
// Confirm flow contract:
//   1. Validate provider_id regex `^[a-z][a-z0-9-]{2,40}$`
//   2. `keyring_set({ provider_id, api_key })` IPC first
//   3. On keyring success → `useProviderConfig().upsertProvider(...)`
//   4. On keyring failure → display error, do NOT call upsertProvider
//
// The api_key never crosses the sidecar wire — only the keyring
// IPC carries it. The upsertProvider POST body is a pure metadata
// shape (id / type / model / base_url).

import { computed, ref, watch } from 'vue'

import {
  type ProviderSpec,
  useProviderConfig
} from '~/composables/useProviderConfig'

interface Props {
  open: boolean
  initial?: ProviderSpec | null
}

const props = withDefaults(defineProps<Props>(), { initial: null })
const emit = defineEmits<{ close: []; saved: [provider: ProviderSpec] }>()

const PROVIDER_TYPES = [
  { value: 'openai_chat', label: 'Chat (OpenAI compatible)' },
  { value: 'openai_embedding', label: 'Embedding (OpenAI compatible)' }
] as const

const id = ref('')
const type = ref<string>('openai_chat')
const model = ref('')
const baseUrl = ref('https://api.openai.com/v1')
const apiKey = ref('')
const apiKeyVisible = ref(false)

const error = ref<string | null>(null)
const submitting = ref(false)

const ID_RE = /^[a-z][a-z0-9-]{2,40}$/

const idValid = computed(() => ID_RE.test(id.value))
const canSubmit = computed(
  () =>
    idValid.value &&
    !!model.value.trim() &&
    !!baseUrl.value.trim() &&
    !!apiKey.value &&
    !submitting.value
)

watch(
  () => props.initial,
  (next) => {
    if (next) {
      id.value = next.id
      type.value = next.type
      model.value = next.model
      baseUrl.value = next.base_url
    } else {
      id.value = ''
      type.value = 'openai_chat'
      model.value = ''
      baseUrl.value = 'https://api.openai.com/v1'
    }
    apiKey.value = ''
    apiKeyVisible.value = false
    error.value = null
  },
  { immediate: true }
)

async function callKeyringSet(
  providerId: string,
  key: string
): Promise<{ ok: boolean; code?: string }> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const res = await invoke<{ ok: boolean; code?: string }>('keyring_set', {
      providerId,
      apiKey: key
    })
    return res ?? { ok: true }
  } catch (e) {
    return { ok: false, code: 'KEYRING_BACKEND_ERROR' }
  }
}

async function onConfirm(): Promise<void> {
  if (!canSubmit.value) return
  submitting.value = true
  error.value = null
  try {
    const keyringResult = await callKeyringSet(id.value, apiKey.value)
    if (!keyringResult.ok) {
      error.value = keyringResult.code ?? 'KEYRING_ERROR'
      submitting.value = false
      return
    }
    const spec: ProviderSpec = {
      id: id.value,
      type: type.value,
      model: model.value,
      base_url: baseUrl.value
    }
    await useProviderConfig().upsertProvider(spec)
    emit('saved', spec)
    emit('close')
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <div
    v-if="open"
    data-testid="provider-edit-modal-dim"
    class="fixed inset-0 z-[60] grid place-items-center bg-surface-0/80"
    @click.self="emit('close')"
  >
    <aside
      data-testid="provider-edit-modal"
      class="max-w-[480px] w-full mx-4 p-6 rounded-lg bg-surface-1 border border-border-base shadow-xl"
      @click.stop
    >
      <h3 class="text-[16px] font-semibold text-text-base mb-4">
        {{ initial ? 'Edit provider' : 'Add provider' }}
      </h3>

      <label class="block text-[12px] uppercase tracking-wide text-text-mute mb-1">
        provider id
      </label>
      <input
        v-model="id"
        data-testid="provider-edit-id"
        :disabled="!!initial"
        placeholder="openai-default"
        class="w-full px-3 py-1.5 mb-3 rounded-md bg-surface-2 text-text-base text-[13px]"
      />

      <label class="block text-[12px] uppercase tracking-wide text-text-mute mb-1">
        type
      </label>
      <select
        v-model="type"
        data-testid="provider-edit-type"
        class="w-full px-3 py-1.5 mb-3 rounded-md bg-surface-2 text-text-base text-[13px]"
      >
        <option v-for="opt in PROVIDER_TYPES" :key="opt.value" :value="opt.value">
          {{ opt.label }}
        </option>
      </select>

      <label class="block text-[12px] uppercase tracking-wide text-text-mute mb-1">
        model
      </label>
      <input
        v-model="model"
        data-testid="provider-edit-model"
        placeholder="gpt-4o-mini"
        class="w-full px-3 py-1.5 mb-3 rounded-md bg-surface-2 text-text-base text-[13px]"
      />

      <label class="block text-[12px] uppercase tracking-wide text-text-mute mb-1">
        base_url
      </label>
      <input
        v-model="baseUrl"
        data-testid="provider-edit-base-url"
        class="w-full px-3 py-1.5 mb-3 rounded-md bg-surface-2 text-text-base text-[13px]"
      />

      <label class="block text-[12px] uppercase tracking-wide text-text-mute mb-1">
        api_key
      </label>
      <div class="flex gap-2 mb-3">
        <input
          v-model="apiKey"
          :type="apiKeyVisible ? 'text' : 'password'"
          data-testid="provider-edit-api-key"
          placeholder="sk-..."
          class="flex-1 px-3 py-1.5 rounded-md bg-surface-2 text-text-base text-[13px]"
        />
        <button
          type="button"
          data-testid="provider-edit-api-key-toggle"
          class="px-3 py-1.5 rounded-md text-[12px] bg-surface-2 text-text-dim hover:text-text-base"
          @click="apiKeyVisible = !apiKeyVisible"
        >
          {{ apiKeyVisible ? 'hide' : 'show' }}
        </button>
      </div>

      <p
        v-if="error"
        data-testid="provider-edit-error"
        class="text-[13px] text-rose-400 mb-3"
      >
        {{ error }}
      </p>

      <div class="flex justify-end gap-3">
        <button
          type="button"
          data-testid="provider-edit-cancel"
          class="px-3 py-1.5 rounded-md text-[13px] bg-surface-2 text-text-dim hover:bg-surface-3 hover:text-text-base"
          @click="emit('close')"
        >
          Cancel
        </button>
        <button
          type="button"
          data-testid="provider-edit-confirm"
          :disabled="!canSubmit"
          class="px-3 py-1.5 rounded-md text-[13px] bg-blue-500 text-white disabled:opacity-50 hover:bg-blue-600"
          @click="onConfirm"
        >
          Confirm
        </button>
      </div>
    </aside>
  </div>
</template>
