<script setup lang="ts">
// `/onboarding/providers` — second step. Two side-by-side forms (chat
// + embedding). Submit fans out to keyring_set × 2 → upsertProvider
// × 2 → setBinding × 4 in strict order.
//
// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-onboarding/spec.md
//   Requirement: Onboarding wizard exposes three sequential routes
//   Requirement: Onboarding writes through keyring and provider config in correct order

import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import { useProviderConfig } from '~/composables/useProviderConfig'
import { useSidecar } from '~/composables/useSidecar'
import { openExternal } from '~/utils/external-link'
import { getTosUrl, type KnownProviderType } from '~/utils/provider-tos'

definePageMeta({ layout: false })

const router = useRouter()
const config = useProviderConfig()
const sidecar = useSidecar()

const ID_RE = /^[a-z][a-z0-9-]{2,40}$/

interface FormState {
  id: string
  type: KnownProviderType
  model: string
  base_url: string
  api_key: string
}

const chat = ref<FormState>({
  id: 'openai-default',
  type: 'openai_chat',
  model: 'gpt-4o-mini',
  base_url: 'https://api.openai.com/v1',
  api_key: ''
})
const embed = ref<FormState>({
  id: 'openai-embed-3',
  type: 'openai_embedding',
  model: 'text-embedding-3-small',
  base_url: 'https://api.openai.com/v1',
  api_key: ''
})

const chatTosUrl = computed(() => getTosUrl(chat.value.type))
const embedTosUrl = computed(() => getTosUrl(embed.value.type))

const submitting = ref(false)
const error = ref<string | null>(null)

function isValid(form: FormState): boolean {
  return (
    ID_RE.test(form.id) &&
    !!form.model.trim() &&
    !!form.base_url.trim() &&
    !!form.api_key
  )
}

const canSubmit = computed(
  () => isValid(chat.value) && isValid(embed.value) && !submitting.value
)

async function callKeyringSet(
  providerId: string,
  apiKey: string
): Promise<{ ok: boolean; code?: string }> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const res = await invoke<{ ok: boolean; code?: string }>('keyring_set', {
      providerId,
      apiKey
    })
    return res ?? { ok: true }
  } catch {
    return { ok: false, code: 'KEYRING_BACKEND_ERROR' }
  }
}

