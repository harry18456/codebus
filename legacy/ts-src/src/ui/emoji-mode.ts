export type EmojiMode = 'auto' | 'on' | 'off'

export interface EmojiEnv {
  isTTY: boolean
  env: Record<string, string | undefined>
}

export function resolveEmojiMode(flag: EmojiMode, runtime: EmojiEnv): boolean {
  if (flag === 'on') return true
  if (flag === 'off') return false
  return runtime.isTTY
      && !runtime.env.CI
      && !runtime.env.NO_EMOJI
      && runtime.env.TERM !== 'dumb'
}

export function detectRuntime(): EmojiEnv {
  return {
    isTTY: Boolean(process.stdout.isTTY),
    env: process.env
  }
}
