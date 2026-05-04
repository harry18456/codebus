import { describe, it, expect } from 'vitest'
import { renderEvent, renderBanner } from '../../src/ui/render.js'

describe('renderEvent', () => {
  it('renders thought with emoji (one-line)', () => {
    const out = renderEvent({ kind: 'thought', text: 'thinking' }, { useEmoji: true, useColor: false })
    expect(out).toContain('🤔')
    expect(out).toContain('thinking')
    expect(out.split('\n').length).toBe(1)
  })

  it('renders thought with symbol when no emoji', () => {
    const out = renderEvent({ kind: 'thought', text: 'thinking' }, { useEmoji: false, useColor: false })
    expect(out).toContain('◆')
    expect(out).not.toContain('🤔')
  })

  it('renders tool_use Write with ✍️ + indented file path (two-line)', () => {
    const out = renderEvent(
      { kind: 'tool_use', name: 'Write', input: { file_path: 'wiki/pages/a.md' } },
      { useEmoji: true, useColor: false }
    )
    expect(out).toContain('✍️')
    expect(out).toContain('a.md')
    const [first, second] = out.split('\n')
    expect(first).toContain('[正在生成]')
    expect(second).toBe('    wiki/pages/a.md')
  })

  it('renders tool_use Read with two-line label + Read(file_path)', () => {
    const out = renderEvent(
      { kind: 'tool_use', name: 'Read', input: { file_path: 'src/x.py' } },
      { useEmoji: true, useColor: false }
    )
    expect(out).toContain('🛠️')
    const [first, second] = out.split('\n')
    expect(first).toContain('[呼叫工具]')
    expect(second).toBe('    Read(src/x.py)')
  })

  it('renders tool_use Glob unwrapping pattern', () => {
    const out = renderEvent(
      { kind: 'tool_use', name: 'Glob', input: { pattern: 'raw/code/**/*' } },
      { useEmoji: true, useColor: false }
    )
    const lines = out.split('\n')
    expect(lines[1]).toBe('    Glob(raw/code/**/*)')
  })

  it('renders tool_use Grep with pattern + path', () => {
    const out = renderEvent(
      { kind: 'tool_use', name: 'Grep', input: { pattern: 'hash', path: 'src' } },
      { useEmoji: true, useColor: false }
    )
    const lines = out.split('\n')
    expect(lines[1]).toBe('    Grep(hash, src)')
  })

  it('normalizes Windows backslash paths to forward slash', () => {
    const out = renderEvent(
      { kind: 'tool_use', name: 'Read', input: { file_path: 'D:\\side_project\\app\\src\\main.ts' } },
      { useEmoji: true, useColor: false }
    )
    expect(out).not.toContain('\\')
    expect(out).toContain('D:/side_project/app/src/main.ts')
  })

  it('renders tool_result error with 👀 (color marks error)', () => {
    const out = renderEvent(
      { kind: 'tool_result', output: 'fail', isError: true },
      { useEmoji: true, useColor: false }
    )
    expect(out).toContain('👀')
    expect(out).toContain('fail')
  })

  it('suppresses Write tool_result success echo', () => {
    expect(renderEvent(
      { kind: 'tool_result', output: 'File created successfully at: wiki/pages/a.md', isError: false },
      { useEmoji: true, useColor: false }
    )).toBe('')
    expect(renderEvent(
      { kind: 'tool_result', output: 'File updated successfully at: wiki/index.md', isError: false },
      { useEmoji: true, useColor: false }
    )).toBe('')
  })

  it('suppresses Edit tool_result success echo (different phrasing)', () => {
    expect(renderEvent(
      { kind: 'tool_result', output: 'The file D:\\side_project\\app\\.codebus\\wiki\\index.md has been updated successfully.', isError: false },
      { useEmoji: true, useColor: false }
    )).toBe('')
    expect(renderEvent(
      { kind: 'tool_result', output: 'The file wiki/log.md has been edited successfully', isError: false },
      { useEmoji: true, useColor: false }
    )).toBe('')
  })

  it('condenses Read tool_result (cat -n style) to line count', () => {
    const fileContent = '   1  line one\n   2  line two\n   3  line three\n   4  line four'
    const out = renderEvent(
      { kind: 'tool_result', output: fileContent, isError: false },
      { useEmoji: true, useColor: false }
    )
    expect(out).toContain('(4 lines)')
    // Should NOT include the actual line content
    expect(out).not.toContain('line one')
  })

  it('keeps generic tool_result text (truncated, indented)', () => {
    const out = renderEvent(
      { kind: 'tool_result', output: 'arbitrary tool output', isError: false },
      { useEmoji: true, useColor: false }
    )
    expect(out).toContain('arbitrary tool output')
    const lines = out.split('\n')
    expect(lines[1]).toBe('    arbitrary tool output')
  })

  it('renders done as empty line', () => {
    expect(renderEvent({ kind: 'done' }, { useEmoji: true, useColor: false })).toBe('')
  })
})

describe('renderBanner', () => {
  it('start banner with emoji + path normalized', () => {
    const out = renderBanner('start', { path: 'D:\\repo' }, { useEmoji: true, useColor: false })
    expect(out).toContain('🚌')
    expect(out).toContain('D:/repo')
    expect(out).not.toContain('\\')
  })

  it('done banner with symbol fallback + path normalized', () => {
    const out = renderBanner('done', { wikiPath: 'D:\\repo\\.codebus\\wiki' }, { useEmoji: false, useColor: false })
    expect(out).toContain('✓')
    expect(out).not.toContain('🎉')
    expect(out).toContain('D:/repo/.codebus/wiki')
  })

  it('goal banner shows goal text', () => {
    const out = renderBanner('goal', { goal: '了解結帳' }, { useEmoji: true, useColor: false })
    expect(out).toContain('🎯')
    expect(out).toContain('了解結帳')
  })

  it('hint banner with emoji + path normalized', () => {
    const out = renderBanner('hint', { path: 'D:\\repo\\.codebus\\wiki' }, { useEmoji: true, useColor: false })
    expect(out).toContain('💡')
    expect(out).toContain('D:/repo/.codebus/wiki')
  })
})
