## MODIFIED Requirements

### Requirement: Vault Layout

The system SHALL create a `.codebus/` vault under the source repository root containing the following subdirectories: `wiki/concepts/`, `wiki/entities/`, `wiki/modules/`, `wiki/processes/`, `wiki/synthesis/`, `raw/code/`, and `log/`. The system SHALL NOT create `output/` or `goals.jsonl` inside the vault.

#### Scenario: Init creates the seven required subdirectories under .codebus

- **WHEN** init is invoked against a repository with no existing `.codebus/`
- **THEN** the system SHALL create `.codebus/wiki/concepts/`, `.codebus/wiki/entities/`, `.codebus/wiki/modules/`, `.codebus/wiki/processes/`, `.codebus/wiki/synthesis/`, `.codebus/raw/code/`, and `.codebus/log/` AND SHALL NOT create `.codebus/output/` or `.codebus/goals.jsonl`

#### Scenario: Re-running init is idempotent for layout

- **WHEN** init is invoked twice in succession against the same repository
- **THEN** both invocations SHALL succeed AND the second SHALL NOT change the directory listing of the seven required subdirectories beyond what the first established

## ADDED Requirements

### Requirement: Internal Vault .gitignore Content

The system SHALL ensure that the file `.codebus/.gitignore` exists and contains the following four required entries on their own lines: `.lock`, `raw/code/`, `**/.obsidian/`, `logs/`. When the file does not exist, the system SHALL create it containing exactly those four lines, each terminated by a newline, in the declared order. When the file already exists, the system SHALL append any missing required lines while preserving existing content. The system SHALL NOT remove or reorder lines that are already present in the file, including user-added entries beyond the four required ones.

#### Scenario: Creates internal gitignore on first init

- **WHEN** init runs against a repository with no existing `.codebus/.gitignore`
- **THEN** the system SHALL create `.codebus/.gitignore` containing exactly the four lines `.lock`, `raw/code/`, `**/.obsidian/`, `logs/` (in that order, each terminated by a newline) and no other content

#### Scenario: Appends missing required lines to existing gitignore

- **WHEN** init runs against a vault whose `.codebus/.gitignore` contains only the two lines `.lock` and `notes/`
- **THEN** the resulting file SHALL contain the line `.lock` followed by the line `notes/` (preserved order) followed by the three missing required lines `raw/code/`, `**/.obsidian/`, `logs/` (appended in declared order)

#### Scenario: Preserves user additions during merge

- **WHEN** init runs against a vault whose `.codebus/.gitignore` already contains all four required lines plus an extra line `notes/`
- **THEN** after init the file SHALL still contain the line `notes/` AND SHALL NOT add any line a second time

---

### Requirement: Nested Git Repository Initialization

The system SHALL initialize a nested git repository at the path `.codebus/` during init when no `.codebus/.git/` directory exists. The initialization SHALL set the initial branch name to `main` and SHALL configure the nested repository's local `user.email` to the literal string `codebus@local` and local `user.name` to the literal string `codebus`. The system SHALL NOT depend on the user's global git config for either value. When `.codebus/.git/` already exists at init time, the system SHALL treat the operation as a no-op: it SHALL NOT re-initialize the repository, SHALL NOT modify any existing local config, and SHALL NOT alter the existing branch state.

#### Scenario: First init creates nested git repo with codebus identity

- **WHEN** init runs against a repository with no existing `.codebus/.git/`
- **THEN** after init the directory `.codebus/.git/` SHALL exist AND running `git -C <vault-root> config --get user.email` SHALL print exactly `codebus@local` AND running `git -C <vault-root> config --get user.name` SHALL print exactly `codebus`

#### Scenario: Re-init does not overwrite user-modified local git config

- **WHEN** init runs against a vault whose `.codebus/.git/` already exists with local `user.email` previously set to `alice@example.com`
- **THEN** after init the local `user.email` SHALL still equal `alice@example.com` AND the system SHALL NOT have run `git init` against the existing repository

---

### Requirement: Initial Auto-Commit On Init

After all init artifacts have been written (vault layout, raw mirror, internal `.gitignore`, nested git initialization, source repo `.gitignore` mutation, schema, manifest, skill bundles, and the optional Obsidian registration step), the system SHALL execute an auto-commit operation on the nested vault repository consisting of `git add -A` followed by `git commit -m "init: codebus vault"`. When the nested working tree is fully clean (no staged or unstaged changes after `git add -A`), the system SHALL NOT create a new commit and SHALL preserve the existing HEAD. When the auto-commit operation fails for any reason (git binary missing, commit hook rejection, filesystem I/O error), init SHALL exit with a non-zero status code and SHALL emit a stderr message identifying the failure.

#### Scenario: First init produces a commit with the canonical init message

- **WHEN** init runs to completion against a repository with no existing `.codebus/`
- **THEN** running `git -C <vault-root> log --pretty=%s -1` SHALL print exactly the line `init: codebus vault`

#### Scenario: First commit captures the schema file and the manifest

- **WHEN** init runs to completion against a repository
- **THEN** running `git -C <vault-root> ls-tree -r HEAD --name-only` SHALL include both `CLAUDE.md` and `manifest.yaml` in its output AND running `git -C <vault-root> status --porcelain` SHALL print no output

#### Scenario: First commit excludes raw/code via internal gitignore

- **WHEN** init runs to completion against a repository where `raw/code/` contains at least one mirrored file
- **THEN** running `git -C <vault-root> ls-tree -r HEAD --name-only` SHALL NOT include any path beginning with `raw/code/`

#### Scenario: Auto-commit failure surfaces as init non-zero exit

- **WHEN** init runs against a target where the `git` binary is unavailable on PATH at the moment of nested repo initialization
- **THEN** init SHALL exit with a non-zero status code AND SHALL emit a stderr line whose content identifies the failure as related to git or auto-commit
