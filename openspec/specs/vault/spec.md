# vault Specification

## Purpose

The on-disk structure of the `.codebus/` directory `codebus init` creates and subsequent verbs read or write — directory layout (5 wiki taxonomy folders + raw mirror + nested git), `manifest.yaml` source-signal record, internal `.gitignore` rules, source repo `.gitignore` mutation, Obsidian vault registration, sanity check (init refuses to run inside an existing vault), nested git repository initialization with the canonical `init: codebus vault` first commit, and source-signal drift detection that gates raw-mirror re-sync on subsequent verb invocations. Does NOT cover wiki content rules (those live in `lint-feedback-loop`), PII redaction of raw mirror contents (`pii-filter`), or skill bundle SKILL.md authoring (`skill-bundles`).

## Requirements

### Requirement: Vault Layout

The system SHALL create a `.codebus/` vault under the source repository root containing the following subdirectories: `wiki/concepts/`, `wiki/entities/`, `wiki/modules/`, `wiki/processes/`, `wiki/synthesis/`, `raw/code/`, and `log/`. The system SHALL NOT create `output/` or `goals.jsonl` inside the vault.

#### Scenario: Init creates the seven required subdirectories under .codebus

- **WHEN** init is invoked against a repository with no existing `.codebus/`
- **THEN** the system SHALL create `.codebus/wiki/concepts/`, `.codebus/wiki/entities/`, `.codebus/wiki/modules/`, `.codebus/wiki/processes/`, `.codebus/wiki/synthesis/`, `.codebus/raw/code/`, and `.codebus/log/` AND SHALL NOT create `.codebus/output/` or `.codebus/goals.jsonl`

#### Scenario: Re-running init is idempotent for layout

- **WHEN** init is invoked twice in succession against the same repository
- **THEN** both invocations SHALL succeed AND the second SHALL NOT change the directory listing of the seven required subdirectories beyond what the first established


<!-- @trace
source: v3-vault-history
updated: 2026-05-09
code:
  - codebus-core/src/git/nested_repo.rs
  - codebus-core/src/vault/layout.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/git/mod.rs
  - codebus-core/src/lib.rs
tests:
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: Sanity Check Inside Vault

The system SHALL refuse to initialize a vault when the target repository path resolves to a directory whose name is `.codebus` OR whose immediate parent contains a `manifest.yaml` AND a `wiki/` directory at sibling level. The refusal SHALL print a clear error to stderr and exit with non-zero status before any filesystem mutation occurs.

#### Scenario: Refuses init when target path is itself a .codebus vault

- **WHEN** init is invoked with the target repository path resolving to an existing vault root (a directory whose siblings include `wiki/` and `manifest.yaml`)
- **THEN** the system SHALL exit with non-zero status BEFORE creating any directory or writing any file AND SHALL emit a stderr message identifying that the path appears to be a codebus vault


<!-- @trace
source: v3-init
updated: 2026-05-08
code:
  - codebus-core/src/vault/raw_sync.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/schema/mod.rs
  - Cargo.toml
  - codebus-core/src/schema/neutral.md
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-core/src/vault/obsidian_register.rs
  - codebus-core/src/vault/layout.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/sanity_check.rs
  - codebus-core/src/vault/mod.rs
  - codebus-cli/Cargo.toml
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
  - codebus-core/tests/schema_neutrality.rs
-->

---
### Requirement: Raw Mirror with PII Scanner

The system SHALL mirror source files from the repository root into `.codebus/raw/code/` preserving directory structure. The system SHALL skip top-level entries `.codebus/`, `.git/`, and `.env`. The system SHALL honor `<repo>/.gitignore` patterns. The system SHALL skip files larger than 5 mebibytes (5 × 1024 × 1024 bytes). The system SHALL invoke a configured `PiiScanner` against the contents of every mirrored regular file. When the scanner reports any match for a file under the default on-hit policy `Warn`, the system SHALL emit one stderr line per match in the format `pii warn: <pattern_name> at <relative_path>:<byte_offset>` and SHALL include that file in the mirror with content unchanged. The system SHALL NOT include the matched substring text in the warning line. The system SHALL aggregate the total count of PII matches observed across all mirrored files and SHALL expose that count through the raw mirror summary value returned to callers.

#### Scenario: Mirror preserves directory structure and skips top-level dot directories

