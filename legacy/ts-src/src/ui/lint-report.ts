import chalk from 'chalk'
import type { LintResult, LintIssue } from '../core/wiki/lint.js'
import type { RenderOptions } from './render.js'

// Print full lint report (used by `codebus --check`). Goal flow uses the
// shorter `formatLintSummary` helper instead — full report is too noisy
// during ingest where user already sees the agent's tool calls.
export function printLintReport(result: LintResult, opts: RenderOptions): void {
  const coverage = formatCoverage(result)
  if (result.issues.length === 0) {
    const lead = opts.useEmoji ? '✅' : 'ok'
    console.log(`${lead} ${coverage}, no issues`)
    return
  }

  // Group issues by file path for compact output
  const byPath = new Map<string, LintIssue[]>()
  for (const issue of result.issues) {
    if (!byPath.has(issue.path)) byPath.set(issue.path, [])
    byPath.get(issue.path)!.push(issue)
  }

  const headLead = opts.useEmoji ? '🔍' : '#'
  const errorMark = opts.useEmoji ? '✗' : 'x'
  const warnMark = opts.useEmoji ? '⚠' : '!'
  console.log(
    `${headLead} ${coverage}, ` +
    `${result.errorCount} error(s), ${result.warnCount} warning(s)`
  )
  console.log('')

  for (const [path, list] of byPath) {
    const hasError = list.some((i) => i.severity === 'error')
    const lead = hasError ? errorMark : warnMark
    const colored = opts.useColor
      ? (hasError ? chalk.red(lead) : chalk.yellow(lead))
      : lead
    console.log(`${colored} wiki/${path}`)
    for (const issue of list) {
      const sevTag = issue.severity === 'error'
        ? (opts.useColor ? chalk.red('error:') : 'error:')
        : (opts.useColor ? chalk.yellow('warn: ') : 'warn: ')
      console.log(`   ${sevTag} ${issue.message}`)
    }
  }
}

// "{N} pages + {M} nav files scanned" — covers both knowledge-page and
// nav-file lint coverage so a clean run doesn't claim "no issues" for files
// it never opened.
function formatCoverage(result: LintResult): string {
  return `${result.pagesScanned} page${result.pagesScanned === 1 ? '' : 's'} + ` +
    `${result.navFilesScanned} nav file${result.navFilesScanned === 1 ? '' : 's'} scanned`
}

// One-line summary suitable for goal flow's banner sequence.
// Returns empty string when no issues.
export function formatLintSummary(result: LintResult, opts: RenderOptions): string {
  if (result.issues.length === 0) return ''
  const mark = opts.useEmoji ? '⚠' : '!'
  const errors = result.errorCount
  const warns = result.warnCount
  const parts: string[] = []
  if (errors > 0) parts.push(`${errors} error${errors > 1 ? 's' : ''}`)
  if (warns > 0) parts.push(`${warns} warning${warns > 1 ? 's' : ''}`)
  return `${mark} lint: ${parts.join(', ')} — codebus --check 看詳情`
}
