import { existsSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join } from 'node:path'
import yaml from 'js-yaml'

// Resolve home dir via env vars first (HOME / USERPROFILE) so test stubs
// take effect. Falls back to os.homedir() (uv-based) when env unset.
function resolveHome(): string {
  return process.env.HOME ?? process.env.USERPROFILE ?? homedir()
}

export interface GlobalConfig {
  emoji?: 'auto' | 'on' | 'off'
}

const VALID_EMOJI = ['auto', 'on', 'off'] as const

function pickKnownFields(parsed: unknown): GlobalConfig {
  const out: GlobalConfig = {}
  if (!parsed || typeof parsed !== 'object') return out
  const data = parsed as Record<string, unknown>
  if ('emoji' in data) {
    const v = data.emoji
    if (typeof v === 'string' && (VALID_EMOJI as readonly string[]).includes(v)) {
      out.emoji = v as GlobalConfig['emoji']
    } else {
      console.warn(
        `codebus: ignoring invalid emoji value '${String(v)}' in ~/.codebus/config.yaml ` +
        `(must be auto|on|off)`
      )
    }
  }
  // Phase-2 fields (default_provider / api_keys / token_usage_log) are
  // silently ignored so users can pre-fill them without warnings.
  return out
}

export async function loadGlobalConfig(): Promise<GlobalConfig> {
  const path = join(resolveHome(), '.codebus', 'config.yaml')
  if (!existsSync(path)) return {}
  let raw: string
  try {
    raw = await readFile(path, 'utf8')
  } catch {
    return {}
  }
  let parsed: unknown
  try {
    parsed = yaml.load(raw)
  } catch (err) {
    console.warn(
      `codebus: failed to parse ~/.codebus/config.yaml — using defaults ` +
      `(${(err as Error).message})`
    )
    return {}
  }
  return pickKnownFields(parsed)
}