- **WHEN** a repository contains `src/main.rs`, `nested/lib.rs`, `.git/config`, `.env`, and `.codebus/manifest.yaml`
- **THEN** the system SHALL mirror `src/main.rs` and `nested/lib.rs` into `.codebus/raw/code/src/main.rs` and `.codebus/raw/code/nested/lib.rs` respectively AND SHALL NOT mirror `.git/config`, `.env`, or `.codebus/manifest.yaml`

#### Scenario: Mirror honors source repo .gitignore

- **WHEN** the source repository's `.gitignore` contains `target/` AND a file `target/debug/foo.rs` exists
- **THEN** the system SHALL NOT mirror `target/debug/foo.rs` into the vault

#### Scenario: Mirror skips files exceeding the size limit

- **WHEN** a source file is larger than 5 × 1024 × 1024 bytes
- **THEN** the system SHALL skip that file (no mirror entry created) and continue processing remaining files

#### Scenario: PII match emits stderr warning and still mirrors file

- **WHEN** a source file at `src/aws.py` contains the substring `AKIAIOSFODNN7EXAMPLE` (an AWS access key shape) and is mirrored
- **THEN** stderr SHALL contain a line beginning with `pii warn: aws-access-key at src/aws.py:` AND the file `.codebus/raw/code/src/aws.py` SHALL exist with the original content unchanged

#### Scenario: stderr warning omits matched text

- **WHEN** a mirrored source file contains the substring `alice@example.com`
- **THEN** the corresponding stderr `pii warn:` line SHALL NOT contain the substring `alice@example.com`

#### Scenario: Multiple matches in one file produce multiple warning lines

- **WHEN** a single mirrored source file contains both `alice@example.com` and `192.168.1.42`
- **THEN** stderr SHALL contain two distinct `pii warn:` lines whose `<pattern_name>` segments are `email` and `ipv4` respectively

#### Scenario: Raw mirror summary aggregates total PII match count

- **WHEN** the raw mirror processes a repository where exactly N PII matches are reported across all mirrored files combined
- **THEN** the summary value returned to callers SHALL expose a PII match count field equal to N


<!-- @trace
source: v3-pii
updated: 2026-05-09
code:
  - codebus-core/src/pii/scanners/null_scanner.rs
  - codebus-core/src/pii/scanners/mod.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/lib.rs
  - codebus-core/Cargo.toml
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/pii/provider.rs
  - codebus-core/src/pii/mod.rs
  - codebus-core/src/pii/scanners/regex_basic.rs
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
-->

---
### Requirement: Source Repo .gitignore Mutation

When the source repository contains a `.git/` directory at its root, the system SHALL ensure the literal entry `.codebus/` is present on its own line in the source repository's root `.gitignore` file. If `.gitignore` does not exist, the system SHALL create it. If the entry is already present, the system SHALL NOT modify the file. If the existing file lacks a trailing newline, the system SHALL add one before appending the entry. When the source repository has no `.git/` directory at its root, the system SHALL NOT modify or create `.gitignore`.

#### Scenario: Adds .codebus entry to existing .gitignore

- **WHEN** init runs against a git repository with `.gitignore` containing `node_modules\n` (with trailing newline)
- **THEN** the resulting `.gitignore` SHALL equal `node_modules\n.codebus/\n` exactly

#### Scenario: Creates .gitignore when missing

- **WHEN** init runs against a git repository without a `.gitignore` file
- **THEN** the system SHALL create `.gitignore` containing exactly `.codebus/\n`

#### Scenario: Idempotent when entry already present

- **WHEN** init runs against a git repository whose `.gitignore` already contains a line equal to `.codebus/`
- **THEN** the system SHALL NOT modify the file AND the file SHALL contain exactly one line equal to `.codebus/`

#### Scenario: Skips non-git directory

- **WHEN** init runs against a directory without a `.git/` subdirectory
- **THEN** the system SHALL NOT create or modify `.gitignore` at the target path


<!-- @trace
source: v3-init
updated: 2026-05-08
code:
  - codebus-core/src/vault/raw_sync.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/schema/mod.rs
  - Cargo.toml
  - codebus-core/src/schema/neutral.md
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-core/src/vault/obsidian_register.rs
  - codebus-core/src/vault/layout.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/sanity_check.rs
  - codebus-core/src/vault/mod.rs
  - codebus-cli/Cargo.toml
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
  - codebus-core/tests/schema_neutrality.rs
