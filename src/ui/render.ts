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

function lead(key: Glyph, useEmoji: boolean): string {
  return useEmoji ? EMOJI[key] : SYMBOL[key]
}

function colored(text: string, color: 'cyan' | 'green' | 'dim' | 'red', useColor: boolean): string {
  if (!useColor) return text
  return chalk[color](text)
}

export function renderEvent(event: StreamEvent, opts: RenderOptions): string {
  switch (event.kind) {
    case 'thought':
      return `${lead('thought', opts.useEmoji)} ${colored('[Agent 思考]', 'dim', opts.useColor)} ${event.text}`
    case 'tool_use': {
      if (event.name === 'Write' || event.name === 'Edit') {
        const fp = (event.input as { file_path?: string } | null)?.file_path ?? '(unknown)'
        return `${lead('write', opts.useEmoji)} ${colored('[正在生成]', 'green', opts.useColor)} ${fp}`
      }
      const argStr = JSON.stringify(event.input ?? null)
      const trimmed = argStr.length > 80 ? argStr.slice(0, 80) + '…' : argStr
      return `${lead('tool', opts.useEmoji)} ${colored('[呼叫工具]', 'cyan', opts.useColor)} ${event.name}(${trimmed})`
    }
    case 'tool_result': {
      const color = event.isError ? 'red' : 'dim'
      const body = event.output.length > 200 ? event.output.slice(0, 200) + '…' : event.output
      return `${lead('result', opts.useEmoji)} ${colored('[觀察結果]', color, opts.useColor)} ${body}`
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
    case 'start': return `${sym} CodeBus 啟動！正在駛入 ${data.path} ...`
    case 'goal': return `${sym} 任務目標：${data.goal}`
    case 'done': return `${sym} 完成。wiki 已生成於 ${data.wikiPath}`
    case 'hint': return `${sym} 請用 Obsidian 開 ${data.path}`
  }
}
