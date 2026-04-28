// Browser-safe markdown frontmatter parser. Replaces `gray-matter`,
// which depends on Node's global `Buffer` and throws
// "Buffer is not defined" inside the Tauri WebView / Nuxt SPA bundle.
//
// gray-matter under the hood uses js-yaml; this helper does the same
// without the buffer-touching plumbing. Format mirrors gray-matter's
// `{ data, content }` so callers can swap with a single import flip.

import yaml from 'js-yaml'

export interface FrontmatterResult {
  data: Record<string, unknown>
  content: string
}

const FRONTMATTER_RE = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/

export function parseFrontmatter(raw: string): FrontmatterResult {
  const match = raw.match(FRONTMATTER_RE)
  if (!match) {
    return { data: {}, content: raw }
  }
  const yamlBlock = match[1] ?? ''
  const content = match[2] ?? ''
  let data: Record<string, unknown> = {}
  try {
    const parsed = yaml.load(yamlBlock)
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      data = parsed as Record<string, unknown>
    }
  } catch (err) {
    // eslint-disable-next-line no-console
    console.warn('parseFrontmatter: yaml.load failed', err)
  }
  return { data, content }
}
