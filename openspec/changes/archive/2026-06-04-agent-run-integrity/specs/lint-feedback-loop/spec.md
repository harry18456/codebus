## ADDED Requirements

### Requirement: Vault Gate Integrity Check

The lint subsystem SHALL verify that the vault PreToolUse gate configuration at `<vault-root>/.claude/settings.json` still installs the two hooks codebus relies on to sandbox the claude-path agent: a `Bash` matcher routing to `codebus hook check-bash` AND a `Read` matcher routing to `codebus hook check-read`. This check SHALL read exactly that single file; it SHALL NOT scan, traverse, or read any other path outside the `wiki/` subtree, and SHALL NOT broaden lint into a general vault-structure validator. The check is a detection signal only — it SHALL NOT modify, restore, or rewrite the settings file (the Lint Read-Only Invariant continues to hold).

The required hook set (the matcher → command pairs `Bash` → `codebus hook check-bash` and `Read` → `codebus hook check-read`) SHALL be sourced from the same definition that `codebus init` uses to author the default settings file, so the linter and the installer cannot drift.

The check SHALL emit a lint issue with `severity: error` and the stable kebab-case rule identifier `vault-gate-integrity` when ANY of the following holds: the settings file is absent; the file does not parse as JSON; `hooks.PreToolUse` is missing or is not an array; the `Bash` → `codebus hook check-bash` hook entry is absent; or the `Read` → `codebus hook check-read` hook entry is absent. The issue `message` SHALL identify which condition failed (which required hook is missing, or that the file is absent / unparseable). When BOTH required hook entries are present, the check SHALL emit NO `vault-gate-integrity` issue, regardless of any additional user-added matcher entries, hook commands, or top-level keys present in the file (preserving the write-if-missing user-customization contract).

The issue path for a `vault-gate-integrity` finding SHALL be the settings file location: in `text` format it SHALL render as the vault-relative path `.claude/settings.json` verbatim, WITHOUT the `wiki/` prefix that the text format applies to wiki-subtree issue paths; in `json` format the issue `path` SHALL be the absolute filesystem path of the settings file. This finding SHALL be counted in the `error_count` totals like any other error-severity issue.

#### Scenario: Intact gate produces no issue

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains both the `Bash` → `codebus hook check-bash` and `Read` → `codebus hook check-read` PreToolUse hook entries
- **THEN** the lint result SHALL NOT contain any issue whose `rule` is `vault-gate-integrity`

#### Scenario: Emptied PreToolUse array is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` parses as JSON but whose `hooks.PreToolUse` array has been rewritten to empty
- **THEN** the lint result SHALL contain one `error`-severity issue whose `rule` is `vault-gate-integrity` per missing required hook — i.e., two such issues when both the `Bash` and `Read` gates are absent — and each issue `message` SHALL identify the specific missing gate (consistent with the per-hook reporting in the Missing-Bash and Missing-Read scenarios)

#### Scenario: Missing Bash gate hook is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Read` → `codebus hook check-read` entry but not the `Bash` → `codebus hook check-bash` entry
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue whose `message` identifies the missing `Bash` check-bash gate

#### Scenario: Missing Read gate hook is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Bash` → `codebus hook check-bash` entry but not the `Read` → `codebus hook check-read` entry
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue whose `message` identifies the missing `Read` check-read gate

#### Scenario: User-added settings do not cause a false positive

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` retains both required hook entries AND also contains additional user-added PreToolUse entries or unrelated top-level keys
- **THEN** the lint result SHALL NOT contain any `vault-gate-integrity` issue

#### Scenario: Absent or unparseable settings file is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` is absent, OR whose content does not parse as JSON
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue

#### Scenario: Gate finding path representation per format

- **WHEN** a `vault-gate-integrity` issue is emitted for a vault rooted at `<abs-vault>/`
- **THEN** in `text` format the issue path SHALL render as `.claude/settings.json` with no `wiki/` prefix AND in `json` format the issue `path` SHALL equal `<abs-vault>/.claude/settings.json` (absolute)

#### Scenario: Gate check never modifies the vault

- **WHEN** the system runs lint against any vault, whether or not the gate is intact
- **THEN** `<vault-root>/.claude/settings.json` SHALL be byte-identical before and after the lint invocation
