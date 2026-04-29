import { describe, expect, it, vi, beforeEach } from 'vitest'
import { ref, nextTick, type Ref } from 'vue'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

// Single shared reactive ref backing the mocked useAuditJsonl output. Tests
// drive the composable by mutating `mockEntriesRef.value` after importing
// the module under test.
const mockEntriesRef: Ref<unknown[]> = ref([])

vi.mock('~/composables/useAuditJsonl', () => ({
  useAuditJsonl: vi.fn(() => ({
    entries: mockEntriesRef,
    loading: ref(false),
    error: ref(null),
    reload: vi.fn()
  }))
}))

beforeEach(() => {
  mockEntriesRef.value = []
})

async function importComposable() {
  return await import('~/composables/useSanitizeAudit')
}

describe('useSanitizeAudit', () => {
  it('parses source dict form into human-readable view', async () => {
    mockEntriesRef.value = [
      {
        ts: '2026-04-29T08:30:00.100Z',
        schema_version: 1,
        rules_version: '2026-04-20-1',
        pass: 1,
        session_id: 'sess_a',
        source: { pass: 'scanner', path: 'src/auth.ts' },
        rule_id: 'pii_email_v1',
        kind: 'email',
        placeholder_index: 1,
        extra: {}
      }
    ]
    const { useSanitizeAudit } = await importComposable()
    const api = useSanitizeAudit('/abs/ws')
    await nextTick()
    expect(api.rowViews.value).toHaveLength(1)
    expect(api.rowViews.value[0]!.sourceView).toEqual({
      kind: 'file',
      pass: 'scanner',
      path: 'src/auth.ts',
      label: 'Scanner · src/auth.ts'
    })
  })

  it('parses source string forms (file: + message:) into human-readable view', async () => {
    mockEntriesRef.value = [
      {
        ts: '2026-04-29T08:31:12.400Z',
        schema_version: 1,
        rules_version: '2026-04-20-1',
        pass: 2,
        session_id: 'sess_a',
        source: 'file:src/config.py',
        rule_id: 'pii_email_v1',
        kind: 'email',
        placeholder_index: 2,
        extra: {}
      },
      {
        ts: '2026-04-29T08:31:13.020Z',
        schema_version: 1,
        rules_version: '2026-04-20-1',
        pass: 2,
        session_id: 'sess_a',
        source: 'message:msg_abc123',
        rule_id: 'detect_secrets_aws_v1',
        kind: 'secret',
        placeholder_index: 1,
        extra: {}
      }
    ]
    const { useSanitizeAudit } = await importComposable()
    const api = useSanitizeAudit('/abs/ws')
    await nextTick()
    expect(api.rowViews.value[0]!.sourceView).toEqual({
      kind: 'file',
      pass: null,
      path: 'src/config.py',
      label: 'src/config.py'
    })
    expect(api.rowViews.value[1]!.sourceView).toEqual({
      kind: 'message',
      pass: null,
      message_id: 'msg_abc123',
      label: 'message msg_abc123'
    })
  })

  it('kindSummary counts unique kinds reactively', async () => {
    mockEntriesRef.value = [
      makeRow({ kind: 'secret' }),
      makeRow({ kind: 'pii' }),
      makeRow({ kind: 'pii' }),
      makeRow({ kind: 'internal' }),
      makeRow({ kind: 'secret' })
    ]
    const { useSanitizeAudit } = await importComposable()
    const api = useSanitizeAudit('/abs/ws')
    await nextTick()
    expect(api.kindSummary.value).toEqual(
      new Map([
        ['secret', 2],
        ['pii', 2],
        ['internal', 1]
      ])
    )

    mockEntriesRef.value = [
      ...(mockEntriesRef.value as unknown[]),
      makeRow({ kind: 'pii' })
    ]
    await nextTick()
    expect(api.kindSummary.value).toEqual(
      new Map([
        ['secret', 2],
        ['pii', 3],
        ['internal', 1]
      ])
    )
  })

  it('sessionTimeline groups by session_id and sorts by ts ascending', async () => {
    mockEntriesRef.value = [
      makeRow({
        ts: '2026-04-29T08:00:00.000Z',
        session_id: 'sess_a'
      }),
      makeRow({
        ts: '2026-04-29T07:30:00.000Z',
        session_id: 'sess_a'
      }),
      makeRow({
        ts: '2026-04-29T08:15:00.000Z',
        session_id: 'sess_a'
      }),
      makeRow({
        ts: '2026-04-29T09:00:00.000Z',
        session_id: 'sess_b'
      })
    ]
    const { useSanitizeAudit } = await importComposable()
    const api = useSanitizeAudit('/abs/ws')
    await nextTick()
    const sessA = api.sessionTimeline.value.get('sess_a')
    expect(sessA).toBeDefined()
    expect(sessA).toHaveLength(3)
    expect(sessA!.map((r) => r.row.ts)).toEqual([
      '2026-04-29T07:30:00.000Z',
      '2026-04-29T08:00:00.000Z',
      '2026-04-29T08:15:00.000Z'
    ])
    const sessB = api.sessionTimeline.value.get('sess_b')
    expect(sessB).toBeDefined()
    expect(sessB).toHaveLength(1)
  })

  it('source: useSanitizeAudit.ts does not call read_audit_jsonl directly', () => {
    const sourcePath = resolve(
      process.cwd(),
      'app/composables/useSanitizeAudit.ts'
    )
    const source = readFileSync(sourcePath, 'utf-8')
    expect(source).not.toContain('read_audit_jsonl')
  })
})

function makeRow(overrides: Partial<Record<string, unknown>> = {}): Record<string, unknown> {
  return {
    ts: '2026-04-29T08:30:00.000Z',
    schema_version: 1,
    rules_version: '2026-04-20-1',
    pass: 1,
    session_id: 'sess_default',
    source: 'file:src/default.ts',
    rule_id: 'pii_email_v1',
    kind: 'email',
    placeholder_index: 1,
    extra: {},
    ...overrides
  }
}
