## ADDED Requirements

### Requirement: Monorepo directory layout

The repository SHALL contain three top-level implementation directories plus a fixtures directory, matching the Monorepo layout decision recorded in `docs/decisions.md` D-013.

#### Scenario: Required directories exist

- **WHEN** the repository is inspected after M1 is complete
- **THEN** directories `tauri/`, `sidecar/`, `web/`, and `tests/fixtures/` MUST exist at the repository root

#### Scenario: Each implementation directory holds its language manifest

- **WHEN** a build tool is invoked inside an implementation directory
- **THEN** `tauri/src-tauri/Cargo.toml`, `sidecar/pyproject.toml`, and `web/package.json` MUST be present and parse without errors

### Requirement: Python toolchain managed by uv

The sidecar project SHALL use uv as its Python package manager and virtual environment tool, per `docs/decisions.md` D-014.

#### Scenario: Clean install reproduces environment

- **WHEN** a developer runs `uv sync` from a clean checkout inside `sidecar/`
- **THEN** uv MUST create a virtual environment and install all locked dependencies without manual intervention

#### Scenario: Lockfile is committed

- **WHEN** the repository is cloned
- **THEN** `sidecar/uv.lock` MUST exist and MUST be tracked by git

### Requirement: Pre-commit stage-0 hooks configured

The repository SHALL contain `.pre-commit-config.yaml` at the root with the stage-0 hook set defined in design decision D-local-7.

#### Scenario: Stage-0 hooks run on commit

- **WHEN** `pre-commit run --all-files` is executed after `pre-commit install`
- **THEN** the hooks `trailing-whitespace`, `end-of-file-fixer`, `check-yaml`, `check-json`, `check-merge-conflict`, and `mixed-line-ending` (configured with `--fix=lf`) MUST run and MUST report success on a clean repository

#### Scenario: Commit gate integrates with Claude Code hook

- **WHEN** a `git commit` is issued inside a Claude Code Bash tool call
- **THEN** the PreToolUse hook in `.claude/settings.json` MUST invoke `pre-commit run --hook-stage pre-commit` and MUST block the commit (exit code 2) if any stage-0 hook fails
