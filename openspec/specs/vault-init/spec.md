# vault-init Specification

## Purpose

Initialize and maintain the `.codebus/` vault structure under a user repo, including `raw/code/`, `wiki/` (5-folder Karpathy-style knowledge structure), `output/`, a nested `.git/`, the built-in `CLAUDE.md` schema, source repo `.gitignore` integration, and a file-based lock to prevent concurrent vault mutation.

## Requirements

### Requirement: Initialize .codebus/ vault structure under user repo

When invoked with `--repo <path>` and no `--goal` or `--query`, the system SHALL create the `.codebus/` vault under the given path containing all subdirectories required by the wiki workflow.

#### Scenario: Fresh init creates all expected paths

- **WHEN** the user runs `codebus --repo /path/to/myrepo` and `/path/to/myrepo/.codebus/` does not yet exist
- **THEN** the system creates `/path/to/myrepo/.codebus/` with subdirectories `raw/`, `raw/code/`, `wiki/`, `wiki/concepts/`, `wiki/entities/`, `wiki/modules/`, `wiki/processes/`, `wiki/synthesis/`, `output/`, and files `CLAUDE.md`, `goals.jsonl`, `.gitignore` (Karpathy-style 5-folder knowledge structure; folder = page `type` enum). The system SHALL NOT create `wiki/goals/`.

#### Scenario: Internal .gitignore excludes lock and raw code

- **WHEN** init writes `.codebus/.gitignore`
- **THEN** the file contains entries for `.lock` and `raw/code/` so that lock files and the codebase mirror are not tracked by the nested git repo


<!-- @trace
source: codebus-v2-phase1
updated: 2026-05-05
code:
  - src/infra/fs/raw-sync.ts
  - docs/superpowers/REVIEW_LESSONS.md
  - src/infra/cli-detect.ts
  - src/core/wiki/types.ts
  - README.md
  - src/core/wiki/frontmatter.ts
  - package.json
  - src/core/vault/lock.ts
  - src/core/vault/sanity-check.ts
  - src/core/wiki/stale-detect.ts
  - src/schema/claude-md.ts
  - src/commands/goal.ts
  - src/core/wiki/date.ts
  - LICENSE
  - src/cli.ts
  - tsconfig.json
  - src/commands/query.ts
  - src/infra/git/source-version.ts
  - src/ui/lint-report.ts
  - docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md
  - src/infra/llm/types.ts
  - .spectra.yaml
  - src/core/vault/layout.ts
  - src/infra/git/nested-repo.ts
  - src/commands/check.ts
  - src/core/wiki/page-merge.ts
  - vitest.config.ts
  - src/commands/init.ts
  - src/ui/stream-parser.ts
  - src/core/wiki/lint.ts
  - src/infra/llm/claude-cli.ts
  - src/infra/fs/file-ops.ts
  - src/ui/emoji-mode.ts
  - src/infra/global-config.ts
  - src/core/wiki/frontmatter-repair.ts
  - src/ui/render.ts
tests:
  - tests/e2e/init-smoke.test.ts
  - tests/infra/fs/file-ops.test.ts
  - tests/commands/goal.test.ts
  - tests/cli.test.ts
  - tests/commands/query.test.ts
  - tests/commands/check.test.ts
  - tests/core/wiki/date.test.ts
  - tests/core/wiki/page-merge.test.ts
  - tests/core/wiki/stale-detect.test.ts
  - tests/infra/cli-detect.test.ts
  - tests/ui/emoji-mode.test.ts
  - tests/core/wiki/frontmatter-repair.test.ts
  - tests/infra/git/source-version.test.ts
  - tests/commands/init.test.ts
  - tests/infra/global-config.test.ts
  - tests/ui/stream-parser.test.ts
  - tests/core/vault/sanity-check.test.ts
  - tests/core/wiki/lint.test.ts
  - tests/infra/git/nested-repo.test.ts
  - tests/infra/fs/raw-sync.test.ts
  - tests/core/vault/layout.test.ts
  - tests/core/vault/lock.test.ts
  - tests/infra/llm/claude-cli.test.ts
  - tests/schema/claude-md.test.ts
  - tests/core/wiki/frontmatter.test.ts
  - tests/ui/render.test.ts
