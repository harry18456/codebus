import { describe, it, expect } from 'vitest'
import { repairWikilinkList } from '../../../src/core/wiki/frontmatter-repair.js'

describe('repairWikilinkList', () => {
  it('quotes wikilink list values', () => {
    const input = `related: [[a]], [[b]], [[c]]`
    const output = repairWikilinkList(input)
    expect(output).toBe(`related: ["[[a]]", "[[b]]", "[[c]]"]`)
  })

  it('handles single wikilink with no comma', () => {
    const input = `related: [[only-one]]`
    const output = repairWikilinkList(input)
    expect(output).toBe(`related: ["[[only-one]]"]`)
  })

  it('leaves already-quoted wikilink list untouched', () => {
    const input = `related: ["[[a]]", "[[b]]"]`
    expect(repairWikilinkList(input)).toBe(input)
  })

  it('only repairs wikilink-shaped lines, not other arrays', () => {
    const input = `tags: [foo, bar]`
    expect(repairWikilinkList(input)).toBe(input)
  })

  it('repairs each line independently in multi-line input', () => {
    const input = `related: [[a]], [[b]]\nsee_also: [[x]], [[y]]`
    const expected = `related: ["[[a]]", "[[b]]"]\nsee_also: ["[[x]]", "[[y]]"]`
    expect(repairWikilinkList(input)).toBe(expected)
  })
})
