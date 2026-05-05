import { describe, it, expect } from 'vitest'
import { parseClaudeStreamLine } from '../../src/ui/stream-parser.js'

describe('parseClaudeStreamLine', () => {
  it('parses assistant.text as thought', () => {
    const line = JSON.stringify({
      type: 'assistant',
      message: { content: [{ type: 'text', text: 'hello' }] }
    })
    expect(parseClaudeStreamLine(line)).toEqual([{ kind: 'thought', text: 'hello' }])
  })

  it('parses assistant.tool_use', () => {
    const line = JSON.stringify({
      type: 'assistant',
      message: { content: [{ type: 'tool_use', name: 'Read', input: { path: 'a' } }] }
    })
    expect(parseClaudeStreamLine(line)).toEqual([
      { kind: 'tool_use', name: 'Read', input: { path: 'a' } }
    ])
  })

  it('parses user.tool_result success', () => {
    const line = JSON.stringify({
      type: 'user',
      message: { content: [{ type: 'tool_result', content: [{ text: 'ok' }] }] }
    })
    expect(parseClaudeStreamLine(line)).toEqual([
      { kind: 'tool_result', output: 'ok', isError: false }
    ])
  })

  it('parses user.tool_result error', () => {
    const line = JSON.stringify({
      type: 'user',
      message: { content: [{ type: 'tool_result', content: 'fail', is_error: true }] }
    })
    expect(parseClaudeStreamLine(line)).toEqual([
      { kind: 'tool_result', output: 'fail', isError: true }
    ])
  })

  it('returns multiple events when assistant.content has multiple items + skips thinking', () => {
    const line = JSON.stringify({
      type: 'assistant',
      message: { content: [
        { type: 'thinking', thinking: 'internal' },
        { type: 'text', text: 'visible' },
        { type: 'tool_use', name: 'Grep', input: { pattern: 'x' } }
      ]}
    })
    expect(parseClaudeStreamLine(line)).toEqual([
      { kind: 'thought', text: 'visible' },
      { kind: 'tool_use', name: 'Grep', input: { pattern: 'x' } }
    ])
  })

  it('returns empty for system / result / rate_limit_event / unknown', () => {
    expect(parseClaudeStreamLine(JSON.stringify({ type: 'system', subtype: 'init' }))).toEqual([])
    expect(parseClaudeStreamLine(JSON.stringify({ type: 'result', subtype: 'success' }))).toEqual([])
    expect(parseClaudeStreamLine(JSON.stringify({ type: 'rate_limit_event' }))).toEqual([])
    expect(parseClaudeStreamLine(JSON.stringify({ type: 'totally_unknown_future' }))).toEqual([])
  })

  it('forward-compat: returns empty for malformed JSON instead of throwing', () => {
    expect(parseClaudeStreamLine('{{{not valid json')).toEqual([])
    expect(parseClaudeStreamLine('')).toEqual([])
  })
})
