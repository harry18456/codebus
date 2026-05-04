#!/usr/bin/env node
import { Command } from 'commander'
import { unlinkSync } from 'node:fs'
import { runInit } from './commands/init.js'
import { runGoal } from './commands/goal.js'
import { runQuery } from './commands/query.js'
import { vaultPaths } from './core/vault/layout.js'
import { checkRepoIsNotVault } from './core/vault/sanity-check.js'
import { ClaudeCliProvider } from './infra/llm/claude-cli.js'
import { loadGlobalConfig } from './infra/global-config.js'
import { resolveEmojiMode, detectRuntime, type EmojiMode } from './ui/emoji-mode.js'
import { renderEvent, renderBanner } from './ui/render.js'
import type { StreamEvent } from './infra/llm/types.js'

const program = new Command()
program
  .name('codebus')
  .description('Build an LLM wiki for any codebase via claude -p')
  .version('0.1.0')
  .option('--repo <path>', 'repo path (default: cwd)', process.cwd())
  .option('--goal <text>', 'build wiki for this goal')
  .option('--query <text>', 'ask the wiki a question')
  .option('--debug', 'verbose stream-json output')
  .option('--emoji <mode>', 'emoji mode: auto | on | off')
  .option('--no-emoji', 'sugar for --emoji off')

program.parse()
const opts = program.opts()

let activeProvider: { cancel(): void } | null = null

async function main(): Promise<void> {
  // CRITICAL (review iter-8): declare repo BEFORE registering SIGINT handler.
  // SIGINT is async; if Ctrl+C fires (or buffered ^C) between handler
  // registration and `const repo = opts.repo`, handler accesses repo in
  // TDZ → ReferenceError.
  const repo: string = opts.repo

  // Reject obvious mistakes (pointing --repo at a vault / inside a vault /
  // at ~/.codebus/) before any disk mutation. Without this, codebus would
  // gleefully create nested .codebus/.codebus/ vaults and the agent would
  // try to wiki-ify its own wiki.
  const sanity = checkRepoIsNotVault(repo)
  if (!sanity.ok) {
    console.error(`error: ${sanity.reason}`)
    if (sanity.hint) console.error(`hint: ${sanity.hint}`)
    process.exit(2)
  }

  const globalCfg = await loadGlobalConfig()
  const VALID = ['auto', 'on', 'off'] as const
  const cliEmoji =
    typeof opts.emoji === 'string' && (VALID as readonly string[]).includes(opts.emoji)
      ? (opts.emoji as EmojiMode)
      : undefined
  // Settings priority for emoji mode (per spec §17.3):
  //   1. --emoji on/off (explicit enum wins)
  //   2. --no-emoji (sugar = off; commander sets opts.emoji=false)
  //   3. NO_EMOJI env
  //   4. ~/.codebus/config.yaml
  //   5. 'auto' default
  const emojiFlag: EmojiMode =
    cliEmoji !== undefined ? cliEmoji :
    opts.emoji === false ? 'off' :
    process.env.NO_EMOJI ? 'off' :
    (globalCfg.emoji ?? 'auto')
  const useEmoji = resolveEmojiMode(emojiFlag, detectRuntime())
  const useColor = Boolean(process.stdout.isTTY) && !process.env.NO_COLOR
  const renderOpts = { useEmoji, useColor }

  // SIGINT (Ctrl+C): cancel provider + best-effort race-free unlink lock.
  process.on('SIGINT', () => {
    console.error('\n中止 — wiki 可能半寫；可手動 git -C .codebus reset --hard 復原')
    if (activeProvider) activeProvider.cancel()
    try {
      unlinkSync(vaultPaths(repo).lock)
    } catch (e: unknown) {
      const code = (e as NodeJS.ErrnoException | undefined)?.code
      if (code !== 'ENOENT') { /* swallow — best effort cleanup */ }
    }
    process.exit(130)
  })

  if (!opts.goal && !opts.query) {
    console.log(renderBanner('start', { path: repo }, renderOpts))
    await runInit(repo)
    // Init creates an empty vault — do NOT reuse the goal-completion
    // banners ("wiki 已生成" / "Obsidian 開") because no wiki content
    // exists yet. Tell the user the actual next step.
    const ok = useEmoji ? '✨' : '✓'
    const tip = useEmoji ? '💡' : 'i'
    const vaultPath = `${repo}/.codebus`.replace(/\\/g, '/')
    console.log(`${ok} Vault 已初始化於 ${vaultPath}`)
    console.log(`${tip} 下一步：codebus --goal "<你的探索目標>"`)
    return
  }

  const provider = new ClaudeCliProvider()
  activeProvider = provider
  const onEvent = (e: StreamEvent): void => {
    const line = renderEvent(e, renderOpts)
    if (line) console.log(line)
  }

  if (opts.goal) {
    console.log(renderBanner('start', { path: repo }, renderOpts))
    console.log(renderBanner('goal', { goal: String(opts.goal) }, renderOpts))
    const result = await runGoal({ repoRoot: repo, goal: String(opts.goal), provider, onEvent })
    if (result.wikiChanged) {
      console.log(renderBanner('done', { wikiPath: `${repo}/.codebus/wiki` }, renderOpts))
      // Point Obsidian at the wiki/ subdir, not .codebus/ root — vault opens
      // clean (no .git / raw / output / goals.jsonl / CLAUDE.md clutter to
      // hide). Wikilinks still resolve since all pages live under wiki/.
      console.log(renderBanner('hint', { path: `${repo}/.codebus/wiki` }, renderOpts))
    } else {
      // Agent ran but didn't write anything — typically self-judged the
      // goal as not wiki-shaped (e.g. "create test.md" violates schema)
      // or refused for other reasons. Do NOT show the goal-completion
      // banner ("wiki 已生成") which would be misleading.
      const shrug = useEmoji ? '🤷' : '~'
      console.log(`${shrug} Agent 跑完但沒動 wiki — 可能此 goal 不適合（agent 自我判斷拒絕）`)
      console.log(`   raw 已 sync、goals.jsonl 已記錄；wiki 內容無變化`)
    }
  } else if (opts.query) {
    console.log(renderBanner('start', { path: repo }, renderOpts))
    await runQuery({ repoRoot: repo, query: String(opts.query), provider, onEvent })
  }
}

main().catch((err: Error) => {
  console.error(`error: ${err.message}`)
  process.exit(1)
})
