import chalk from 'chalk'
import type { StreamEvent } from '../infra/llm/types.js'

export interface RenderOptions {
  useEmoji: boolean
  useColor: boolean
}

const EMOJI = {
  thought: '🤔',
  tool: '🛠️ ',
  write: '✍️ ',
  result: '👀',
  start: '🚌',
  goal: '🎯',
  done: '🎉',
  hint: '💡'
} as const

const SYMBOL = {
  thought: '◆',
  tool: '→',
  write: '+',
  result: '←',
  start: '▶',
  goal: '◎',
  done: '✓',
  hint: 'i'
} as const

type Glyph = keyof typeof EMOJI

const INDENT = '    '

function lead(key: Glyph, useEmoji: boolean): string {
  return useEmoji ? EMOJI[key] : SYMBOL[key]
}

function colored(text: string, color: 'cyan' | 'green' | 'dim' | 'red', useColor: boolean): string {
  if (!useColor) return text
  return chalk[color](text)
}

// Display Windows backslash paths as forward-slash for visual consistency.
function normalizePath(p: string): string {
  return p.replace(/\\/g, '/')
}

// Unwrap the primary arg for known tools so output is `Read(file)` not
// `Read({"file_path":"..."})` with double-escaped quotes.
function formatToolArgs(name: string, input: unknown): string {
  if (!input || typeof input !== 'object') return ''
  const inp = input as Record<string, unknown>
  switch (name) {
    case 'Read':
    case 'Write':
    case 'Edit':
    case 'NotebookEdit':
      return normalizePath(String(inp.file_path ?? inp.notebook_path ?? ''))
    case 'Glob':
      return String(inp.pattern ?? '')
    case 'Grep': {
      const pattern = String(inp.pattern ?? '')
      const path = inp.path ? `, ${normalizePath(String(inp.path))}` : ''
      return `${pattern}${path}`
    }
    default: {
      const json = JSON.stringify(inp)
      return json.length > 80 ? json.slice(0, 80) + '…' : json
    }
  }
}

// Suppress redundant Write/Edit success echo (the Write event already
// displayed the path one line above; the result is duplicate noise).
// Two known formats from Claude CLI tools:
//   Write (new file):      "File created successfully at: PATH"
//   Edit (existing file):  "The file PATH has been updated successfully."
function isWriteSuccessEcho(text: string): boolean {
  return /^File (created|updated|edited) successfully at:/.test(text)
      || /^The file .+ has been (updated|edited|created|written) successfully\.?$/.test(text)
}

// Condense Read tool output (cat -n style line-numbered file contents)
// to a count summary — the agent uses the content internally; the user
// just needs to know the read happened.
function readLineCount(text: string): number | null {
  const lines = text.split('\n')
  const numbered = lines.filter((l) => /^\s*\d+\s/.test(l))
  // Heuristic: must be majority cat-n style to count as Read output.
  return numbered.length >= 3 && numbered.length / lines.length > 0.5
    ? numbered.length
    : null
}

function indent(text: string): string {
  return text.split('\n').map((l) => `${INDENT}${l}`).join('\n')
}

// Light markdown styling for thought text. Phase 1 covers the most common
// noise sources only: bold / inline code / wikilinks. Italic and headings
// skipped — italic risks colliding with literal `*` in code, headings are
// rare in agent thought stream. Phase 2 will extend (see spec §16 Terminal
// output 升級) to add OSC 8 hyperlink for wikilinks (clickable in modern
// terminals) once vault root context is propagated to the render layer.
//
// When useColor is false (CI / NO_COLOR / non-TTY), markdown stays literal
// so log files / CI capture see exactly what the agent wrote.
function styleMarkdown(text: string, useColor: boolean): string {
  if (!useColor) return text
  return text
    .replace(/\*\*([^*]+)\*\*/g, (_, body: string) => chalk.bold(body))
    .replace(/`([^`]+)`/g, (_, body: string) => chalk.cyan(body))
    .replace(/(\[\[[^\]]+\]\])/g, (_, body: string) => chalk.cyan.underline(body))
}

export function renderEvent(event: StreamEvent, opts: RenderOptions): string {
  switch (event.kind) {
    case 'thought': {
      // Multi-line thought (e.g. final summary with bullets / markdown) →
      // two-line with indented body so the structure aligns with the rest
      // of the output. Single-line thought stays inline (text IS content).
      const styled = styleMarkdown(event.text, opts.useColor)
      const label = `${lead('thought', opts.useEmoji)} ${colored('[Agent 思考]', 'dim', opts.useColor)}`
      if (event.text.includes('\n')) {
        return `${label}\n${indent(styled)}`
      }
      return `${label} ${styled}`
    }

    case 'tool_use': {
      // Special-case Write/Edit to use the write glyph.
      if (event.name === 'Write' || event.name === 'Edit') {
        const fp = normalizePath(String((event.input as { file_path?: string } | null)?.file_path ?? '(unknown)'))
        return `${lead('write', opts.useEmoji)} ${colored('[正在生成]', 'green', opts.useColor)}\n${INDENT}${fp}`
      }
      // Two-line for other tools: label / indented `Tool(args)`.
      const args = formatToolArgs(event.name, event.input)
      return `${lead('tool', opts.useEmoji)} ${colored('[呼叫工具]', 'cyan', opts.useColor)}\n${INDENT}${event.name}(${args})`
    }

    case 'tool_result': {
      if (isWriteSuccessEcho(event.output)) return ''  // suppress duplicate
      const color = event.isError ? 'red' : 'dim'
      const lineCount = readLineCount(event.output)
      const body = lineCount !== null
        ? `(${lineCount} lines)`
        : (event.output.length > 200 ? event.output.slice(0, 200) + '…' : event.output)
      return `${lead('result', opts.useEmoji)} ${colored('[觀察結果]', color, opts.useColor)}\n${indent(body)}`
    }

    case 'done':
      return ''
  }
}

export type BannerKind = 'start' | 'goal' | 'done' | 'hint'
export type BannerData = Record<string, string>

export function renderBanner(kind: BannerKind, data: BannerData, opts: RenderOptions): string {
  const sym = lead(kind, opts.useEmoji)
  switch (kind) {
    // start / done banners borrow the「上車舞」chant — codebus is the bus,
    // user is the passenger. Phrasing chosen to fire only at boarding /
    // disembarking moments so the meme stays light, not noisy.
    case 'start': return `${sym} 來囉來囉~ CodeBus 駛入 ${normalizePath(data.path)} ...`
    case 'goal': return `${sym} 任務目標：${data.goal}`
    case 'done': return `${sym} 掰掰~下車囉！wiki 已生成於 ${normalizePath(data.wikiPath)}`
    case 'hint': return `${sym} 請用 Obsidian 開 ${normalizePath(data.path)}`
  }
}