-->

---
### Requirement: Per-Repo Schema File

The system SHALL write `.codebus/CLAUDE.md` containing the path-D-neutral schema (Karpathy 5-folder taxonomy, frontmatter conventions, wikilinks rules, page conflict rules, stopping criteria) when no `CLAUDE.md` exists at that path. When `.codebus/CLAUDE.md` already exists, the system SHALL NOT modify it. The schema content SHALL NOT reference vendor-specific tool names (`claude`, `anthropic`, `stream-json`, `--tools`, `codex`, `gemini`, `cursor`).

#### Scenario: Writes schema when missing

- **WHEN** init runs against a target whose `.codebus/CLAUDE.md` does not exist
- **THEN** the system SHALL write the schema content to that path AND the written content SHALL contain the substrings `concepts`, `entities`, `modules`, `processes`, `synthesis` (the five taxonomy folder names)

#### Scenario: Preserves existing schema customization

- **WHEN** init runs against a target whose `.codebus/CLAUDE.md` already contains the line `# my customized schema header`
- **THEN** the system SHALL NOT modify the file AND the file SHALL still contain `# my customized schema header`

#### Scenario: Schema content is vendor-neutral

- **WHEN** the schema content is written to `.codebus/CLAUDE.md`
- **THEN** the file content SHALL NOT contain any of the substrings `claude`, `anthropic`, `stream-json`, `--tools`, `codex`, `gemini`, `cursor` (case-insensitive)


<!-- @trace
source: v3-init
updated: 2026-05-08
code:
  - codebus-core/src/vault/raw_sync.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/schema/mod.rs
  - Cargo.toml
  - codebus-core/src/schema/neutral.md
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-core/src/vault/obsidian_register.rs
  - codebus-core/src/vault/layout.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/sanity_check.rs
  - codebus-core/src/vault/mod.rs
  - codebus-cli/Cargo.toml
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
  - codebus-core/tests/schema_neutrality.rs
-->

---
### Requirement: Vault Manifest Records Sync State

The system SHALL write `.codebus/manifest.yaml` containing five top-level fields plus one nested mapping: `codebus_version` (string, cargo package version of `codebus-cli` at first init time), `created_at` (string, UTC ISO 8601 timestamp ending in `Z` at first init time), `repo_root` (string, absolute filesystem path of the source repository at first init time), `last_sync_at` (string, UTC ISO 8601 timestamp updated on every init invocation), and `source_signal` (mapping with three keys: `git_head` (string or null), `file_count` (integer), `total_bytes` (integer)).

On first init (no manifest exists), the system SHALL write all five top-level fields plus `source_signal`. On subsequent init invocations against an existing manifest, the system SHALL preserve `codebus_version`, `created_at`, and `repo_root` unchanged, AND SHALL update `last_sync_at` to the current UTC timestamp, AND SHALL update `source_signal` to reflect the current source state.

The `source_signal.git_head` value SHALL be the verbatim contents of `<repo>/.git/HEAD` (which may be a symbolic ref like `ref: refs/heads/main\n` or a detached SHA) when `<repo>/.git/HEAD` is readable, OR null when no git repo is detected (`<repo>/.git/` absent).

The `source_signal.file_count` and `source_signal.total_bytes` SHALL be the file count and aggregate byte total of files mirrored by `raw_sync` during this init invocation.

#### Scenario: Writes manifest with all required fields on first init

- **WHEN** init runs against a target whose `.codebus/manifest.yaml` does not exist
- **THEN** the system SHALL create `.codebus/manifest.yaml` AND parsing it as YAML SHALL yield a mapping containing the keys `codebus_version`, `created_at`, `repo_root`, `last_sync_at`, and `source_signal` AND both `created_at` and `last_sync_at` SHALL match the format `YYYY-MM-DDTHH:MM:SSZ` AND `repo_root` SHALL be an absolute path

#### Scenario: source_signal records git_head when target is a git repo

- **WHEN** init runs against a git repository whose `<repo>/.git/HEAD` exists with content `ref: refs/heads/main\n`
- **THEN** the manifest's `source_signal.git_head` SHALL equal the verbatim content of that HEAD file (preserving the `ref: ` prefix and trailing newline)

#### Scenario: source_signal records null git_head when target is non-git

- **WHEN** init runs against a directory without a `.git/` subdirectory
- **THEN** the manifest's `source_signal.git_head` SHALL be YAML null

