// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/sidecar-runtime/spec.md
//   Requirement: Sidecar accepts provider config mutation endpoints (consumer)
// And openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Requirement: Provider pool CRUD touches keyring and config
//   Requirement: Role binding change propagates via hot-swap
//
// `useProviderConfig` is a module-level singleton (same convention as
// `useQaSession` / `useIntervention`) that proxies provider pool /
// role bindings / PII mode mutations to the sidecar settings endpoints.

import { describe, expect, it, vi, beforeEach } from 'vitest'
import { ref } from 'vue'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

const fetchMock = vi.fn()
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: (...args: unknown[]) => fetchMock(...args)
  })
}))

import {
  useProviderConfig,
  _resetForTest
} from '~/composables/useProviderConfig'

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' }
  })
}

function noBodyResponse(status = 204): Response {
  return new Response('', { status })
}

beforeEach(() => {
  fetchMock.mockReset()
  _resetForTest()
})

const SAMPLE_SNAPSHOT = {
  providers: [
    {
      id: 'openai-default',
      type: 'openai_chat',
      model: 'gpt-4o-mini',
      base_url: 'https://api.openai.com/v1'
    },
    {
      id: 'openai-embed-3',
      type: 'openai_embedding',
      model: 'text-embedding-3-small',
      base_url: 'https://api.openai.com/v1'
    }
  ],
  bindings: {
    reasoning: 'openai-default',
    judge: 'openai-default',
    chat: 'openai-default',
    embed: 'openai-embed-3'
  },
  pii_mode: 'rule',
  pii_provider_id: null
}

describe('useProviderConfig — module-level singleton', () => {
  it('two callers receive the same singleton refs (Object.is)', () => {
    const a = useProviderConfig()
    const b = useProviderConfig()
    expect(Object.is(a.providers, b.providers)).toBe(true)
    expect(Object.is(a.bindings, b.bindings)).toBe(true)
    expect(Object.is(a.piiMode, b.piiMode)).toBe(true)
  })

  it('loadConfig() GET /settings/providers and hydrates state', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(SAMPLE_SNAPSHOT))
    const api = useProviderConfig()
    await api.loadConfig()

    expect(fetchMock).toHaveBeenCalledTimes(1)
    const call = fetchMock.mock.calls[0]!
    expect(call[0]).toBe('/settings/providers')
    expect(api.providers.value).toHaveLength(2)
    expect(api.bindings.value.embed).toBe('openai-embed-3')
    expect(api.piiMode.value).toBe('rule')
  })

  it('upsertProvider POSTs without api_key', async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse(SAMPLE_SNAPSHOT))
      .mockResolvedValueOnce(noBodyResponse())
    const api = useProviderConfig()
    await api.loadConfig()
    fetchMock.mockClear()

    fetchMock.mockResolvedValueOnce(noBodyResponse())
    await api.upsertProvider({
      id: 'anthropic-claude',
      type: 'openai_chat',
      model: 'claude-haiku',
      base_url: 'https://api.anthropic.com/v1'
    })

    const call = fetchMock.mock.calls[0]!
    expect(call[0]).toBe('/settings/providers')
    const init = call[1] as RequestInit
    expect(init.method).toBe('POST')
    const body = JSON.parse(init.body as string)
    expect(body).toMatchObject({
      id: 'anthropic-claude',
      type: 'openai_chat',
      model: 'claude-haiku',
      base_url: 'https://api.anthropic.com/v1'
    })
    expect('api_key' in body).toBe(false)
  })

  it('deleteProvider DELETE /settings/providers/{id}', async () => {
    const api = useProviderConfig()
    fetchMock.mockResolvedValueOnce(noBodyResponse())
    await api.deleteProvider('openai-default')
    const call = fetchMock.mock.calls[0]!
    expect(call[0]).toBe('/settings/providers/openai-default')
    expect((call[1] as RequestInit).method).toBe('DELETE')
  })

  it('setBinding PUT /settings/bindings with single role payload', async () => {
    const api = useProviderConfig()
    fetchMock.mockResolvedValueOnce(noBodyResponse())
    await api.setBinding('chat', 'anthropic-claude')
    const call = fetchMock.mock.calls[0]!
    expect(call[0]).toBe('/settings/bindings')
    const init = call[1] as RequestInit
    expect(init.method).toBe('PUT')
    const body = JSON.parse(init.body as string)
    expect(body).toEqual({ chat: 'anthropic-claude' })
  })

  it('setPiiMode PUT /settings/pii-mode', async () => {
    const api = useProviderConfig()
    fetchMock.mockResolvedValueOnce(noBodyResponse())
    await api.setPiiMode('rule')
    const call = fetchMock.mock.calls[0]!
    expect(call[0]).toBe('/settings/pii-mode')
    const init = call[1] as RequestInit
    expect(init.method).toBe('PUT')
    const body = JSON.parse(init.body as string)
    expect(body).toEqual({ mode: 'rule', provider_id: null })
  })
})

