import { describe, it, expect } from 'vitest'
import { resolveEmojiMode } from '../../src/ui/emoji-mode.js'

describe('resolveEmojiMode', () => {
  it('returns true when flag=on regardless of env', () => {
    expect(resolveEmojiMode('on', { isTTY: false, env: { CI: '1' } })).toBe(true)
  })

  it('returns false when flag=off', () => {
    expect(resolveEmojiMode('off', { isTTY: true, env: {} })).toBe(false)
  })

  it('auto: returns true when tty + no CI + no NO_EMOJI + TERM != dumb', () => {
    expect(resolveEmojiMode('auto', { isTTY: true, env: {} })).toBe(true)
  })

  it('auto: returns false when in CI', () => {
    expect(resolveEmojiMode('auto', { isTTY: true, env: { CI: '1' } })).toBe(false)
  })

  it('auto: returns false when NO_EMOJI is set', () => {
    expect(resolveEmojiMode('auto', { isTTY: true, env: { NO_EMOJI: '1' } })).toBe(false)
  })

  it('auto: returns false when not TTY', () => {
    expect(resolveEmojiMode('auto', { isTTY: false, env: {} })).toBe(false)
  })

  it('auto: returns false when TERM=dumb', () => {
    expect(resolveEmojiMode('auto', { isTTY: true, env: { TERM: 'dumb' } })).toBe(false)
  })
})
