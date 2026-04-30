import { execSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'
import { describe, it } from 'vitest'

// Defensive baseline test for change fix-phase7-typecheck-baseline.
//
// Spec: openspec/specs/frontend-shell/spec.md
//   "Frontend typecheck baseline stays at zero errors"
//
// Spawns `vue-tsc --noEmit -p .` from inside the test process. Future
// regressions that escape `npm run typecheck` audits get caught at the
// standard `npm run test` (vitest run) gate. The failure message echoes
// the entire vue-tsc stdout/stderr so the developer can locate the
// regression from the vitest output alone.

const TS_ERROR_PATTERN = /error TS\d+:/
const webRoot = resolve(fileURLToPath(import.meta.url), '../../..')

describe('frontend-shell typecheck baseline', () => {
  it('vue-tsc --noEmit -p . exits 0 and emits no error TS diagnostics', () => {
    let exitCode = 0
    let output = ''
    try {
      // `--build` is required: `web/tsconfig.json` is project-references-only
      // (`files: []` + 4 references). Without `--build`, vue-tsc / tsc does
      // not traverse the references and silently checks zero files. The flag
      // matches what `nuxt typecheck` runs internally.
      output = execSync('npx vue-tsc --build --noEmit 2>&1', {
        cwd: webRoot,
        encoding: 'utf-8'
      })
    } catch (e) {
      const err = e as {
        status?: number | null
        stdout?: string | Buffer
        stderr?: string | Buffer
      }
      exitCode = typeof err.status === 'number' ? err.status : 1
      const out =
        typeof err.stdout === 'string'
          ? err.stdout
          : (err.stdout?.toString('utf-8') ?? '')
      const errOut =
        typeof err.stderr === 'string'
          ? err.stderr
          : (err.stderr?.toString('utf-8') ?? '')
      output = `${out}\n${errOut}`
    }

    if (exitCode !== 0 || TS_ERROR_PATTERN.test(output)) {
      throw new Error(
        `vue-tsc baseline regression. exit=${exitCode}\n----\n${output}\n----`
      )
    }
  }, 180_000)
})
