export interface SourceRef {
  path: string
  sha256?: string
  at_commit?: string
}

export type PageType = 'concept' | 'entity' | 'module' | 'process' | 'synthesis'

// Single source of truth: page type ↔ wiki/ subfolder name. Karpathy-style
// 5-bucket structure (concepts / entities / modules / processes / synthesis).
// folder names are pluralised; 'synthesis' has no plural form so the type
// and folder name happen to match.
export const PAGE_TYPE_FOLDERS: Record<PageType, string> = {
  concept: 'concepts',
  entity: 'entities',
  module: 'modules',
  process: 'processes',
  synthesis: 'synthesis'
}

export const PAGE_TYPE_FROM_FOLDER: Record<string, PageType> = {
  concepts: 'concept',
  entities: 'entity',
  modules: 'module',
  processes: 'process',
  synthesis: 'synthesis'
}

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