#### Scenario: source_signal records file aggregates from raw mirror

- **WHEN** init runs against a target where `raw_sync` mirrors exactly N files totaling M bytes
- **THEN** `source_signal.file_count` SHALL equal N AND `source_signal.total_bytes` SHALL equal M

#### Scenario: Re-init preserves write-once fields and updates sync state fields

- **WHEN** init is invoked twice against the same repository, with at least one source file modified between invocations
- **THEN** the second invocation SHALL leave `codebus_version`, `created_at`, and `repo_root` unchanged from the first invocation AND SHALL update `last_sync_at` to a timestamp newer than (or equal to) the first invocation's `last_sync_at` AND SHALL update `source_signal` to reflect the new file aggregates


<!-- @trace
source: v3-init
updated: 2026-05-08
code:
  - codebus-core/src/vault/raw_sync.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/schema/mod.rs
  - Cargo.toml
  - codebus-core/src/schema/neutral.md
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-core/src/vault/obsidian_register.rs
  - codebus-core/src/vault/layout.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/sanity_check.rs
  - codebus-core/src/vault/mod.rs
  - codebus-cli/Cargo.toml
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
  - codebus-core/tests/schema_neutrality.rs
-->

---
### Requirement: Obsidian Vault Auto-Registration

When the user has not passed `--no-obsidian-register`, the system SHALL register `.codebus/wiki/` as an Obsidian vault by writing an entry into the platform Obsidian configuration file. Registration SHALL be fail-soft: if the Obsidian config directory does not exist, OR an existing config cannot be parsed, OR write permission is denied, the system SHALL print a stderr hint AND continue init successfully (exit zero). When the user passes `--no-obsidian-register`, the system SHALL NOT touch the Obsidian config file regardless of its state.

#### Scenario: Default flow registers vault

- **WHEN** init runs without `--no-obsidian-register` against a system with an existing Obsidian config directory
- **THEN** the platform Obsidian config SHALL gain a vault entry referencing the absolute path of `.codebus/wiki/` AND the system SHALL exit with status zero

#### Scenario: --no-obsidian-register skips registration entirely

- **WHEN** init runs with `--no-obsidian-register`
- **THEN** the system SHALL NOT read or write the Obsidian config file AND init SHALL still exit with status zero

#### Scenario: Obsidian unavailable does not fail init

- **WHEN** init runs without `--no-obsidian-register` against a system where the Obsidian config directory does not exist
- **THEN** the system SHALL print a stderr hint indicating Obsidian is not detected AND init SHALL still exit with status zero

<!-- @trace
source: v3-init
updated: 2026-05-08
code:
  - codebus-core/src/vault/raw_sync.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/schema/mod.rs
  - Cargo.toml
  - codebus-core/src/schema/neutral.md
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-core/src/vault/obsidian_register.rs
  - codebus-core/src/vault/layout.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/sanity_check.rs
  - codebus-core/src/vault/mod.rs
  - codebus-cli/Cargo.toml
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
  - codebus-core/tests/schema_neutrality.rs
-->

---
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


<!-- @trace
source: v3-vault-history
updated: 2026-05-09
code:
  - codebus-core/src/git/nested_repo.rs
  - codebus-core/src/vault/layout.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/git/mod.rs
  - codebus-core/src/lib.rs
tests:
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: Nested Git Repository Initialization

The system SHALL initialize a nested git repository at the path `.codebus/` during init when no `.codebus/.git/` directory exists. The initialization SHALL set the initial branch name to `main` and SHALL configure the nested repository's local `user.email` to the literal string `codebus@local` and local `user.name` to the literal string `codebus`. The system SHALL NOT depend on the user's global git config for either value. When `.codebus/.git/` already exists at init time, the system SHALL treat the operation as a no-op: it SHALL NOT re-initialize the repository, SHALL NOT modify any existing local config, and SHALL NOT alter the existing branch state.

#### Scenario: First init creates nested git repo with codebus identity

- **WHEN** init runs against a repository with no existing `.codebus/.git/`
- **THEN** after init the directory `.codebus/.git/` SHALL exist AND running `git -C <vault-root> config --get user.email` SHALL print exactly `codebus@local` AND running `git -C <vault-root> config --get user.name` SHALL print exactly `codebus`

#### Scenario: Re-init does not overwrite user-modified local git config