-->

---
### Requirement: Install built-in CLAUDE.md schema

The system SHALL write a built-in CLAUDE.md schema to `.codebus/CLAUDE.md` during init, and SHALL NOT overwrite an existing CLAUDE.md (to preserve user customizations).

#### Scenario: First init writes schema

- **WHEN** init runs and `.codebus/CLAUDE.md` does not exist
- **THEN** the system writes the built-in schema content (12 sections covering role, layout, workflow, frontmatter, etc.) with an SPDX-License-Identifier header

#### Scenario: Re-init preserves user-modified schema

- **WHEN** `.codebus/CLAUDE.md` already exists with user modifications
- **AND** init runs again
- **THEN** the system leaves the existing file unchanged


<!-- @trace
source: codebus-v2-phase1
updated: 2026-05-04
code:
  - src/infra/fs/raw-sync.ts
  - docs/superpowers/REVIEW_LESSONS.md
  - src/infra/cli-detect.ts
  - src/core/wiki/types.ts
  - README.md
  - src/core/wiki/frontmatter.ts
  - package.json
  - src/core/vault/lock.ts
  - src/core/vault/sanity-check.ts
  - src/core/wiki/stale-detect.ts
  - src/schema/claude-md.ts
  - src/commands/goal.ts
  - src/core/wiki/date.ts
  - LICENSE
  - src/cli.ts
  - tsconfig.json
  - src/commands/query.ts
  - src/infra/git/source-version.ts
  - src/ui/lint-report.ts
  - docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md
  - src/infra/llm/types.ts
  - .spectra.yaml
  - src/core/vault/layout.ts
  - src/infra/git/nested-repo.ts
  - src/commands/check.ts
  - src/core/wiki/page-merge.ts
  - vitest.config.ts
  - src/commands/init.ts
  - src/ui/stream-parser.ts
  - src/core/wiki/lint.ts
  - src/infra/llm/claude-cli.ts
  - src/infra/fs/file-ops.ts
  - src/ui/emoji-mode.ts
  - src/infra/global-config.ts
  - src/core/wiki/frontmatter-repair.ts
  - src/ui/render.ts
tests:
  - tests/e2e/init-smoke.test.ts
  - tests/infra/fs/file-ops.test.ts
  - tests/commands/goal.test.ts
  - tests/cli.test.ts
  - tests/commands/query.test.ts
  - tests/commands/check.test.ts
  - tests/core/wiki/date.test.ts
  - tests/core/wiki/page-merge.test.ts
  - tests/core/wiki/stale-detect.test.ts
  - tests/infra/cli-detect.test.ts
  - tests/ui/emoji-mode.test.ts
  - tests/core/wiki/frontmatter-repair.test.ts
  - tests/infra/git/source-version.test.ts
  - tests/commands/init.test.ts
  - tests/infra/global-config.test.ts
  - tests/ui/stream-parser.test.ts
  - tests/core/vault/sanity-check.test.ts
  - tests/core/wiki/lint.test.ts
  - tests/infra/git/nested-repo.test.ts
  - tests/infra/fs/raw-sync.test.ts
  - tests/core/vault/layout.test.ts
  - tests/core/vault/lock.test.ts
  - tests/infra/llm/claude-cli.test.ts
  - tests/schema/claude-md.test.ts
  - tests/core/wiki/frontmatter.test.ts
  - tests/ui/render.test.ts
-->

---
### Requirement: Initialize nested git repository at .codebus/.git

The system SHALL initialize a nested git repo at `.codebus/.git` during init so wiki revisions are versioned independently of the source repo.

#### Scenario: Nested git is created with initial commit

- **WHEN** init runs and `.codebus/.git` does not exist
- **THEN** the system runs `git init -b main` inside `.codebus/`, configures a codebus identity, and produces an initial commit containing the schema and structure

#### Scenario: Re-init does not re-initialize existing nested git

