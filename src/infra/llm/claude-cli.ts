import { spawn as defaultSpawn, type ChildProcess } from 'node:child_process'
import { createInterface } from 'node:readline'
import type { LLMProvider, InvokeOptions, StreamEvent, LLMMode } from './types.js'
import { parseClaudeStreamLine } from '../../ui/stream-parser.js'

export type SpawnFn = typeof defaultSpawn

export interface ClaudeCliConfig {
  binary?: string
  timeoutMs?: number
  spawn?: SpawnFn  // injectable for testing
}

export type ExitVerdict =
  | { kind: 'success' }
  | { kind: 'oauth-needed' }
  | { kind: 'generic-error' }

export class ClaudeCliProvider implements LLMProvider {
  private child: ChildProcess | null = null
  private cfg: Required<ClaudeCliConfig>

  constructor(cfg: ClaudeCliConfig = {}) {
    this.cfg = {
      binary: cfg.binary ?? 'claude',
      timeoutMs: cfg.timeoutMs ?? 30 * 60 * 1000,
      spawn: cfg.spawn ?? defaultSpawn
    }
  }

  // Three must-set sandbox flags (spike-verified):
  //  - acceptEdits: default mode + -p blocks all Write tool calls (spike B);
  //    acceptEdits auto-accepts Write/Edit while Bash etc still ask (and are
  //    disallowed below).
  //  - disallowedTools: hard-disable Bash/WebFetch/WebSearch (+ Write/Edit
  //    in query mode for read-only enforcement).
  //  - cwd is supplied via opts.cwd in invoke(): spike E confirmed cwd =
  //    .codebus/ gives system-level isolation from user source repo.
  // No --add-dir: spike confirmed it widens, not narrows; cannot scope cwd.
  buildArgv(opts: { mode: LLMMode; vaultRoot: string }): string[] {
    const disallowed = ['Bash', 'WebFetch', 'WebSearch']
    if (opts.mode === 'query') disallowed.push('Write', 'Edit')
    void opts.vaultRoot  // kept in signature for phase 2 settings whitelist
    return [
      '-p',
      '--output-format', 'stream-json',
      '--input-format', 'stream-json',
      '--verbose',
      '--permission-mode', 'acceptEdits',
      '--disallowedTools', disallowed.join(',')
    ]
  }

  async *invoke(opts: InvokeOptions): AsyncIterable<StreamEvent> {
    const argv = this.buildArgv({ mode: opts.mode, vaultRoot: opts.vaultRoot })
    this.child = this.cfg.spawn(this.cfg.binary, argv, { cwd: opts.cwd })

    const timer = setTimeout(() => this.cancel(), this.cfg.timeoutMs)

    let stderrBuf = ''
    this.child.stderr?.on('data', (chunk) => { stderrBuf += chunk.toString() })

    // Single user-turn message via stream-json input. Real schema: top-level
    // {type:"user", message:{role:"user", content:"..."}}.
    const inputMsg = {
      type: 'user',
      message: { role: 'user', content: `${opts.systemPrompt}\n\n${opts.userMessage}` }
    }
    this.child.stdin?.write(JSON.stringify(inputMsg) + '\n')
    this.child.stdin?.end()

    const stdout = this.child.stdout
    if (!stdout) throw new Error('claude -p produced no stdout')
    const rl = createInterface({ input: stdout })

    try {
      for await (const line of rl) {
        if (!line.trim()) continue
        for (const event of parseClaudeStreamLine(line)) yield event
      }
      const code: number = await new Promise((resolve) =>
        this.child!.once('exit', (c) => resolve(c ?? 0))
      )
      const verdict = this.classifyExit(code, stderrBuf)
      if (verdict.kind === 'oauth-needed') {
        throw new Error(
          'Claude CLI 未認證 — 請在 terminal 跑 `claude` 完成 OAuth，再重新執行 codebus'
        )
      }
      if (verdict.kind === 'generic-error') {
        throw new Error(`claude -p exited ${code}: ${stderrBuf.slice(0, 500)}`)
      }
      yield { kind: 'done' }
    } finally {
      clearTimeout(timer)
    }
  }

  classifyExit(code: number, stderr: string): ExitVerdict {
    if (code === 0) return { kind: 'success' }
    if (/unauthen|auth(?:enticat)?(?:ed|ion)?|token|login/i.test(stderr)) {
      return { kind: 'oauth-needed' }
    }
    return { kind: 'generic-error' }
  }

  cancel(): void {
    if (this.child && !this.child.killed) {
      this.child.kill('SIGTERM')
    }
  }
}
