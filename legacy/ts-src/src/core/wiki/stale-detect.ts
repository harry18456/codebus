import type { PageFrontmatter } from './types.js'

export interface StaleResult {
  isStale: boolean
  changedSources: string[]
}

export function detectStaleSources(
  fm: PageFrontmatter,
  currentHashes: Map<string, string>
): StaleResult {
  const changed: string[] = []
  for (const src of fm.sources) {
    const current = currentHashes.get(src.path)
    if (current === undefined || current !== src.sha256) {
      changed.push(src.path)
    }
  }
  return {
    isStale: changed.length > 0,
    changedSources: changed
  }
}
