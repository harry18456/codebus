import matter from 'gray-matter'
import type { PageFrontmatter, ParsedPage, PageType, SourceRef } from './types.js'

const REQUIRED_FIELDS: (keyof PageFrontmatter)[] = [
  'title', 'type', 'sources', 'goals', 'created', 'updated', 'related', 'stale'
]

const VALID_TYPES: PageType[] = ['concept', 'entity', 'module', 'process', 'synthesis']

export function parsePage(content: string): ParsedPage {
  const { data, content: body } = matter(content)

  for (const field of REQUIRED_FIELDS) {
    if (!(field in data)) {
      throw new Error(`Missing required field in frontmatter: ${field}`)
    }
  }

  const type = data.type as PageType
  if (!VALID_TYPES.includes(type)) {
    throw new Error(`Invalid page type: ${String(data.type)} (must be one of ${VALID_TYPES.join('|')})`)
  }

  return {
    frontmatter: {
      title: String(data.title),
      type,
      sources: normalizeSources(data.sources),
      goals: Array.isArray(data.goals) ? data.goals.map(String) : [],
      created: String(data.created),
      updated: String(data.updated),
      related: Array.isArray(data.related) ? data.related.map(String) : [],
      stale: data.stale === true
    },
    body
  }
}

function normalizeSources(raw: unknown): SourceRef[] {
  if (!Array.isArray(raw)) return []
  return raw
    .filter((s): s is Record<string, unknown> => typeof s === 'object' && s !== null)
    .map((s) => {
      const src: SourceRef = { path: String(s.path) }
      if (typeof s.sha256 === 'string' && s.sha256) src.sha256 = s.sha256
      if (typeof s.at_commit === 'string' && s.at_commit) src.at_commit = s.at_commit
      return src
    })
}

export function serializePage(frontmatter: PageFrontmatter, body: string): string {
  return matter.stringify(body, frontmatter as unknown as Record<string, unknown>)
}
