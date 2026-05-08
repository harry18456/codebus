import { describe, it, expect } from 'vitest'
import { utcTodayISO } from '../../../src/core/wiki/date.js'

describe('utcTodayISO', () => {
  it('returns YYYY-MM-DD format (UTC)', () => {
    const today = utcTodayISO()
    expect(today).toMatch(/^\d{4}-\d{2}-\d{2}$/)
  })

  it('matches new Date().toISOString().slice(0,10)', () => {
    expect(utcTodayISO()).toBe(new Date().toISOString().slice(0, 10))
  })
})