- **WHEN** `.codebus/.git` already exists
- **AND** init runs again
- **THEN** the system leaves the existing `.git` directory untouched


<!-- @trace
source: codebus-v2-phase1
updated: 2026-05-04
code:
  - src/infra/fs/raw-sync.ts
  - docs/superpowers/REVIEW_LESSONS.md
  - src/infra/cli-detect.ts
  - src/core/wiki/types.ts
  - README.md
  - src/core/wiki/frontmatter.ts
  - package.json
  - src/core/vault/lock.ts
  - src/core/vault/sanity-check.ts
  - src/core/wiki/stale-detect.ts
  - src/schema/claude-md.ts
  - src/commands/goal.ts
  - src/core/wiki/date.ts
  - LICENSE
  - src/cli.ts
  - tsconfig.json
  - src/commands/query.ts
  - src/infra/git/source-version.ts
  - src/ui/lint-report.ts
  - docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md
  - src/infra/llm/types.ts
  - .spectra.yaml
  - src/core/vault/layout.ts
  - src/infra/git/nested-repo.ts
  - src/commands/check.ts
  - src/core/wiki/page-merge.ts
  - vitest.config.ts
  - src/commands/init.ts
  - src/ui/stream-parser.ts
  - src/core/wiki/lint.ts
  - src/infra/llm/claude-cli.ts
  - src/infra/fs/file-ops.ts
  - src/ui/emoji-mode.ts
  - src/infra/global-config.ts
  - src/core/wiki/frontmatter-repair.ts
  - src/ui/render.ts
tests:
  - tests/e2e/init-smoke.test.ts
  - tests/infra/fs/file-ops.test.ts
  - tests/commands/goal.test.ts
  - tests/cli.test.ts
  - tests/commands/query.test.ts
  - tests/commands/check.test.ts
  - tests/core/wiki/date.test.ts
  - tests/core/wiki/page-merge.test.ts
  - tests/core/wiki/stale-detect.test.ts
  - tests/infra/cli-detect.test.ts
  - tests/ui/emoji-mode.test.ts
  - tests/core/wiki/frontmatter-repair.test.ts
  - tests/infra/git/source-version.test.ts
  - tests/commands/init.test.ts
  - tests/infra/global-config.test.ts
  - tests/ui/stream-parser.test.ts
  - tests/core/vault/sanity-check.test.ts
  - tests/core/wiki/lint.test.ts
  - tests/infra/git/nested-repo.test.ts
  - tests/infra/fs/raw-sync.test.ts
  - tests/core/vault/layout.test.ts
  - tests/core/vault/lock.test.ts
  - tests/infra/llm/claude-cli.test.ts
  - tests/schema/claude-md.test.ts
  - tests/core/wiki/frontmatter.test.ts
  - tests/ui/render.test.ts
-->

---
### Requirement: Add .codebus to source repo .gitignore when source is a git repo

The system SHALL add `.codebus` to the source repo's `.gitignore` if the source is a git repo, creating the file when missing and avoiding duplicate entries.

#### Scenario: Source repo .gitignore is missing

- **WHEN** init runs and source repo `.git/` exists but `.gitignore` does not
- **THEN** the system creates `.gitignore` containing the line `.codebus`

#### Scenario: Source repo .gitignore already contains .codebus

- **WHEN** `.gitignore` already lists `.codebus`
- **THEN** the system does not append a duplicate entry

#### Scenario: Source path is not a git repo

- **WHEN** the source path has no `.git/` directory
- **THEN** the system skips `.gitignore` mutation but still creates the `.codebus/` vault


