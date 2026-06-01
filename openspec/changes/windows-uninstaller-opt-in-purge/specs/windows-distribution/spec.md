## MODIFIED Requirements

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
