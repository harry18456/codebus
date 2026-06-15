## Why

The claude path now has vault-root containment for out-of-vault reads, but sensitive basenames inside the vault still have a Read versus Glob/Grep asymmetry. `Read` is blocked by the hook basename backstop, while Glob/Grep skip the denylist branch and can expose in-vault `*.pem`, `*.key`, and `*id_rsa*` content. Fresh vault `.claude/settings.json` also lacks Claude Code `permissions.deny`, so this in-vault secret boundary has no tool-layer backstop and no lint-detectable contract.

## What Changes

- Fresh vault materialized `.claude/settings.json` gains `permissions.deny` entries for sensitive basename Read rules using forward-slash gitignore globs. The effective basename coverage is `*.pem`, `*.key`, and `*id_rsa*`; the Claude Code rule strings use bracket classes such as `Read(**/*.[pP][eE][mM])` so Glob/Grep are covered case-insensitively in practice.
- The sensitive basename deny list becomes a single source that feeds both the materialized `permissions.deny` rules and the existing `check-read` basename backstop. The hook backstop remains defense-in-depth for Read.
- `vault-gate-integrity` expands from hook-only verification to hook-plus-deny verification. It reports an error when the settings file is missing the required sensitive basename deny rules, when `permissions.deny` is missing or empty, or when a required rule is altered.
- The lint rule remains detection-only and preserves the existing lint read-only invariant. No automatic migration or rewrite of existing `.claude/settings.json` is introduced by lint.
- Tests cover fresh settings JSON content, lint detection for removed deny rules, Claude Code case behavior, and hook/list drift prevention.

## Non-Goals

- Do not add home-prefix deny entries such as `~/.ssh/**`; vault containment and the existing hook fallback remain responsible for out-of-vault paths.
- Do not solve cross-session persistence when a secret is embedded inside non-sensitive filenames such as `.yaml` or `.env` that are later searched from the vault.
- Do not change containment allowlists, vault-root containment behavior, PII mirror behavior, or SEC-4 scope.
- Do not auto-migrate existing customized `.claude/settings.json`; detection is handled by lint and remediation can be explicit.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `lint-feedback-loop`: strengthen the vault-internal Claude settings and `vault-gate-integrity` contract so sensitive basename deny rules protect Read, Glob, and Grep and are lint-detectable.

## Impact

- Affected specs: `lint-feedback-loop`
- Affected code:
  - Modified: `codebus-core/src/vault/settings.rs`
  - Modified: `codebus-core/src/wiki/lint/rules/vault_gate_integrity.rs`
  - Modified: `codebus-cli/src/commands/hook.rs`
  - Modified: `codebus-cli/tests/hook_check_read.rs`
  - Modified: `codebus-cli/tests/lint_flow.rs`
  - Modified: `codebus-core/tests/vault_init.rs`
  - Removed: none
