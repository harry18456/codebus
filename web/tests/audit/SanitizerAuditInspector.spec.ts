import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { ref } from 'vue'
import type { SanitizerRule } from '~/composables/useSanitizerRules'

// Mock useSanitizerRules so the inspector's rule explainer can be driven
// per-test. By default lookup returns null; specific tests override the
// mock to surface a known rule.
const lookupMock = vi.fn(() => null as SanitizerRule | null)
const loadOnceMock = vi.fn(async () => {})
vi.mock('~/composables/useSanitizerRules', () => ({
  useSanitizerRules: () => ({
    snapshot: ref(null),
    loadOnce: loadOnceMock,
    lookup: (rule_id: string) => lookupMock(rule_id)
  })
}))

import SanitizerAuditInspector from '~/components/audit/SanitizerAuditInspector.vue'
import type { SanitizeAuditEntry } from '~/composables/useSanitizeAudit'

function makeRow(overrides: Partial<SanitizeAuditEntry> = {}): SanitizeAuditEntry {
  return {
    ts: '2026-04-29T08:30:01.123Z',
    schema_version: 1,
    rules_version: '2026-04-20-1',
    pass: 2,
    session_id: 'sess_abc123',
    source: { pass: 'provider', path: 'src/auth.ts' },
    rule_id: 'aws_access_key',
    kind: 'secret',
    placeholder_index: 1,
    extra: {},
    ...overrides
  }
}

describe('SanitizerAuditInspector overlay metadata', () => {
  it('renders all ten metadata field values when row has full payload', () => {
    const row = makeRow({
      source: { pass: 'provider', path: 'src/auth.ts' },
      extra: { foo: 'bar' }
    })
    const wrapper = mount(SanitizerAuditInspector, { props: { row } })
    const text = wrapper.text()
    expect(text).toContain('2026-04-29T08:30:01.123Z')
    expect(text).toContain('1') // schema_version
    expect(text).toContain('2026-04-20-1') // rules_version
    expect(text).toContain('Pass 2') // pass label substring
    expect(text).toContain('sess_abc123') // session_id
    expect(text).toContain('src/auth.ts') // source path
    expect(text).toContain('aws_access_key') // rule_id
    expect(text).toContain('secret') // kind
    expect(text).toContain('foo') // extra key visible
    expect(text).toContain('bar') // extra value visible
    // Placeholder identifier text in header
    expect(text).toContain('<REDACTED:secret#1>')
    wrapper.unmount()
  })

  it('maps pass integer to human-readable label (Pass 1/2/3)', () => {
    const cases: Array<[number, string]> = [
      [1, 'Pass 1 · Scanner (KB ingestion)'],
      [2, 'Pass 2 · Provider pre-flight (LLM call)'],
      [3, 'Pass 3 · Q&A add_to_kb']
    ]
    for (const [passVal, expected] of cases) {
      const wrapper = mount(SanitizerAuditInspector, {
        props: { row: makeRow({ pass: passVal }) }
      })
      expect(wrapper.text()).toContain(expected)
      wrapper.unmount()
    }
  })

  it('does not attempt raw value reconstruction (no AKIA in DOM, no unknown audit reads)', () => {
    const row = makeRow({
      rule_id: 'aws_access_key',
      kind: 'secret',
      source: 'file:src/auth.ts'
    })
    const wrapper = mount(SanitizerAuditInspector, { props: { row } })
    const html = wrapper.html()
    // The inspector MUST NOT render any text matching the original AWS key
    // pattern — its job is metadata only.
    expect(/AKIA[0-9A-Z]{16}/.test(html)).toBe(false)
    wrapper.unmount()
  })

  it('renders extra.allowlisted=true as green checkmark chip (not generic key:value row)', () => {
    const row = makeRow({ extra: { allowlisted: true } })
    const wrapper = mount(SanitizerAuditInspector, { props: { row } })
    const text = wrapper.text()
    expect(text).toContain('✓ allowlisted')
    // Must not also render the generic "allowlisted: true" key-value row.
    expect(text).not.toContain('allowlisted: true')
    wrapper.unmount()
  })

  it('renders fallback text for unknown source shape (no throw)', () => {
    // Suppress console error spam from Vue runtime (if any).
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    expect(() => {
      const wrapper = mount(SanitizerAuditInspector, {
        props: {
          row: makeRow({ source: { foo: 'bar' } as unknown as SanitizeAuditEntry['source'] })
        }
      })
      expect(wrapper.text()).toContain('(unknown source format)')
      wrapper.unmount()
    }).not.toThrow()
    expect(errSpy).not.toHaveBeenCalled()
    errSpy.mockRestore()
  })

  it('exposes no mutation affordances (delete / edit / remove / redact further / modify)', () => {
    const wrapper = mount(SanitizerAuditInspector, { props: { row: makeRow() } })
    const buttons = wrapper.findAll('button, a, [role="button"]')
    for (const btn of buttons) {
      const label = (btn.text() + ' ' + (btn.attributes('aria-label') ?? '')).toLowerCase()
      expect(label).not.toContain('delete')
      expect(label).not.toContain('edit')
      expect(label).not.toContain('remove')
      expect(label).not.toContain('redact further')
      expect(label).not.toContain('modify')
    }
    wrapper.unmount()
  })
})

describe('SanitizerAuditInspector D-015 banner', () => {
  it('always renders the verbatim D-015 banner text', () => {
    const wrapper = mount(SanitizerAuditInspector, { props: { row: makeRow() } })
    const text = wrapper.text()
    expect(text).toContain('Audit metadata only · raw values are not retained per D-015.')
    expect(text).toContain('Placeholder reveal requires a future audit-unlock capability.')
    wrapper.unmount()
  })

  it('has no hideBanner / dismissBanner / bannerVisible prop and no dismiss button', () => {
    const wrapper = mount(SanitizerAuditInspector, { props: { row: makeRow() } })
    // The banner element should not have a close button paired with it.
    const html = wrapper.html().toLowerCase()
    expect(html).not.toMatch(/aria-label="dismiss banner"/)
    expect(html).not.toMatch(/aria-label="close banner"/)
    expect(html).not.toMatch(/data-action="dismiss-banner"/)
    // The component public props surface (defineProps) should not declare
    // any of the forbidden names. We check via the rendered DOM not
    // including a banner-toggle button as a proxy.
    const buttons = wrapper.findAll('button')
    for (const btn of buttons) {
      const label = (btn.text() + ' ' + (btn.attributes('aria-label') ?? '')).toLowerCase()
      expect(label).not.toMatch(/banner/)
    }
    wrapper.unmount()
  })

  it('SANITIZER_AUDIT_BANNER is exported as a constant and matches the rendered text', async () => {
    const mod = await import('~/components/audit/SanitizerAuditInspector.vue')
    expect(typeof mod.SANITIZER_AUDIT_BANNER).toBe('string')
    expect(mod.SANITIZER_AUDIT_BANNER).toContain(
      'Audit metadata only · raw values are not retained per D-015.'
    )
    expect(mod.SANITIZER_AUDIT_BANNER).toContain(
      'Placeholder reveal requires a future audit-unlock capability.'
    )
  })
})
