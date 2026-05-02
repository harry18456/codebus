// Backs SHALL clauses in
// openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//   Requirement: Entry page exposes folder-picker workspace onramp
//     Scenario: Scan terminal event chains kb-build automatically (UI assertion)
//     Scenario: kb-build terminal event unlocks generate CTA
//     Scenario: Generate terminal event renders enter-tutorial CTA
//     Scenario: SSE error pauses onramp with retry affordance

import { mount } from '@vue/test-utils'
import { describe, expect, it, vi, beforeEach } from 'vitest'
import { ref, type Ref } from 'vue'
import type { OnrampPhase } from '~/composables/useWorkspaceOnramp'
import type { SseEvent } from '~/composables/useSseTask'

interface MockOnramp {
  phase: Ref<OnrampPhase>
  workspaceId: Ref<string | null>
  pickedPath: Ref<string | null>
  progressEvents: Ref<SseEvent[]>
  errorMsg: Ref<string | null>
  errorCode: Ref<string | null>
  activeTaskId: Ref<string | null>
  start: ReturnType<typeof vi.fn>
  triggerGenerate: ReturnType<typeof vi.fn>
  retry: ReturnType<typeof vi.fn>
}

const onrampMock: MockOnramp = {
  phase: ref<OnrampPhase>('idle'),
  workspaceId: ref<string | null>(null),
  pickedPath: ref<string | null>(null),
  progressEvents: ref<SseEvent[]>([]),
  errorMsg: ref<string | null>(null),
  errorCode: ref<string | null>(null),
  activeTaskId: ref<string | null>(null),
  start: vi.fn(),
  triggerGenerate: vi.fn(),
  retry: vi.fn()
}

vi.mock('~/composables/useWorkspaceOnramp', async (importOriginal) => {
  const original = (await importOriginal()) as Record<string, unknown>
  return {
    ...original,
    useWorkspaceOnramp: () => onrampMock
  }
})

import WorkspaceOnrampCard from '~/components/workspace-onramp/WorkspaceOnrampCard.vue'

const stubs = {
  // Render the NuxtLink as a plain anchor so we can assert on `href`.
  NuxtLink: {
    props: ['to'],
    template: '<a :data-testid="$attrs[\'data-testid\']" :href="to"><slot /></a>',
    inheritAttrs: false
  }
}

beforeEach(() => {
  onrampMock.phase.value = 'idle'
  onrampMock.workspaceId.value = null
  onrampMock.pickedPath.value = null
  onrampMock.progressEvents.value = []
  onrampMock.errorMsg.value = null
  onrampMock.errorCode.value = null
  onrampMock.activeTaskId.value = null
  onrampMock.start.mockReset()
  onrampMock.triggerGenerate.mockReset()
  onrampMock.retry.mockReset()
})

describe('<WorkspaceOnrampCard>', () => {
  it('renders idle prompt when phase=idle', () => {
    const wrapper = mount(WorkspaceOnrampCard, { global: { stubs } })
    expect(wrapper.find('[data-testid="onramp-idle"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="onramp-generate-cta"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="onramp-enter-tutorial"]').exists()).toBe(false)
  })

  it('renders generate CTA when phase=scan-complete and clicking calls triggerGenerate', async () => {
    onrampMock.phase.value = 'scan-complete'
    onrampMock.workspaceId.value = 'ws_b3e6cc56defb'
    onrampMock.pickedPath.value = 'C:\\Users\\harry\\Code\\demo'
    const wrapper = mount(WorkspaceOnrampCard, { global: { stubs } })
    const cta = wrapper.find('[data-testid="onramp-generate-cta"]')
    expect(cta.exists()).toBe(true)
    await cta.trigger('click')
    expect(onrampMock.triggerGenerate).toHaveBeenCalledTimes(1)
  })

  it('renders enter-tutorial anchor with /tutorial/<workspaceId> when phase=ready', () => {
    onrampMock.phase.value = 'ready'
    onrampMock.workspaceId.value = 'ws_abc123def456'
    onrampMock.pickedPath.value = '/home/alice/projects/foo'
    const wrapper = mount(WorkspaceOnrampCard, { global: { stubs } })
    const link = wrapper.find('[data-testid="onramp-enter-tutorial"]')
    expect(link.exists()).toBe(true)
    expect(link.attributes('href')).toBe('/tutorial/ws_abc123def456')
  })

  it('renders errorMsg + retry button when phase=error', async () => {
    onrampMock.phase.value = 'error'
    onrampMock.workspaceId.value = 'ws_b3e6cc56defb'
    onrampMock.pickedPath.value = '/abs/path'
    onrampMock.errorMsg.value = 'oops scan failed'
    const wrapper = mount(WorkspaceOnrampCard, { global: { stubs } })
    expect(wrapper.text()).toContain('oops scan failed')
    const retry = wrapper.find('[data-testid="onramp-retry"]')
    expect(retry.exists()).toBe(true)
    await retry.trigger('click')
    expect(onrampMock.retry).toHaveBeenCalledTimes(1)
  })

  it('shows path tail + workspaceId in non-idle phases', () => {
    onrampMock.phase.value = 'scanning'
    onrampMock.workspaceId.value = 'ws_b3e6cc56defb'
    onrampMock.pickedPath.value = 'C:/Users/Harry/Code/demo'
    const wrapper = mount(WorkspaceOnrampCard, { global: { stubs } })
    expect(wrapper.find('[data-testid="onramp-path-tail"]').text()).toBe('demo')
    expect(wrapper.find('[data-testid="onramp-workspace-id"]').text()).toBe(
      'ws_b3e6cc56defb'
    )
  })

  it('renders <OnrampProgress> in all four in-flight phases', () => {
    for (const phase of ['scanning', 'indexing', 'exploring', 'generating'] as const) {
      onrampMock.phase.value = phase
      onrampMock.workspaceId.value = 'ws_b3e6cc56defb'
      onrampMock.pickedPath.value = '/abs/path'
      const wrapper = mount(WorkspaceOnrampCard, { global: { stubs } })
      expect(wrapper.find('[data-testid="onramp-progress"]').exists()).toBe(
        true,
        `OnrampProgress should render for phase=${phase}`
      )
      wrapper.unmount()
    }
  })
})