async function onNext(): Promise<void> {
  if (!canSubmit.value) return
  submitting.value = true
  error.value = null
  try {
    const r1 = await callKeyringSet(chat.value.id, chat.value.api_key)
    if (!r1.ok) {
      error.value = `Chat provider 寫入 keyring 失敗：${r1.code ?? 'unknown'}`
      return
    }
    const r2 = await callKeyringSet(embed.value.id, embed.value.api_key)
    if (!r2.ok) {
      error.value = `Embedding provider 寫入 keyring 失敗：${r2.code ?? 'unknown'}`
      return
    }
    const chatSpec = {
      id: chat.value.id,
      type: chat.value.type,
      model: chat.value.model,
      base_url: chat.value.base_url
    }
    const embedSpec = {
      id: embed.value.id,
      type: embed.value.type,
      model: embed.value.model,
      base_url: embed.value.base_url
    }
    await config.upsertProvider(chatSpec)
    await config.upsertProvider(embedSpec)
    await config.setBinding('reasoning', chat.value.id)
    await config.setBinding('judge', chat.value.id)
    await config.setBinding('chat', chat.value.id)
    await config.setBinding('embed', embed.value.id)
    // Push the just-written keyring entries to sidecar memory so
    // `/healthz.dependency.llm_chat` flips to `ready` before we let the
    // user advance to /onboarding/done. Failure MUST stop the flow —
    // otherwise the user sees "一切就緒" but the entry page redirects
    // them back to /onboarding/welcome, producing a confusing loop.
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('push_startup_config_cmd', {
        providerIds: [chat.value.id, embed.value.id]
      })
    } catch (e) {
      error.value = `推送 keys 到 sidecar 失敗：${
        e instanceof Error ? e.message : String(e)
      }`
      return
    }
    // Verify both LLM lanes report `ready` before routing — the done
    // page is meant to be a real success confirmation, not a ritual.
    try {
      const r = await sidecar.fetch('/healthz')
      if (!r.ok) {
        error.value = `Sidecar healthz 回 ${r.status}，請確認 sidecar 已啟動`
        return
      }
      const body = (await r.json()) as {
        dependency?: { llm_chat?: string; llm_embed?: string }
      }
      const chatLane = body.dependency?.llm_chat
      const embedLane = body.dependency?.llm_embed
      if (chatLane !== 'ready' || embedLane !== 'ready') {
        error.value =
          `Sidecar 仍未就緒：llm_chat=${chatLane ?? 'unknown'}, ` +
          `llm_embed=${embedLane ?? 'unknown'}。請確認 API key 正確並再試一次。`
        return
      }
    } catch (e) {
      error.value = `無法驗證 sidecar 狀態：${
        e instanceof Error ? e.message : String(e)
      }`
      return
    }
    router.push('/onboarding/done')
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <main
    data-testid="onboarding-providers"
    class="flex flex-col items-center min-h-screen py-8 px-8"
  >
    <div class="max-w-[920px] w-full flex flex-col gap-6">
      <h1 class="text-[22px] font-semibold text-white text-center">
        設定 LLM 提供者
      </h1>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <fieldset
          data-testid="onboarding-form-chat"
          class="p-4 rounded-lg bg-surface-1 border border-border-base"
        >
          <legend class="px-2 text-[14px] font-semibold text-text-base bg-surface-1">
            Chat（對話模型）
          </legend>
          <input
            v-model="chat.id"
            data-testid="onboarding-chat-id"
            placeholder="provider id"
            class="w-full px-3 py-1.5 my-2 rounded-md bg-surface-2 text-text-base text-[13px]"
          />
          <input
            v-model="chat.model"
            data-testid="onboarding-chat-model"
            placeholder="model"
            class="w-full px-3 py-1.5 my-2 rounded-md bg-surface-2 text-text-base text-[13px]"
          />
          <input
            v-model="chat.base_url"
            data-testid="onboarding-chat-base-url"
            class="w-full px-3 py-1.5 my-2 rounded-md bg-surface-2 text-text-base text-[13px]"
          />
          <input
            v-model="chat.api_key"
            type="password"
            data-testid="onboarding-chat-api-key"
            placeholder="api key"
            class="w-full px-3 py-1.5 my-2 rounded-md bg-surface-2 text-text-base text-[13px]"
          />
          <p v-if="chatTosUrl" class="text-[11.5px] text-text-mute mt-1">
            送出前請先閱讀
            <a
              :href="chatTosUrl"
              data-testid="onboarding-chat-tos-link"
              class="underline cursor-pointer"
              @click.prevent="openExternal(chatTosUrl)"
              >此 provider 的服務條款</a
            >。
          </p>
        </fieldset>
        <fieldset
          data-testid="onboarding-form-embed"
          class="p-4 rounded-lg bg-surface-1 border border-border-base"
        >
          <legend class="px-2 text-[14px] font-semibold text-text-base bg-surface-1">
            Embedding（向量模型）
          </legend>
          <input
            v-model="embed.id"
            data-testid="onboarding-embed-id"
            class="w-full px-3 py-1.5 my-2 rounded-md bg-surface-2 text-text-base text-[13px]"
          />
          <input
            v-model="embed.model"
            data-testid="onboarding-embed-model"
            class="w-full px-3 py-1.5 my-2 rounded-md bg-surface-2 text-text-base text-[13px]"
          />
          <input
            v-model="embed.base_url"
            data-testid="onboarding-embed-base-url"
            class="w-full px-3 py-1.5 my-2 rounded-md bg-surface-2 text-text-base text-[13px]"
          />
          <input
            v-model="embed.api_key"
            type="password"
            data-testid="onboarding-embed-api-key"
            placeholder="api key"
            class="w-full px-3 py-1.5 my-2 rounded-md bg-surface-2 text-text-base text-[13px]"
          />
          <p v-if="embedTosUrl" class="text-[11.5px] text-text-mute mt-1">
            送出前請先閱讀
            <a
              :href="embedTosUrl"
              data-testid="onboarding-embed-tos-link"
              class="underline cursor-pointer"
              @click.prevent="openExternal(embedTosUrl)"
              >此 provider 的服務條款</a
            >。
          </p>
        </fieldset>
      </div>
      <p
        v-if="error"
        data-testid="onboarding-providers-error"
        class="text-[13px] text-rose-400 text-center"
      >
        {{ error }}
      </p>
      <button
        type="button"
        data-testid="onboarding-providers-next"
        :disabled="!canSubmit"
        class="self-center px-4 py-2 rounded-md bg-blue-500 text-white text-[14px] disabled:opacity-50"
        @click="onNext"
      >
        下一步
      </button>
    </div>
  </main>
</template>
