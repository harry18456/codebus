const WIKILINK_LIST_LINE = /^(\s*[A-Za-z_][\w-]*\s*:\s*)(\[\[[^\]]+\]\](?:\s*,\s*\[\[[^\]]+\]\])*)\s*$/

export function repairWikilinkList(text: string): string {
  return text
    .split('\n')
    .map((line) => {
      const m = line.match(WIKILINK_LIST_LINE)
      if (!m) return line
      const prefix = m[1]
      const items = m[2]
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean)
        .map((s) => `"${s}"`)
        .join(', ')
      return `${prefix}[${items}]`
    })
    .join('\n')
}