<!-- @trace
source: codebus-v2-phase1
updated: 2026-05-04
code:
  - src/infra/fs/raw-sync.ts
  - docs/superpowers/REVIEW_LESSONS.md
  - src/infra/cli-detect.ts
  - src/core/wiki/types.ts
  - README.md
  - src/core/wiki/frontmatter.ts
  - package.json
  - src/core/vault/lock.ts
  - src/core/vault/sanity-check.ts
  - src/core/wiki/stale-detect.ts
  - src/schema/claude-md.ts
  - src/commands/goal.ts
  - src/core/wiki/date.ts
  - LICENSE
  - src/cli.ts
  - tsconfig.json
  - src/commands/query.ts
  - src/infra/git/source-version.ts
  - src/ui/lint-report.ts
  - docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md
  - src/infra/llm/types.ts
  - .spectra.yaml
  - src/core/vault/layout.ts
  - src/infra/git/nested-repo.ts
  - src/commands/check.ts
  - src/core/wiki/page-merge.ts
  - vitest.config.ts
  - src/commands/init.ts
  - src/ui/stream-parser.ts
  - src/core/wiki/lint.ts
  - src/infra/llm/claude-cli.ts
  - src/infra/fs/file-ops.ts
  - src/ui/emoji-mode.ts
  - src/infra/global-config.ts
  - src/core/wiki/frontmatter-repair.ts
  - src/ui/render.ts
tests:
  - tests/e2e/init-smoke.test.ts
  - tests/infra/fs/file-ops.test.ts
  - tests/commands/goal.test.ts
  - tests/cli.test.ts
  - tests/commands/query.test.ts
  - tests/commands/check.test.ts
  - tests/core/wiki/date.test.ts
  - tests/core/wiki/page-merge.test.ts
  - tests/core/wiki/stale-detect.test.ts
  - tests/infra/cli-detect.test.ts
  - tests/ui/emoji-mode.test.ts
  - tests/core/wiki/frontmatter-repair.test.ts
  - tests/infra/git/source-version.test.ts
  - tests/commands/init.test.ts
  - tests/infra/global-config.test.ts
  - tests/ui/stream-parser.test.ts
  - tests/core/vault/sanity-check.test.ts
  - tests/core/wiki/lint.test.ts
  - tests/infra/git/nested-repo.test.ts
  - tests/infra/fs/raw-sync.test.ts
  - tests/core/vault/layout.test.ts
  - tests/core/vault/lock.test.ts
  - tests/infra/llm/claude-cli.test.ts
  - tests/schema/claude-md.test.ts
  - tests/core/wiki/frontmatter.test.ts
  - tests/ui/render.test.ts
-->

---
### Requirement: Init is idempotent

Running init twice SHALL produce the same final state without errors.

#### Scenario: Two consecutive inits succeed

- **WHEN** the user runs `codebus --repo X` twice in a row
- **THEN** both invocations succeed, `.codebus/` exists with all expected paths, and no duplicate entries appear in source `.gitignore`


<!-- @trace
source: codebus-v2-phase1
updated: 2026-05-04
code:
  - src/infra/fs/raw-sync.ts
  - docs/superpowers/REVIEW_LESSONS.md
  - src/infra/cli-detect.ts
  - src/core/wiki/types.ts
  - README.md
  - src/core/wiki/frontmatter.ts
  - package.json
  - src/core/vault/lock.ts
  - src/core/vault/sanity-check.ts
  - src/core/wiki/stale-detect.ts
  - src/schema/claude-md.ts
  - src/commands/goal.ts
  - src/core/wiki/date.ts
  - LICENSE
  - src/cli.ts
  - tsconfig.json
  - src/commands/query.ts
  - src/infra/git/source-version.ts
  - src/ui/lint-report.ts
  - docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md
  - src/infra/llm/types.ts
  - .spectra.yaml
  - src/core/vault/layout.ts
  - src/infra/git/nested-repo.ts
  - src/commands/check.ts
  - src/core/wiki/page-merge.ts
  - vitest.config.ts
  - src/commands/init.ts
  - src/ui/stream-parser.ts
  - src/core/wiki/lint.ts
  - src/infra/llm/claude-cli.ts
  - src/infra/fs/file-ops.ts
  - src/ui/emoji-mode.ts
  - src/infra/global-config.ts
  - src/core/wiki/frontmatter-repair.ts
  - src/ui/render.ts
