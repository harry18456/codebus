# windows-distribution Specification

## Purpose

Defines how codebus is packaged and distributed on Windows: a single Tauri NSIS installer that ships both the `codebus-app` GUI and the `codebus` CLI binaries, installs per-user without Administrator rights, manages the CLI's PATH entry (add on install, reverse on uninstall), and never touches user data.

## Requirements

### Requirement: Windows installer bundles GUI and CLI binaries

The `tauri build` invocation, with bundling enabled, SHALL produce a Windows NSIS installer (`-setup.exe`) whose payload contains BOTH the `codebus-app` GUI binary (installed as `codebus-app.exe` at the install root, since Tauri derives the main binary name from the app crate's bin name, not the product name) AND the `codebus` CLI binary (installed as `codebus.exe` under a `bin/` subdirectory). The test-only `mock-claude` binary SHALL NOT be included in the payload. The installer SHALL carry product version `3.0.0` and identifier `com.codebus.app`.

#### Scenario: build produces installer with both binaries

- **WHEN** `tauri build` runs on Windows with `bundle.active = true` and `bundle.targets = "nsis"`
- **THEN** an NSIS `-setup.exe` is produced whose payload contains the GUI `codebus-app.exe` at the install root and the CLI `codebus.exe` under `bin/`

#### Scenario: test-only binary excluded

- **WHEN** the installer payload is inspected
- **THEN** the `mock-claude` binary is absent from the payload

##### Example: payload contents

| Path in payload | Present | Source |
| --------------- | ------- | ------ |
| `codebus-app.exe` (install root) | yes | GUI, pkg codebus-app-tauri |
| `bin\codebus.exe` | yes | CLI, pkg codebus-cli |
| installer hook script (embedded) | yes | windows/installer-hooks.nsh |
| `mock-claude.exe` | no | test-only, excluded |


<!-- @trace
source: windows-installer-foundation
updated: 2026-06-01
code:
  - codebus-app/src-tauri/tauri.conf.json
  - codebus-app/scripts/stage-cli.mjs
-->

---
### Requirement: Installer adds CLI to per-user PATH and reverses on uninstall

The installer SHALL run per-user (`installMode = "currentUser"`, no Administrator required). On install it SHALL add the CLI's `bin/` directory to the current user's PATH (HKCU) idempotently. On uninstall it SHALL remove ONLY the PATH segment it added. The PATH modification SHALL NOT require Administrator privileges and SHALL NOT write to machine-wide (HKLM) PATH.

#### Scenario: install adds bin directory to user PATH

- **WHEN** the user runs the installer
- **THEN** the install directory's `bin/` is appended to the HKCU PATH and a change broadcast is sent so newly opened shells see it

#### Scenario: install is idempotent

- **WHEN** the installer runs and the target `bin/` segment is already present in HKCU PATH
- **THEN** the installer does not append a duplicate segment

#### Scenario: uninstall removes only the added segment

- **WHEN** the user uninstalls codebus
- **THEN** only the previously added `bin/` segment is removed from HKCU PATH and the rest of PATH is left intact


<!-- @trace
source: windows-installer-foundation
updated: 2026-06-01
code:
  - codebus-app/src-tauri/windows/installer-hooks.nsh
  - codebus-app/src-tauri/tauri.conf.json
-->

---
### Requirement: Installer and uninstaller never touch user data

The installer SHALL NOT read or write the user's `~/.codebus/` directory or any vault's `.codebus/` directory. By default (no explicit user opt-in), uninstall SHALL preserve all user data and SHALL remove only installed program files and the PATH segment the installer added.

The uninstaller SHALL offer the user an explicit opt-in to additionally remove global user data. When, and only when, the user explicitly opts in, uninstall SHALL additionally remove, on a best-effort basis: (a) the user's global `~/.codebus/` directory, (b) the Tauri app data directory for identifier `com.codebus.app` (`%LOCALAPPDATA%\com.codebus.app`), and (c) the Azure API-key keyring entries for both providers' azure profiles (the `codebus-claude-azure` and `codebus-codex-azure` Credential Manager services by default, or the user-configured `keyring_service` overrides). Under NO circumstance — including when the user opts in — SHALL the uninstaller read, traverse, or delete any vault's `.codebus/` directory; vault wikis are never touched.

The opt-in purge SHALL be best-effort: any individual removal step that fails or hangs SHALL NOT block or abort the uninstall. The default-preserve path SHALL remain the behavior whenever the user does not explicitly opt in (including silent or unattended uninstall, which SHALL be treated as no opt-in).

#### Scenario: uninstall preserves user data by default

- **WHEN** the user uninstalls codebus and does not opt in to removing settings and credentials
- **THEN** the user's `~/.codebus/` directory, all vault `.codebus/` directories, the `com.codebus.app` app data, and the keyring credentials all remain untouched on disk, and only program files and the added PATH segment are removed

#### Scenario: opt-in purge removes global user data, credentials, and app data

- **WHEN** the user uninstalls codebus and explicitly opts in to removing settings and credentials
- **THEN** uninstall additionally removes, best-effort, the global `~/.codebus/` directory, the `%LOCALAPPDATA%\com.codebus.app` app data directory, and the azure keyring entries for the claude and codex providers

#### Scenario: vault .codebus directories are never touched even on opt-in purge

- **WHEN** the user opts in to the full purge during uninstall
- **THEN** no vault `.codebus/` directory inside any repository is read, traversed, or deleted

#### Scenario: a failing purge step does not block uninstall

- **WHEN** the user opts in to the purge AND one of the removal steps fails or hangs (for example the keyring backend is unavailable or a directory is locked)
- **THEN** the uninstall continues and completes, and the failing step is silently skipped

##### Example: opt-in purge coverage

| Target | Default (no opt-in) | Opt-in purge |
| ------ | ------------------- | ------------ |
| Program files + added PATH segment | removed | removed |
| `~/.codebus/` (global config + logs) | preserved | removed (best-effort) |
| `%LOCALAPPDATA%\com.codebus.app` (app data) | preserved | removed (best-effort) |
| keyring `codebus-claude-azure` / `codebus-codex-azure` | preserved | removed (best-effort) |
| any vault `.codebus/` (repo wiki) | never touched | never touched |


<!-- @trace
source: windows-uninstaller-opt-in-purge
updated: 2026-06-01
code:
  - codebus-app/src-tauri/windows/installer-hooks.nsh
-->
