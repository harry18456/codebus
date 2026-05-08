import type { StreamEvent } from '../infra/llm/types.js'

// Schema verified by spike against claude CLI 2.1.126:
//   {type:"system", subtype:...}                                              → skip
//   {type:"assistant", message:{content:[{type:"text"|"tool_use"|"thinking"}]}}
//   {type:"user", message:{content:[{type:"tool_result"}]}}
//   {type:"rate_limit_event"}                                                 → skip
//   {type:"result", subtype:...}                                              → skip
//
// Returns 0..N StreamEvent per line; assistant.content[] can carry text +
// tool_use together. Caller iterates the array.
export function parseClaudeStreamLine(rawLine: string): StreamEvent[] {
  let parsed: any
  try { parsed = JSON.parse(rawLine) } catch { return [] }

  if (parsed?.type === 'assistant') {
    const items = parsed.message?.content
    if (!Array.isArray(items)) return []
    const events: StreamEvent[] = []
    for (const item of items) {
      if (item?.type === 'text' && item.text) {
        events.push({ kind: 'thought', text: String(item.text) })
      } else if (item?.type === 'tool_use') {
        events.push({
          kind: 'tool_use',
          name: String(item.name ?? ''),
          input: item.input
        })
      }
      // 'thinking' items skipped (internal reasoning, not user-facing)
    }
    return events
  }

  if (parsed?.type === 'user') {
    const items = parsed.message?.content
    if (!Array.isArray(items)) return []
    const events: StreamEvent[] = []
    for (const item of items) {
      if (item?.type === 'tool_result') {
        const content = Array.isArray(item.content)
          ? item.content.map((c: any) => c?.text ?? '').join('')
          : String(item.content ?? '')
        events.push({
          kind: 'tool_result',
          output: content,
          isError: Boolean(item.is_error)
        })
      }
    }
    return events
  }

  // system / result / rate_limit_event / unknown → skip
  return []
}
