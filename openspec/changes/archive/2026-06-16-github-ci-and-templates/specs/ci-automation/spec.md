## ADDED Requirements

### Requirement: Branch and pull request CI workflow

A GitHub Actions CI workflow SHALL run for branch pushes and pull requests. The workflow SHALL use `windows-latest` as the runner so the default validation environment covers the repository's Windows-specific process, CLI shim, path, and keyring behavior. The workflow SHALL remain separate from the existing tag-triggered Windows release workflow and SHALL NOT publish releases or build release installers.

#### Scenario: branch push triggers CI

- **WHEN** a commit is pushed to a repository branch
- **THEN** the CI workflow runs on `windows-latest` and executes validation jobs without invoking the release workflow

#### Scenario: pull request triggers CI

- **WHEN** a pull request is opened or updated
- **THEN** the CI workflow runs on `windows-latest` and reports Rust and app validation status as GitHub Actions checks

#### Scenario: tag release workflow remains separate

- **WHEN** a `v*` tag is pushed for release automation
- **THEN** the existing release workflow remains responsible for installer publishing, and the CI workflow does not create or publish a GitHub Release

### Requirement: Rust workspace validation with clippy baseline guard

The CI workflow SHALL install the stable Rust toolchain, use workspace Cargo caching, run `cargo test --workspace`, and run `cargo clippy --workspace`. The clippy step MUST NOT use `-D warnings` because the repository has an accepted warning baseline. The baseline guard SHALL permit warning counts at or below `codebus-core=8`, `codebus-cli=5`, and `codebus-app-tauri=6`, and SHALL fail when any package exceeds its baseline.

#### Scenario: existing clippy baseline passes

- **WHEN** `cargo clippy --workspace` reports warning counts no higher than `codebus-core=8`, `codebus-cli=5`, and `codebus-app-tauri=6`
- **THEN** the clippy validation step passes and exposes the warning output in the CI logs

#### Scenario: new clippy warning fails CI

- **WHEN** `cargo clippy --workspace` reports a warning count above the accepted baseline for any tracked package
- **THEN** the clippy validation step fails and identifies the package whose warning count exceeded the baseline

### Requirement: codebus-app npm validation

The CI workflow SHALL install Node.js 20, enable npm cache using `codebus-app/package-lock.json`, run `npm ci` in `codebus-app`, then run `npm run test` and `npm run typecheck` in `codebus-app`. The CI workflow SHALL treat failures from Vitest or TypeScript typechecking as failing checks.

#### Scenario: app tests and typecheck pass

- **WHEN** `npm ci`, `npm run test`, and `npm run typecheck` complete successfully in `codebus-app`
- **THEN** the app validation portion of CI passes

#### Scenario: app validation failure fails CI

- **WHEN** Vitest or TypeScript typechecking exits non-zero in `codebus-app`
- **THEN** the CI workflow reports a failed GitHub Actions check