tests:
  - tests/e2e/init-smoke.test.ts
  - tests/infra/fs/file-ops.test.ts
  - tests/commands/goal.test.ts
  - tests/cli.test.ts
  - tests/commands/query.test.ts
  - tests/commands/check.test.ts
  - tests/core/wiki/date.test.ts
  - tests/core/wiki/page-merge.test.ts
  - tests/core/wiki/stale-detect.test.ts
  - tests/infra/cli-detect.test.ts
  - tests/ui/emoji-mode.test.ts
  - tests/core/wiki/frontmatter-repair.test.ts
  - tests/infra/git/source-version.test.ts
  - tests/commands/init.test.ts
  - tests/infra/global-config.test.ts
  - tests/ui/stream-parser.test.ts
  - tests/core/vault/sanity-check.test.ts
  - tests/core/wiki/lint.test.ts
  - tests/infra/git/nested-repo.test.ts
  - tests/infra/fs/raw-sync.test.ts
  - tests/core/vault/layout.test.ts
  - tests/core/vault/lock.test.ts
  - tests/infra/llm/claude-cli.test.ts
  - tests/schema/claude-md.test.ts
  - tests/core/wiki/frontmatter.test.ts
  - tests/ui/render.test.ts
-->

---
### Requirement: Acquire file-based lock for vault operations

The system SHALL acquire an exclusive file-based lock at `.codebus/.lock` before performing any operation that mutates the vault (init, ingest, future operations) and SHALL release it on normal completion.

#### Scenario: Lock prevents concurrent vault mutation

- **WHEN** one codebus process holds the lock at `.codebus/.lock`
- **AND** a second codebus process attempts to acquire the same lock
- **THEN** the second process fails with an error indicating the lock is already held

<!-- @trace
source: codebus-v2-phase1
updated: 2026-05-04
code:
  - src/infra/fs/raw-sync.ts
  - docs/superpowers/REVIEW_LESSONS.md
  - src/infra/cli-detect.ts
  - src/core/wiki/types.ts
  - README.md
  - src/core/wiki/frontmatter.ts
  - package.json
  - src/core/vault/lock.ts
  - src/core/vault/sanity-check.ts
  - src/core/wiki/stale-detect.ts
  - src/schema/claude-md.ts
  - src/commands/goal.ts
  - src/core/wiki/date.ts
  - LICENSE
  - src/cli.ts
  - tsconfig.json
  - src/commands/query.ts
  - src/infra/git/source-version.ts
  - src/ui/lint-report.ts
  - docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md
  - src/infra/llm/types.ts
  - .spectra.yaml
  - src/core/vault/layout.ts
  - src/infra/git/nested-repo.ts
  - src/commands/check.ts
  - src/core/wiki/page-merge.ts
  - vitest.config.ts
  - src/commands/init.ts
  - src/ui/stream-parser.ts
  - src/core/wiki/lint.ts
  - src/infra/llm/claude-cli.ts
  - src/infra/fs/file-ops.ts
  - src/ui/emoji-mode.ts
  - src/infra/global-config.ts
  - src/core/wiki/frontmatter-repair.ts
  - src/ui/render.ts
tests:
  - tests/e2e/init-smoke.test.ts
  - tests/infra/fs/file-ops.test.ts
  - tests/commands/goal.test.ts
  - tests/cli.test.ts
  - tests/commands/query.test.ts
  - tests/commands/check.test.ts
  - tests/core/wiki/date.test.ts
  - tests/core/wiki/page-merge.test.ts
  - tests/core/wiki/stale-detect.test.ts
  - tests/infra/cli-detect.test.ts
  - tests/ui/emoji-mode.test.ts
  - tests/core/wiki/frontmatter-repair.test.ts
  - tests/infra/git/source-version.test.ts
  - tests/commands/init.test.ts
  - tests/infra/global-config.test.ts
  - tests/ui/stream-parser.test.ts
  - tests/core/vault/sanity-check.test.ts
  - tests/core/wiki/lint.test.ts
  - tests/infra/git/nested-repo.test.ts
  - tests/infra/fs/raw-sync.test.ts
  - tests/core/vault/layout.test.ts
  - tests/core/vault/lock.test.ts
  - tests/infra/llm/claude-cli.test.ts
  - tests/schema/claude-md.test.ts
  - tests/core/wiki/frontmatter.test.ts
  - tests/ui/render.test.ts
