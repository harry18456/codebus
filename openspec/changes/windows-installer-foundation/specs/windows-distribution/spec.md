## ADDED Requirements

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

### Requirement: Installer and uninstaller never touch user data

The installer and uninstaller SHALL NOT read or write the user's `~/.codebus/` directory or any vault's `.codebus/` directory. Uninstall SHALL preserve all user data by default and SHALL remove only installed program files and the PATH segment the installer added.

#### Scenario: uninstall preserves user data

- **WHEN** the user uninstalls codebus
- **THEN** the user's `~/.codebus/` directory and all vault `.codebus/` directories remain untouched on disk
