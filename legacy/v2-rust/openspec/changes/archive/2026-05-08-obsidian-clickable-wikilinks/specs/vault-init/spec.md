## ADDED Requirements

### Requirement: Auto-register .codebus/wiki/ as Obsidian vault on init

The system SHALL register `<repo>/.codebus/wiki/` as an Obsidian vault by writing into the user-level `obsidian.json` file during the init flow (after `.codebus/` skeleton creation, before lint and PII filter steps). The registered vault entry SHALL contain `path` (absolute, OS-native separators), `ts` (current Unix milliseconds), and `open: false`. The vault id SHALL be the lowercase 16-hex prefix of `SHA-256(absolute_path.to_lowercase())`.

The registration SHALL skip cleanly without aborting init in any of the following conditions:

- The Obsidian config directory does not exist (Obsidian is not installed on this system).
- An Obsidian process is currently running (detected via OS-specific process listing).
- The user passed `--no-obsidian-register` on the codebus CLI.
- Writing to `obsidian.json` fails for any I/O reason (permission denied, disk full, etc.); the system SHALL log a warning and continue init.

When skipping, the system SHALL emit a single hint line to stderr explaining why and what the user can do (e.g., manually add the vault in Obsidian's UI). When skipping due to "Obsidian is running", the hint SHALL state that closing Obsidian and re-running `codebus --repo <X>` will retry.

The cross-OS resolution of `obsidian.json` SHALL be:

- Windows: `%APPDATA%\obsidian\obsidian.json`
- macOS: `~/Library/Application Support/obsidian/obsidian.json`
- Linux: `~/.config/obsidian/obsidian.json`

#### Scenario: Fresh init writes new vault entry

- **WHEN** the user runs `codebus --repo X` for the first time, Obsidian is installed (`obsidian.json` exists with `{"vaults":{}}`), Obsidian is not running, and `--no-obsidian-register` is not set
- **THEN** `obsidian.json` is updated to contain a vault entry whose key is `SHA-256(abs_path.lowercase())[:16]`, `path` is the absolute path to `<X>/.codebus/wiki`, `ts` is the current Unix milliseconds, and `open` is false; init completes successfully

#### Scenario: Obsidian not installed silently skips

- **WHEN** the user runs `codebus --repo X` and the Obsidian config directory does not exist on the filesystem
- **THEN** the system skips Obsidian registration without printing an error, and init completes normally

#### Scenario: Obsidian running emits hint and skips

- **WHEN** the user runs `codebus --repo X`, `obsidian.json` exists, but an Obsidian process is detected
- **THEN** the system skips writing `obsidian.json`, emits a single stderr line containing the substrings "Obsidian" and "running" and instructing the user to close Obsidian and re-run, and init completes successfully

#### Scenario: --no-obsidian-register opt-out skips

- **WHEN** the user runs `codebus --repo X --no-obsidian-register`
- **THEN** the system does not call any Obsidian registration code path, `obsidian.json` is not read or written, and init completes normally

#### Scenario: Existing same-path entry reuses its id

- **WHEN** the user runs `codebus --repo X` and `obsidian.json` already contains a vault entry whose `path` equals `<X>/.codebus/wiki` (case-insensitive on Windows) but whose key is a different id (e.g., user previously added the vault manually in Obsidian, producing a random id)
- **THEN** the system reuses the existing entry's id (not the SHA-256 id) and only updates the entry's `ts` field; the vault list does not gain a duplicate entry

#### Scenario: I/O error during write logs warning and continues

- **WHEN** the user runs `codebus --repo X` and writing to `obsidian.json` fails (permission denied, disk full, etc.)
- **THEN** the system logs a warning containing the error reason, does not abort init, and the rest of the init flow (lint, PII setup, etc.) proceeds normally

### Requirement: Resolve effective vault id for hyperlink emission

The system SHALL expose the effective vault id (the actual key of the registered vault entry in `obsidian.json` after auto-registration) via the goal / query / fix flow so that the terminal renderer can construct OSC 8 hyperlink URIs targeting the correct vault. The effective id SHALL be the id used by the registered entry (which may be the codebus-computed SHA-256 id, an existing user-created random id when reusing a same-path entry, or `None` when registration was skipped).

When registration was skipped (Obsidian not installed, Obsidian running, opt-out flag, or I/O error), the effective id SHALL be `None`. The renderer SHALL treat `None` as "do not emit hyperlinks" (consistent with the terminal-output spec).

#### Scenario: Successful registration returns SHA-256 id

- **WHEN** registration writes a fresh entry with key `a38bcac8afd70c5e`
- **THEN** the goal / query / fix flow injects `vault_id: Some("a38bcac8afd70c5e")` into `RenderOptions`

#### Scenario: Same-path reuse returns existing id

- **WHEN** registration finds and reuses an existing entry whose key is `0bc358f7cc0d4f29` (a user-created random id) for the same path
- **THEN** the flow injects `vault_id: Some("0bc358f7cc0d4f29")` into `RenderOptions`

#### Scenario: Skipped registration returns None

- **WHEN** registration is skipped for any reason (Obsidian running, not installed, opt-out, I/O error)
- **THEN** the flow injects `vault_id: None` into `RenderOptions`, and the renderer emits no OSC 8 hyperlinks
