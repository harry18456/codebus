import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { resolve, join } from 'node:path'
import AuditPanel, {
  type AuditRow,
  type AuditTab
} from '~/components/audit/AuditPanel.vue'

const SANITIZE_ROWS: AuditRow[] = [
  {
    ts: '08:30:00',
    body: 'Pass 1 scanner hit on src/auth.ts',
    rule_id: 'aws_access_key',
    kind: 'secret',
    placeholder_index: 1,
    pass: 1
  },
  {
    ts: '08:31:12',
    body: 'Pass 2 provider pre-flight on prompt',
    rule_id: 'pii_email_v1',
    kind: 'email',
    placeholder_index: 2,
    pass: 2
  },
  {
    ts: '08:32:01',
    body: 'Pass 3 add_to_kb on synthesized answer',
    rule_id: 'net_internal_tld_v1',
    kind: 'internal-domain',
    placeholder_index: 1,
    pass: 3
  }
]

describe('AuditPanel sanitize tab — modified scenarios', () => {
  it('placeholder chip text is <REDACTED:{kind}#{placeholder_index}> with purple token', () => {
    const wrapper = mount(AuditPanel, {
      props: { activeTab: 'sanitize' as AuditTab, rows: SANITIZE_ROWS }
    })
    const placeholderChips = wrapper.findAll('[data-testid="sanitize-placeholder-chip"]')
    expect(placeholderChips.length).toBe(SANITIZE_ROWS.length)
    expect(placeholderChips[0]!.text()).toBe('<REDACTED:secret#1>')
    expect(placeholderChips[1]!.text()).toBe('<REDACTED:email#2>')
    expect(placeholderChips[2]!.text()).toBe('<REDACTED:internal-domain#1>')

    const cls = placeholderChips[0]!.attributes('class') ?? ''
    expect(cls).toMatch(/(text-purple|bg-purple|border-purple)/)
    // MUST NOT use other token families
    for (const forbidden of ['bg-red', 'bg-orange', 'bg-yellow', 'bg-green', 'bg-accent']) {
      expect(cls).not.toContain(forbidden)
    }
    wrapper.unmount()
  })

  it('pass chip shows human-readable label (Pass 1 / Pass 2 / Pass 3), not numeric', () => {
    const wrapper = mount(AuditPanel, {
      props: { activeTab: 'sanitize' as AuditTab, rows: SANITIZE_ROWS }
    })
    const passChips = wrapper.findAll('[data-testid="sanitize-pass-chip"]')
    expect(passChips.length).toBe(SANITIZE_ROWS.length)
    expect(passChips[0]!.text()).toBe('Pass 1')
    expect(passChips[1]!.text()).toBe('Pass 2')
    expect(passChips[2]!.text()).toBe('Pass 3')
    // No bare numeric labels — defensive grep against the chip text.
    for (const chip of passChips) {
      const t = chip.text().trim()
      expect(t).not.toBe('1')
      expect(t).not.toBe('2')
      expect(t).not.toBe('3')
    }
    wrapper.unmount()
  })

  it('sanitize tab row click emits select-row to parent and AuditPanel does not mount its own inspector', async () => {
    const wrapper = mount(AuditPanel, {
      props: { activeTab: 'sanitize' as AuditTab, rows: SANITIZE_ROWS }
    })
    const rowEls = wrapper.findAll('[data-testid="audit-row"]')
    expect(rowEls).toHaveLength(SANITIZE_ROWS.length)
    await rowEls[1]!.trigger('click')
    const events = wrapper.emitted('select-row') ?? []
    expect(events).toHaveLength(1)
    expect(events[0]).toEqual([1])
    // Parent-hosts-overlay contract: AuditPanel must not internally render
    // the SanitizerAuditInspector / inspector / drawer / modal in response.
    const html = wrapper.html().toLowerCase()
    expect(html).not.toContain('saniti zerauditinspector') // typo-safe split
    expect(html).not.toContain('class="inspector')
    expect(html).not.toContain('class="drawer')
    expect(html).not.toContain('class="modal')
    expect(html).not.toContain('class="overlay')
    expect(html).not.toContain('data-component="sanitizerauditinspector"')
    wrapper.unmount()
  })

  // ---------- Regression: pre-existing scenarios stay green ----------

  it('regression: 7 tabs render in canonical order', () => {
    const wrapper = mount(AuditPanel, {
      props: { activeTab: 'sanitize' as AuditTab, rows: [] }
    })
    const tabs = wrapper.findAll('button[data-tab]')
    expect(tabs).toHaveLength(7)
    expect(tabs.map((t) => t.attributes('data-tab'))).toEqual([
      'sanitize',
      'tool',
      'reasoning',
      'token',
      'llm',
      'kb_growth',
      'generator'
    ])
    wrapper.unmount()
  })

  it('regression: empty rows renders empty-state placeholder', () => {
    const wrapper = mount(AuditPanel, {
      props: { activeTab: 'sanitize' as AuditTab, rows: [] }
    })
    expect(wrapper.find('[data-empty="true"]').exists()).toBe(true)
    wrapper.unmount()
  })

  it('regression: emits select-row with clicked index across every tab', async () => {
    for (const tab of [
      'sanitize',
      'tool',
      'reasoning',
      'token',
      'llm',
      'kb_growth',
      'generator'
    ] as AuditTab[]) {
      const wrapper = mount(AuditPanel, {
        props: { activeTab: tab, rows: SANITIZE_ROWS }
      })
      const rowEls = wrapper.findAll('[data-testid="audit-row"]')
      await rowEls[0]!.trigger('click')
      const events = wrapper.emitted('select-row') ?? []
      expect(events.length).toBeGreaterThanOrEqual(1)
      expect(events[0]).toEqual([0])
      wrapper.unmount()
    }
  })

  it('regression: no CB_AUDIT_SAMPLES literal under web/app/', () => {
    const root = resolve(process.cwd(), 'app')
    const matches: string[] = []
    function* walk(dir: string): Generator<string> {
      for (const name of readdirSync(dir)) {
        const full = join(dir, name)
        const st = statSync(full)
        if (st.isDirectory()) {
          yield* walk(full)
        } else if (st.isFile()) {
          yield full
        }
      }
    }
    for (const file of walk(root)) {
      const text = readFileSync(file, 'utf-8')
      if (text.includes('CB_AUDIT_SAMPLES')) matches.push(file)
    }
    expect(matches).toEqual([])
  })

  it('regression: row click does not throw when parent has no listener', async () => {
    const wrapper = mount(AuditPanel, {
      props: { activeTab: 'tool' as AuditTab, rows: SANITIZE_ROWS }
    })
    const rowEls = wrapper.findAll('[data-testid="audit-row"]')
    await expect(rowEls[0]!.trigger('click')).resolves.toBeUndefined()
    wrapper.unmount()
  })

  it('regression: AuditPanel does not internally render any inspector / drawer / modal regardless of tab', () => {
    for (const tab of [
      'sanitize',
      'llm'
    ] as AuditTab[]) {
      const wrapper = mount(AuditPanel, {
        props: { activeTab: tab, rows: SANITIZE_ROWS }
      })
      const html = wrapper.html().toLowerCase()
      expect(html).not.toContain('class="inspector')
      expect(html).not.toContain('class="drawer')
      expect(html).not.toContain('class="modal')
      expect(html).not.toContain('class="overlay')
      wrapper.unmount()
    }
  })
})
