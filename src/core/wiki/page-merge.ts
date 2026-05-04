import type { ParsedPage, SourceRef } from './types.js'

function uniqueSources(a: SourceRef[], b: SourceRef[]): SourceRef[] {
  const seen = new Set<string>()
  const out: SourceRef[] = []
  for (const s of [...a, ...b]) {
    if (!seen.has(s.path)) {
      seen.add(s.path)
      out.push(s)
    }
  }
  return out
}

function uniqueStrings(...lists: string[][]): string[] {
  const seen = new Set<string>()
  const out: string[] = []
  for (const list of lists) {
    for (const s of list) {
      if (!seen.has(s)) {
        seen.add(s)
        out.push(s)
      }
    }
  }
  return out
}

export function mergePage(
  existing: ParsedPage,
  incoming: ParsedPage,
  goalText: string,
  today: string
): ParsedPage {
  const sources = uniqueSources(existing.frontmatter.sources, incoming.frontmatter.sources)
  // Union three sources of goals: existing + this run's goalText + incoming's pre-filled goals.
  // (Iter-8 review caught an earlier impl that ignored incoming.frontmatter.goals.)
  const goals = uniqueStrings(
    existing.frontmatter.goals,
    [goalText],
    incoming.frontmatter.goals
  )
  const related = uniqueStrings(existing.frontmatter.related, incoming.frontmatter.related)

  const sectionHeader = `## from goal: ${goalText} (${today})`
  const body = `${existing.body.trimEnd()}\n\n${sectionHeader}\n\n${incoming.body.trim()}\n`

  return {
    frontmatter: {
      title: existing.frontmatter.title,
      type: existing.frontmatter.type,
      created: existing.frontmatter.created,
      sources,
      goals,
      related,
      updated: today,
      stale: existing.frontmatter.stale
    },
    body
  }
}
