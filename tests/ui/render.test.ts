import { describe, it, expect } from 'vitest'
import { renderEvent, renderBanner } from '../../src/ui/render.js'

describe('renderEvent', () => {
  it('renders thought with emoji', () => {
    const out = renderEvent({ kind: 'thought', text: 'thinking' }, { useEmoji: true, useColor: false })
    expect(out).toContain('🤔')
    expect(out).toContain('thinking')
  })

  it('renders thought with symbol when no emoji', () => {
    const out = renderEvent({ kind: 'thought', text: 'thinking' }, { useEmoji: false, useColor: false })
    expect(out).toContain('◆')
    expect(out).not.toContain('🤔')
  })

  it('renders tool_use Write with ✍️ + file path', () => {
    const out = renderEvent(
      { kind: 'tool_use', name: 'Write', input: { file_path: 'a.md' } },
      { useEmoji: true, useColor: false }
    )
    expect(out).toContain('✍️')
    expect(out).toContain('a.md')
  })

  it('renders tool_use Read with 🛠️', () => {
    const out = renderEvent(
      { kind: 'tool_use', name: 'Read', input: { path: 'src/x.py' } },
      { useEmoji: true, useColor: false }
    )
    expect(out).toContain('🛠️')
    expect(out).toContain('Read')
  })

  it('renders tool_result error with 👀 (color marks error)', () => {
    const out = renderEvent(
      { kind: 'tool_result', output: 'fail', isError: true },
      { useEmoji: true, useColor: false }
    )
    expect(out).toContain('👀')
    expect(out).toContain('fail')
  })

  it('renders done as empty line', () => {
    expect(renderEvent({ kind: 'done' }, { useEmoji: true, useColor: false })).toBe('')
  })
})

describe('renderBanner', () => {
  it('start banner with emoji', () => {
    const out = renderBanner('start', { path: '/tmp/r' }, { useEmoji: true, useColor: false })
    expect(out).toContain('🚌')
    expect(out).toContain('/tmp/r')
  })

  it('done banner with symbol fallback', () => {
    const out = renderBanner('done', { wikiPath: '.codebus/wiki' }, { useEmoji: false, useColor: false })
    expect(out).toContain('✓')
    expect(out).not.toContain('🎉')
  })

  it('goal banner shows goal text', () => {
    const out = renderBanner('goal', { goal: '了解結帳' }, { useEmoji: true, useColor: false })
    expect(out).toContain('🎯')
    expect(out).toContain('了解結帳')
  })

  it('hint banner with emoji', () => {
    const out = renderBanner('hint', { path: '.codebus' }, { useEmoji: true, useColor: false })
    expect(out).toContain('💡')
  })
})
