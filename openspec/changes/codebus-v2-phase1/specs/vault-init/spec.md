## ADDED Requirements

### Requirement: Initialize .codebus/ vault structure under user repo

When invoked with `--repo <path>` and no `--goal` or `--query`, the system SHALL create the `.codebus/` vault under the given path containing all subdirectories required by the wiki workflow.

#### Scenario: Fresh init creates all expected paths

- **WHEN** the user runs `codebus --repo /path/to/myrepo` and `/path/to/myrepo/.codebus/` does not yet exist
- **THEN** the system creates `/path/to/myrepo/.codebus/` with subdirectories `raw/`, `raw/code/`, `wiki/`, `wiki/pages/`, `wiki/goals/`, `output/`, and files `CLAUDE.md`, `goals.jsonl`, `.gitignore`

#### Scenario: Internal .gitignore excludes lock and raw code

- **WHEN** init writes `.codebus/.gitignore`
- **THEN** the file contains entries for `.lock` and `raw/code/` so that lock files and the codebase mirror are not tracked by the nested git repo

### Requirement: Install built-in CLAUDE.md schema

The system SHALL write a built-in CLAUDE.md schema to `.codebus/CLAUDE.md` during init, and SHALL NOT overwrite an existing CLAUDE.md (to preserve user customizations).

#### Scenario: First init writes schema

- **WHEN** init runs and `.codebus/CLAUDE.md` does not exist
- **THEN** the system writes the built-in schema content (12 sections covering role, layout, workflow, frontmatter, etc.) with an SPDX-License-Identifier header

#### Scenario: Re-init preserves user-modified schema

- **WHEN** `.codebus/CLAUDE.md` already exists with user modifications
- **AND** init runs again
- **THEN** the system leaves the existing file unchanged

### Requirement: Initialize nested git repository at .codebus/.git

The system SHALL initialize a nested git repo at `.codebus/.git` during init so wiki revisions are versioned independently of the source repo.

#### Scenario: Nested git is created with initial commit

- **WHEN** init runs and `.codebus/.git` does not exist
- **THEN** the system runs `git init -b main` inside `.codebus/`, configures a codebus identity, and produces an initial commit containing the schema and structure

#### Scenario: Re-init does not re-initialize existing nested git

- **WHEN** `.codebus/.git` already exists
- **AND** init runs again
- **THEN** the system leaves the existing `.git` directory untouched

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

### Requirement: Init is idempotent

Running init twice SHALL produce the same final state without errors.

#### Scenario: Two consecutive inits succeed

- **WHEN** the user runs `codebus --repo X` twice in a row
- **THEN** both invocations succeed, `.codebus/` exists with all expected paths, and no duplicate entries appear in source `.gitignore`

### Requirement: Acquire file-based lock for vault operations

The system SHALL acquire an exclusive file-based lock at `.codebus/.lock` before performing any operation that mutates the vault (init, ingest, future operations) and SHALL release it on normal completion.

#### Scenario: Lock prevents concurrent vault mutation

- **WHEN** one codebus process holds the lock at `.codebus/.lock`
- **AND** a second codebus process attempts to acquire the same lock
- **THEN** the second process fails with an error indicating the lock is already held