-->

---
### Requirement: Auto-register .codebus/wiki/ as Obsidian vault on init

The system SHALL register `<repo>/.codebus/wiki/` as an Obsidian vault by writing into the user-level `obsidian.json` file during the init flow (after `.codebus/` skeleton creation, before lint and PII filter steps). The registered vault entry SHALL contain `path` (absolute, OS-native separators), `ts` (current Unix milliseconds), and `open: false`. The vault id SHALL be the lowercase 16-hex prefix of `SHA-256(absolute_path.to_lowercase())`.

The registration SHALL skip cleanly without aborting init in any of the following conditions:

- The Obsidian config directory does not exist (Obsidian is not installed on this system).
- An Obsidian process is currently running (detected via OS-specific process listing).
- The user passed `--no-obsidian-register` on the codebus CLI.
- Writing to `obsidian.json` fails for any I/O reason (permission denied, disk full, etc.); the system SHALL log a warning and continue init.

When skipping, the system SHALL emit a single hint line to stderr explaining why and what the user can do (e.g., manually add the vault in Obsidian's UI). When skipping due to "Obsidian is running", the hint SHALL state that closing Obsidian and re-running `codebus --repo <X>` will retry.

The cross-OS resolution of `obsidian.json` SHALL be:

- Windows: `%APPDATA%\obsidian\obsidian.json`
- macOS: `~/Library/Application Support/obsidian/obsidian.json`
- Linux: `~/.config/obsidian/obsidian.json`

#### Scenario: Fresh init writes new vault entry

- **WHEN** the user runs `codebus --repo X` for the first time, Obsidian is installed (`obsidian.json` exists with `{"vaults":{}}`), Obsidian is not running, and `--no-obsidian-register` is not set
- **THEN** `obsidian.json` is updated to contain a vault entry whose key is `SHA-256(abs_path.lowercase())[:16]`, `path` is the absolute path to `<X>/.codebus/wiki`, `ts` is the current Unix milliseconds, and `open` is false; init completes successfully

#### Scenario: Obsidian not installed silently skips

- **WHEN** the user runs `codebus --repo X` and the Obsidian config directory does not exist on the filesystem
- **THEN** the system skips Obsidian registration without printing an error, and init completes normally

#### Scenario: Obsidian running emits hint and skips

- **WHEN** the user runs `codebus --repo X`, `obsidian.json` exists, but an Obsidian process is detected
- **THEN** the system skips writing `obsidian.json`, emits a single stderr line containing the substrings "Obsidian" and "running" and instructing the user to close Obsidian and re-run, and init completes successfully

#### Scenario: --no-obsidian-register opt-out skips

- **WHEN** the user runs `codebus --repo X --no-obsidian-register`
- **THEN** the system does not call any Obsidian registration code path, `obsidian.json` is not read or written, and init completes normally

#### Scenario: Existing same-path entry reuses its id

- **WHEN** the user runs `codebus --repo X` and `obsidian.json` already contains a vault entry whose `path` equals `<X>/.codebus/wiki` (case-insensitive on Windows) but whose key is a different id (e.g., user previously added the vault manually in Obsidian, producing a random id)
- **THEN** the system reuses the existing entry's id (not the SHA-256 id) and only updates the entry's `ts` field; the vault list does not gain a duplicate entry

#### Scenario: I/O error during write logs warning and continues

- **WHEN** the user runs `codebus --repo X` and writing to `obsidian.json` fails (permission denied, disk full, etc.)
- **THEN** the system logs a warning containing the error reason, does not abort init, and the rest of the init flow (lint, PII setup, etc.) proceeds normally


<!-- @trace
source: obsidian-clickable-wikilinks
updated: 2026-05-08
code:
  - codebus-core/src/obsidian/mod.rs
  - codebus-core/src/obsidian/config_path.rs
  - codebus-core/src/obsidian/process_detect.rs
  - codebus-core/src/obsidian/registry.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/main.rs
tests:
  - codebus-core/src/obsidian/config_path.rs
  - codebus-core/src/obsidian/process_detect.rs
  - codebus-core/src/obsidian/registry.rs
  - codebus-cli/src/commands/init.rs
-->


<!-- @trace
source: obsidian-clickable-wikilinks
updated: 2026-05-08
code:
  - codebus-core/src/render/markdown_style.rs
  - codebus-core/src/obsidian/mod.rs
  - codebus-core/src/lib.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/obsidian/registry.rs
  - codebus-core/src/wiki/slug_index.rs
  - codebus-core/src/render/mod.rs
  - codebus-core/src/obsidian/config_path.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/wiki/mod.rs
  - codebus-core/src/render/factory.rs
  - codebus-core/src/render/renderers/terminal.rs
  - docs/superpowers/REVIEW_LESSONS.md
  - codebus-core/src/config/loader.rs
  - README.md
  - codebus-core/src/obsidian/process_detect.rs
  - codebus-core/Cargo.toml
  - codebus-cli/src/main.rs
tests:
  - codebus-core/tests/obsidian_hyperlink_e2e.rs
-->

---
### Requirement: Resolve effective vault id for hyperlink emission

The system SHALL expose the effective vault id (the actual key of the registered vault entry in `obsidian.json` after auto-registration) via the goal / query / fix flow so that the terminal renderer can construct OSC 8 hyperlink URIs targeting the correct vault. The effective id SHALL be the id used by the registered entry (which may be the codebus-computed SHA-256 id, an existing user-created random id when reusing a same-path entry, or `None` when registration was skipped).

When registration was skipped (Obsidian not installed, Obsidian running, opt-out flag, or I/O error), the effective id SHALL be `None`. The renderer SHALL treat `None` as "do not emit hyperlinks" (consistent with the terminal-output spec).

#### Scenario: Successful registration returns SHA-256 id

- **WHEN** registration writes a fresh entry with key `a38bcac8afd70c5e`
- **THEN** the goal / query / fix flow injects `vault_id: Some("a38bcac8afd70c5e")` into `RenderOptions`

#### Scenario: Same-path reuse returns existing id

- **WHEN** registration finds and reuses an existing entry whose key is `0bc358f7cc0d4f29` (a user-created random id) for the same path
- **THEN** the flow injects `vault_id: Some("0bc358f7cc0d4f29")` into `RenderOptions`

#### Scenario: Skipped registration returns None

- **WHEN** registration is skipped for any reason (Obsidian running, not installed, opt-out, I/O error)
- **THEN** the flow injects `vault_id: None` into `RenderOptions`, and the renderer emits no OSC 8 hyperlinks


<!-- @trace
source: obsidian-clickable-wikilinks
updated: 2026-05-08
code:
  - codebus-core/src/obsidian/mod.rs
  - codebus-core/src/obsidian/registry.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-cli/src/commands/query.rs
  - codebus-cli/src/commands/fix.rs
tests:
  - codebus-core/src/obsidian/registry.rs
  - codebus-cli/tests/obsidian_hyperlink_e2e.rs
-->

<!-- @trace
source: obsidian-clickable-wikilinks
updated: 2026-05-08
code:
  - codebus-core/src/render/markdown_style.rs
  - codebus-core/src/obsidian/mod.rs
  - codebus-core/src/lib.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/obsidian/registry.rs
  - codebus-core/src/wiki/slug_index.rs
  - codebus-core/src/render/mod.rs
  - codebus-core/src/obsidian/config_path.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/wiki/mod.rs
  - codebus-core/src/render/factory.rs
  - codebus-core/src/render/renderers/terminal.rs
  - docs/superpowers/REVIEW_LESSONS.md
  - codebus-core/src/config/loader.rs
  - README.md
  - codebus-core/src/obsidian/process_detect.rs
  - codebus-core/Cargo.toml
  - codebus-cli/src/main.rs
tests:
  - codebus-core/tests/obsidian_hyperlink_e2e.rs
-->