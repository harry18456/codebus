## Context

Current anchors rechecked in this repo:

- `codebus-cli/src/commands/hook.rs` `check_read` evaluates vault containment first, then the image/sensitive denylist. The second stage is wrapped in `if !is_search_tool(&buf)` at lines 554-562, and `is_search_tool` returns true for `Glob` and `Grep` at lines 735-744. This makes the basename backstop Read-scoped.
- `codebus-cli/src/commands/hook.rs` `check_sensitive_path` blocks basenames matching `*id_rsa*`, `*.pem`, or `*.key` at lines 417-426, with the matcher implemented at lines 459-469.
- `codebus-core/src/vault/settings.rs` `DEFAULT_SETTINGS_JSON` at lines 74-116 contains only `hooks.PreToolUse`; it has no top-level `permissions` object and no `permissions.deny` array.
- `codebus-core/src/wiki/lint/rules/vault_gate_integrity.rs` currently validates the `hooks.PreToolUse` array and `REQUIRED_HOOKS` at lines 66-87. It does not inspect `permissions.deny`.

Claude Code behavior is grounded by official docs and a local probe on Claude Code 2.1.177:

- Official docs: permission rules are evaluated deny, then ask, then allow; rule specificity does not override that order. `Read` rules are applied as a best-effort to built-in read tools including Grep and Glob. `Read`/`Edit` path rules follow gitignore pattern semantics. On Windows, paths are normalized to POSIX form before matching, so required patterns must use forward slash syntax. Source: https://code.claude.com/docs/en/permissions, retrieved 2026-06-16.
- Official docs: PreToolUse hook decisions do not bypass permission rules; matching deny rules still block even when a hook allows. Source: https://code.claude.com/docs/en/permissions, retrieved 2026-06-16.
- Official CLI docs: `--allowedTools` grants tools that execute without prompting, while `--disallowedTools` maps to deny rules. Source: https://code.claude.com/docs/en/cli-reference, retrieved 2026-06-16.
- Local probe: in a temporary project with `permissions.deny` containing `Read(**/*.pem)`, `Read(**/*.key)`, and `Read(**/*id_rsa*)`, Grep over a directory containing `public.txt` and `secret.pem` returned only `public.txt`, both without `--allowedTools` and with `--allowedTools Grep`. Glob for `**/*.pem` returned no files. Therefore `--allowedTools` did not override deny, and Claude Code performed result-level exclusion for Grep/Glob in this case.
- Local probe: the lowercase-only `Read(**/*.pem)` rule did not block `secret.PEM` from Grep results. Bracket-class rules `Read(**/*.[pP][eE][mM])`, `Read(**/*.[kK][eE][yY])`, and `Read(**/*[iI][dD]_[rR][sS][aA]*)` blocked uppercase `.PEM`, uppercase `.KEY`, and uppercase `ID_RSA` variants, returning only `public.txt`.

## Goals / Non-Goals

**Goals:**

- Close the in-vault sensitive basename gap for Read, Glob, and Grep.
- Add the boundary at the Claude Code permission layer so Grep/Glob result lists are scrubbed before the model sees denied file contents or filenames.
- Make required sensitive basename deny rules lint-detectable through `vault-gate-integrity`.
- Keep a single source of truth for sensitive basename rules used by settings generation, lint verification, and the Read hook defense-in-depth matcher.
- Preserve existing hook matcher behavior and the lint read-only invariant.

**Non-Goals:**

- No home-prefix deny entries such as `~/.ssh/**` in `permissions.deny`.
- No automatic migration or rewrite of existing customized `.claude/settings.json` by init or lint.
- No change to vault-root containment, containment allowlists, PII mirror behavior, or SEC-4 scope.
- No attempt to prevent a secret embedded inside a non-sensitive filename from being found in a later session.
- No OS-level sandbox guarantee for arbitrary subprocesses that read files indirectly.

## Decisions

### Use Claude Code Read deny rules as the cross-tool in-vault boundary

Add a top-level `permissions.deny` array to fresh vault `.claude/settings.json`. The required rules are:

```json
[
  "Read(**/*.[pP][eE][mM])",
  "Read(**/*.[kK][eE][yY])",
  "Read(**/*[iI][dD]_[rR][sS][aA]*)"
]
```

The semantic basename set remains `*.pem`, `*.key`, and `*id_rsa*`, but the stored Claude Code rules use bracket classes because the local probe showed lowercase-only patterns leave uppercase basenames visible to Grep. All rules use forward slashes because Claude Code normalizes Windows paths to POSIX form before matching.

Rejected alternative: remove `is_search_tool` and run the hook denylist for Glob/Grep. The hook receives the search root path for Glob/Grep, not each matched file path, and an omitted `tool_input.path` is a valid implicit-vault-root search. The hook cannot perform result-level Grep/Glob scrubbing without reimplementing the tools.

Rejected alternative: add only lowercase rules such as `Read(**/*.pem)`. The local probe showed that this leaves `secret.PEM` visible to Grep.

### Define sensitive basename rules in codebus-core single source

Add one exported rule set in `codebus-core`, colocated with vault settings, that carries both the matching semantics and the Claude Code deny rule string. The shape can be implemented as a small struct plus an enum such as:

```rust
pub enum SensitiveBasenameMatcher {
    SuffixAsciiCaseInsensitive(&'static str),
    ContainsAsciiCaseInsensitive(&'static str),
}

pub struct SensitiveBasenameRule {
    pub matcher: SensitiveBasenameMatcher,
    pub claude_read_rule: &'static str,
}
```

