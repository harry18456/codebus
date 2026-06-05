## ADDED Requirements

### Requirement: Tag-triggered Windows release build

A GitHub Actions workflow SHALL build the Windows NSIS installer on a `windows-latest` runner when a tag matching `v*` is pushed, and SHALL also be triggerable manually via `workflow_dispatch`. The workflow SHALL reproduce the local `tauri build` path by invoking the official `tauri-apps/tauri-action` with `projectPath` set to the app directory, so that the project's existing `beforeBuildCommand` (release CLI staging plus frontend build) runs unchanged. The workflow SHALL NOT reimplement CLI staging or modify the existing bundle configuration. The produced installer SHALL be unsigned.

#### Scenario: tag push triggers build

- **WHEN** a tag matching `v*` is pushed to the repository
- **THEN** the `release-windows` workflow runs on `windows-latest`, prepares the Rust and Node toolchains, runs `npm ci` in the app directory, and invokes `tauri-action` which runs the existing `beforeBuildCommand` and produces an NSIS `-setup.exe`

#### Scenario: manual dispatch triggers build

- **WHEN** the workflow is started manually via `workflow_dispatch`
- **THEN** the same build steps run and produce an NSIS `-setup.exe`

##### Example: trigger configuration

| Trigger | Configured | Purpose |
| ------- | ---------- | ------- |
| push tag `v*` | yes | primary release path |
| `workflow_dispatch` | yes | manual re-run / test |
| push to branch | no | nightly builds are out of scope |

### Requirement: Installer published as a draft GitHub Release

On a successful build, the workflow SHALL publish a GitHub Release in **draft** state, using the built-in `GITHUB_TOKEN` with `contents: write` permission, and SHALL attach the NSIS `-setup.exe` as a release asset. The release SHALL remain a draft until a human publishes it, so version consistency and unsigned-installer guidance can be confirmed before public release.

#### Scenario: successful build creates a draft release with the installer asset

- **WHEN** the build job completes successfully
- **THEN** a draft GitHub Release is created for the triggering tag and the `-setup.exe` is attached to it as an asset, and the release is not published automatically

### Requirement: Failed build does not publish a release

If any workflow step fails (toolchain setup, `npm ci`, the Tauri build, or installer production), the job SHALL fail and SHALL NOT create or publish any GitHub Release. The workflow SHALL NOT leave a partial or empty release.

#### Scenario: build failure produces no release

- **WHEN** any step of the workflow fails before the installer is produced
- **THEN** the job is marked failed and no GitHub Release (draft or published) is created for that run
