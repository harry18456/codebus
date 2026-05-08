import { execSync as defaultExecSync } from 'node:child_process'

export type ExecSyncFn = (
  cmd: string,
  opts?: { stdio?: 'pipe' | 'inherit' | 'ignore'; timeout?: number }
) => Buffer | string

export interface CliCheckResult {
  ok: boolean
  reason?: string
  hint?: string
}

// Verify Anthropic Claude Code CLI is installed and reachable on PATH
// before any codebus flow that depends on `claude -p`. Lets us fail fast
// with an actionable hint instead of waiting for spawn to error mid-flow
// (which surfaces deep inside ClaudeCliProvider.invoke and is harder to
// trace for first-time users).
//
// `exec` parameter is injectable for testing — defaults to node's execSync.
export function checkClaudeCliAvailable(exec: ExecSyncFn = defaultExecSync): CliCheckResult {
  let raw: string
  try {
    raw = exec('claude --version', { stdio: 'pipe', timeout: 5000 }).toString().trim()
  } catch {
    return {
      ok: false,
      reason: '找不到 claude CLI（claude --version 執行失敗）',
      hint: '請安裝 Anthropic Claude Code CLI：npm install -g @anthropic-ai/claude-code'
    }
  }
  // Expected format like "2.1.126 (Claude Code)" or "2.1.126".
  // Be lenient — first token must look like a semver-ish version.
  if (!/^\d+\.\d+(\.\d+)?/.test(raw)) {
    return {
      ok: false,
      reason: `claude --version 回傳格式預期外：${raw}`,
      hint: '可能 claude CLI 版本不相容，升級到 2.x：npm install -g @anthropic-ai/claude-code'
    }
  }
  return { ok: true }
}