- **WHEN** init runs against a vault whose `.codebus/.git/` already exists with local `user.email` previously set to `alice@example.com`
- **THEN** after init the local `user.email` SHALL still equal `alice@example.com` AND the system SHALL NOT have run `git init` against the existing repository


<!-- @trace
source: v3-vault-history
updated: 2026-05-09
code:
  - codebus-core/src/git/nested_repo.rs
  - codebus-core/src/vault/layout.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/git/mod.rs
  - codebus-core/src/lib.rs
tests:
  - codebus-cli/tests/cli_routing.rs
-->

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

<!-- @trace
source: v3-vault-history
updated: 2026-05-09
code:
  - codebus-core/src/git/nested_repo.rs
  - codebus-core/src/vault/layout.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/git/mod.rs
  - codebus-core/src/lib.rs
tests:
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: Source-Signal Detection on Verb Invocation

The system SHALL provide a source-signal drift detection operation that determines whether the raw mirror needs to be re-synced before a verb invocation proceeds. The operation SHALL compare the manifest's persisted `source_signal` (written by init's manifest step) against a freshly-recomputed signal from the same source repository: the `git_head` field (`<repo>/.git/HEAD` verbatim contents or YAML null), the `file_count` field (count of files that would be mirrored under current mirror rules), and the `total_bytes` field (aggregate byte total of the same set). When any of the three fields differs between the persisted signal and the recomputed signal, the operation SHALL report the source as drifted. When all three fields are equal, the operation SHALL report the source as unchanged.

When detection itself cannot complete successfully (manifest file is missing, malformed YAML, or unreadable; git HEAD I/O error; source repository walk failure), the operation SHALL fail-safe and report the source as drifted, ensuring the caller proceeds with a re-sync rather than skipping it.

The detection operation SHALL be invoked by verbs that read or write the raw mirror (currently `goal`); after a re-sync triggered by drift, the system SHALL update the manifest's `source_signal` to reflect the new state.

#### Scenario: Detection reports unchanged when all three signal fields match

- **WHEN** the manifest's `source_signal.git_head`, `source_signal.file_count`, and `source_signal.total_bytes` all equal their respective recomputed values from the current source state
- **THEN** the detection operation SHALL report unchanged AND the caller SHALL skip the raw mirror re-sync

#### Scenario: Detection reports drifted when git_head differs

- **WHEN** the manifest's `source_signal.git_head` is `ref: refs/heads/main\n` AND the current `<repo>/.git/HEAD` content is `ref: refs/heads/feature\n`
- **THEN** the detection operation SHALL report drifted regardless of file_count and total_bytes

#### Scenario: Detection reports drifted when file_count differs

- **WHEN** the manifest's `source_signal.file_count` is 142 AND the recomputed file_count is 143
- **THEN** the detection operation SHALL report drifted

#### Scenario: Detection reports drifted when total_bytes differs

- **WHEN** the manifest's `source_signal.total_bytes` is 89234 AND the recomputed total_bytes is 89890
- **THEN** the detection operation SHALL report drifted

#### Scenario: Detection fail-safe when manifest is missing

- **WHEN** the detection operation is invoked but `<repo>/.codebus/manifest.yaml` does not exist
- **THEN** the operation SHALL report drifted (fail-safe) AND the caller SHALL proceed with a re-sync

#### Scenario: Detection fail-safe when manifest is malformed

- **WHEN** the detection operation is invoked and `<repo>/.codebus/manifest.yaml` cannot be parsed as valid YAML
- **THEN** the operation SHALL report drifted (fail-safe) rather than propagating a parse error

#### Scenario: Re-sync after drift updates the manifest signal

- **WHEN** detection reports drifted, the caller re-runs the raw mirror, and the new mirror state has `file_count=N` and `total_bytes=B`
- **THEN** the manifest's `source_signal.file_count` SHALL equal N AND `source_signal.total_bytes` SHALL equal B AND the manifest's `last_sync_at` SHALL be updated to the current UTC timestamp

<!-- @trace
source: v3-goal
updated: 2026-05-09
code:
  - codebus-core/src/vault/source_signal_detect.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/agent/mod.rs
  - codebus-cli/Cargo.toml
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/vault/mod.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/vault/raw_sync.rs
tests:
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/bins/mock_claude.rs
  - codebus-cli/tests/cli_routing.rs
-->