The rule set contains exactly three entries: suffix `.pem`, suffix `.key`, and contains `id_rsa`. `codebus-cli` already depends on `codebus-core`, so `hook.rs` can replace its local `matches_sensitive_basename_glob` logic with a call into the shared matcher without creating a dependency cycle.

`DEFAULT_SETTINGS_JSON` must not carry a second hand-maintained deny list. Keep the exported name only if it is generated from `REQUIRED_HOOKS` and the sensitive basename rule set, for example with `LazyLock<String>` plus a private builder. `write_settings_if_missing` writes the generated value. Tests parse the generated JSON and assert exact agreement with both `REQUIRED_HOOKS` and the sensitive basename rule set.

### Extend vault-gate-integrity to verify required deny rules

`vault-gate-integrity` keeps reading exactly `<vault-root>/.claude/settings.json` and remains detection-only. After JSON parsing, it validates both:

- `hooks.PreToolUse` contains every `REQUIRED_HOOKS` matcher-command pair.
- `permissions.deny` is an array containing every required sensitive basename `claude_read_rule`.

If `permissions` is absent, `permissions.deny` is absent, `permissions.deny` is not an array, or an individual rule is missing or altered, the rule emits `severity: error`, `rule_id: vault-gate-integrity`, and `path: .claude/settings.json`. Missing/non-array deny state is treated as all sensitive basename deny rules missing. Extra user-added deny rules, extra hooks, and unrelated top-level keys do not trigger an issue.

Messages must name the missing deny rule string so the fix loop has enough information to restore the settings file from lint output. The lint rule itself does not modify the file.

### Preserve write-if-missing settings behavior

`write_settings_if_missing` keeps byte-identical preservation for an existing `.claude/settings.json`. Fresh vaults receive the new generated settings. Existing vaults with old settings become visible through `vault-gate-integrity`; they are not silently rewritten by init or lint.

This preserves the established user-customization contract and keeps migration explicit.

## Implementation Contract

Fresh vault settings contract:

- `codebus init` writes `<vault-root>/.claude/settings.json` only when absent.
- The generated JSON contains top-level `hooks.PreToolUse` with the existing Bash, Read, Glob, and Grep matcher entries.
- The generated JSON contains top-level `permissions.deny` with exactly the required sensitive basename Read deny rules, plus no backslash-based required rules.
- Existing settings files remain byte-identical across init reruns.

Runtime read-boundary contract:

- Claude Code denies Read access to in-vault basenames matching `.pem`, `.key`, or `id_rsa` case-insensitively through the required `Read(...)` deny rules.
- Grep over a directory omits denied matching files from tool results. Glob does not return denied matching files. `--allowedTools` does not override these deny rules.
- `codebus hook check-read` retains its Read basename backstop and uses the same sensitive basename rule set for its ASCII case-insensitive basename match.
- Glob/Grep still skip the hook denylist stage after containment; that remains acceptable because the permission layer is the cross-tool result boundary.

Lint contract:

- `vault-gate-integrity` reports an error for missing, non-array, empty, or altered required sensitive basename deny rules.
- A settings file with all required hooks and all required deny rules produces no `vault-gate-integrity` issue, even when it contains extra user settings.
- A `vault-gate-integrity` issue still renders as `.claude/settings.json` in text output and as the absolute settings path in JSON output.
- Lint never writes, restores, or rewrites the settings file.

Acceptance criteria:

- Unit tests in `codebus-core/src/vault/settings.rs` prove generated settings parse as JSON and match `REQUIRED_HOOKS` plus the sensitive basename rule set exactly.
- Unit tests in `codebus-core/src/wiki/lint/rules/vault_gate_integrity.rs` cover intact settings, missing `permissions`, missing `permissions.deny`, empty deny array, one missing deny rule, and extra user entries.
- CLI lint integration covers a vault whose settings has all hooks but no required deny rules and asserts a `vault-gate-integrity` JSON error naming at least one missing `Read(...)` rule.
- Hook tests cover `.pem`, `.PEM`, `.key`, `.KEY`, `id_rsa`, and `ID_RSA` basename matches through the shared matcher.
- Manual Claude Code probe is recorded in this design; CI tests do not invoke Claude Code or require network credentials.

Scope boundaries:

- In scope: fresh settings generation, shared sensitive basename rule definitions, hook basename backstop refactor, lint validation, tests.
- Out of scope: existing-vault auto-migration, additional deny classes, Bash subprocess sandboxing, raw mirror redaction, source containment behavior.

## Risks / Trade-offs

- [Risk] Claude Code documents Read rules for Grep/Glob as best-effort, not as an OS-level guarantee. -> Mitigation: retain containment and hook backstops, keep Bash constrained by existing gates, and document the 2.1.177 probe as version-grounding rather than a universal filesystem sandbox claim.
- [Risk] Existing vaults remain vulnerable until their settings are updated. -> Mitigation: `vault-gate-integrity` surfaces the missing deny rules as error-severity lint findings, with messages that name the exact rule strings.
- [Risk] Bracket-class deny rules are less readable than lowercase examples. -> Mitigation: store them in a named shared rule set with semantic names and tests that prove they cover uppercase variants.
- [Risk] Replacing a `&str` constant with generated JSON touches tests that import `DEFAULT_SETTINGS_JSON`. -> Mitigation: keep the exported name through `LazyLock<String>` or update internal uses to `default_settings_json()` in one settings module pass.

## Migration Plan

Fresh vaults receive the new settings automatically. Existing vaults are not rewritten by init or lint; running lint reports missing required deny rules. A user or the fix loop can add the exact rules named in the `vault-gate-integrity` messages.

Rollback is a code revert of the settings generation and lint verification additions. Existing user settings that manually added `permissions.deny` remain valid Claude Code settings.

## Open Questions

None.