describe('useProviderConfig — SSE-driven re-fetch', () => {
  it('attached event stream triggers debounced GET /settings/providers on provider_config_changed', async () => {
    const events = ref<{ type: string; data: unknown }[]>([])
    const api = useProviderConfig()
    api.attachEventStream(events)

    // First call: initial loadConfig is the test's responsibility — the
    // composable does not auto-fetch on attach. The SSE-driven re-fetch
    // is what we verify here.
    fetchMock.mockResolvedValueOnce(jsonResponse(SAMPLE_SNAPSHOT))

    events.value = [...events.value, { type: 'provider_config_changed', data: {} }]

    // Debounce window is 100 ms; allow 200 ms slack for the timer.
    await new Promise((r) => setTimeout(r, 200))

    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(fetchMock.mock.calls[0]![0]).toBe('/settings/providers')
  })

  it('multiple events within debounce window collapse to a single re-fetch', async () => {
    const events = ref<{ type: string; data: unknown }[]>([])
    const api = useProviderConfig()
    api.attachEventStream(events)

    fetchMock.mockResolvedValue(jsonResponse(SAMPLE_SNAPSHOT))

    events.value = [
      ...events.value,
      { type: 'provider_config_changed', data: {} }
    ]
    await new Promise((r) => setTimeout(r, 30))
    events.value = [
      ...events.value,
      { type: 'provider_config_changed', data: {} }
    ]
    await new Promise((r) => setTimeout(r, 30))
    events.value = [
      ...events.value,
      { type: 'provider_config_changed', data: {} }
    ]

    await new Promise((r) => setTimeout(r, 200))

    expect(fetchMock).toHaveBeenCalledTimes(1)
  })

  it('non-matching event types do not trigger a re-fetch', async () => {
    const events = ref<{ type: string; data: unknown }[]>([])
    const api = useProviderConfig()
    api.attachEventStream(events)

    events.value = [...events.value, { type: 'agent_thought', data: {} }]
    events.value = [...events.value, { type: 'usage_delta', data: {} }]

    await new Promise((r) => setTimeout(r, 200))
    expect(fetchMock).not.toHaveBeenCalled()
  })
})

describe('useProviderConfig — defensive source grep', () => {
  it('source code contains no `api_key` references', () => {
    const path = join(
      __dirname,
      '..',
      '..',
      'app',
      'composables',
      'useProviderConfig.ts'
    )
    const text = readFileSync(path, 'utf-8')
    // Allow comments containing the phrase as long as no string literal
    // or property name `api_key` appears in code.
    const stripped = text.replace(/\/\/.*$/gm, '').replace(/\/\*[\s\S]*?\*\//g, '')
    expect(stripped).not.toMatch(/['"]api_key['"]/)
    expect(stripped).not.toMatch(/\bapi_key\s*:/)
  })
})
