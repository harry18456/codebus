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