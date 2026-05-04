export interface SourceRef {
  path: string
  sha256?: string
  at_commit?: string
}

export type PageType = 'concept' | 'module' | 'process' | 'entity'

export interface PageFrontmatter {
  title: string
  type: PageType
  sources: SourceRef[]
  goals: string[]
  created: string
  updated: string
  related: string[]
  stale: boolean
}

export interface ParsedPage {
  frontmatter: PageFrontmatter
  body: string
}
