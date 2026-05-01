// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/llm-call-inspector/spec.md
//   Requirement: AuditPanel filters llm tab rows by role for PII separation

import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import AuditPanel, { type AuditRow } from '~/components/audit/AuditPanel.vue'

function row(role: string | undefined, idx: number): AuditRow {
  return {
    ts: `10:00:0${idx}`,
    body: `entry ${idx}`,
    role
  }
}

describe('<AuditPanel> PII filter on llm tab', () => {
  it('llm tab row count excludes pii_detection rows by default', () => {
    const rows: AuditRow[] = [
      row('chat', 0),
      row('reasoning', 1),
      row('chat', 2),
      row('pii_detection', 3),
      row('pii_detection', 4)
    ]
    const wrapper = mount(AuditPanel, {
      props: {
        activeTab: 'llm',
        rows,
        counts: { llm: 5 }
      }
    })
    const renderedRows = wrapper.findAll('[data-testid="audit-row"]')
    expect(renderedRows.length).toBe(3)
    const toggle = wrapper.get('[data-testid="audit-panel-toggle-pii"]')
    expect(toggle.text()).toContain('2')
  })

  it('sanitize tab unaffected by hidePiiDetection prop', () => {
    const rows: AuditRow[] = [
      { ts: '10:00:01', body: 'sanitize evt', kind: 'EMAIL', placeholder_index: 1, pass: 1 },
      { ts: '10:00:02', body: 'sanitize evt', kind: 'TOKEN', placeholder_index: 2, pass: 2 }
    ]
    const wrapper = mount(AuditPanel, {
      props: {
        activeTab: 'sanitize',
        rows,
        hidePiiDetection: true
      }
    })
    const renderedRows = wrapper.findAll('[data-testid="audit-row"]')
    expect(renderedRows.length).toBe(2)
    expect(wrapper.find('[data-testid="audit-panel-toggle-pii"]').exists()).toBe(
      false
    )
  })
})
