## ADDED Requirements

### Requirement: PII Image Read Hook Installation

The `codebus init` subcommand SHALL ensure `<vault_root>/.claude/settings.json` contains a `hooks.PreToolUse` entry whose `matcher` field equals `"Read"` and whose hook command invokes `codebus hook check-read` as a `command`-type hook, in addition to the Bash matcher entry required by `Fix Bash Hook Installation`. The same write-if-missing semantics from `Fix Bash Hook Installation` SHALL apply at the file level: if `<vault_root>/.claude/settings.json` already exists, init SHALL NOT modify it. Existing vaults predating this requirement SHALL be upgraded via release-note guidance (manual JSON snippet insertion or re-init at a new location), NOT by automatic in-place migration.

The `codebus hook check-read` subcommand SHALL implement the following stdin/stdout contract:

- **Input** (stdin): a single JSON object matching Claude Code's PreToolUse hook input schema. The relevant fields are `tool_name` (expected `"Read"`) and `tool_input.file_path` (the absolute or vault-relative file path the agent intends to read).
- **Block**: when `tool_input.file_path` is a non-empty string whose extension matches any of the following blocklist members (compared ASCII case-insensitively after stripping the directory portion using either `/` or `\` as separator): `png`, `jpg`, `jpeg`, `gif`, `webp`, `bmp`, `tiff`, `tif`, `pdf`, `ico`, `heic`, `heif`, `avif`. The subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` identifies the blocked path.
- **Allow**: when `tool_input.file_path` is a non-empty string whose extension is not in the blocklist (including no extension, `.md`, `.rs`, `.svg`, `.txt`, `.json`, `.yaml`, `.toml`, and any other text-bearing extension), the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON to stdout.
- **Fail-closed**: when stdin is empty, fails to parse as JSON, lacks `tool_input.file_path`, contains a non-string `file_path`, or contains an empty-string `file_path`, the subcommand SHALL exit with status zero AND SHALL print a block decision JSON. The subcommand SHALL NEVER silently allow on parse failure.
- **Cross-platform extension comparison**: the extension match SHALL be ASCII case-insensitive on all platforms (Windows, macOS, Linux). This SHALL deliberately diverge from the OS-split case-sensitivity used by `is_codebus_binary` in `Fix Bash Hook Installation`, because file extensions are conventionally case-insensitive across all operating systems and a POSIX case-sensitive match would let `screenshot.PNG` bypass the blocklist on Linux.
- **Path separator handling**: the implementation SHALL strip the directory portion using either `/` or `\` as a path separator before extracting the extension, so `/repo/img.png`, `C:\repo\img.png`, and `C:/repo/img.png` all extract the same extension.

The hook installer SHALL emit the Read matcher entry as a sibling of the existing Bash matcher entry in the same `hooks.PreToolUse` array, not as a nested structure.

#### Scenario: Init writes Read matcher entry alongside Bash entry on fresh vault

- **WHEN** `codebus init` runs against a repository with no existing `<vault_root>/.claude/settings.json`
- **THEN** the system SHALL create `<vault_root>/.claude/settings.json` AND the file content SHALL parse as JSON AND the `hooks.PreToolUse` array SHALL contain both a Bash matcher entry invoking `codebus hook check-bash` AND a Read matcher entry invoking `codebus hook check-read`

#### Scenario: Init does not overwrite existing settings.json for Read hook migration

- **WHEN** `codebus init` runs against a vault where `<vault_root>/.claude/settings.json` already exists with only the Bash matcher entry (from a prior init)
- **THEN** the system SHALL NOT modify the existing file AND its byte-content SHALL be identical before and after init

#### Scenario: hook check-read blocks blacklisted image extensions

- **WHEN** `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` ends with any blocklist extension
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field is a non-empty string identifying the blocked path

##### Example: extension blocklist coverage

| Input file_path | Decision | Notes |
| --- | --- | --- |
| `wiki/diagrams/flow.png` | block | image |
| `assets/logo.JPG` | block | case-insensitive match |
| `docs/manual.pdf` | block | pdf |
| `art/sprite.avif` | block | newer image format |
| `photo.HEIC` | block | uppercase iOS format |
| `wiki/foo.md` | allow | text |
| `src/main.rs` | allow | source code |
| `wiki/diagram.svg` | allow | xml text, scannable by regex_basic |
| `Makefile` | allow | no extension |
| `script` | allow | no extension |

#### Scenario: hook check-read blocks across path separator styles

- **WHEN** `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` is `/repo/assets/img.png` OR `C:\repo\assets\img.png` OR `C:/repo/assets/img.png`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-read allows non-image extensions

- **WHEN** `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` is `wiki/modules/uv-lib.md` OR `codebus-core/src/agent/claude_cli.rs` OR `Makefile`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-read fails closed on missing or invalid file_path

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Read","tool_input":{}}` (no `file_path` field) OR `{"tool_name":"Read","tool_input":{"file_path":""}}` (empty string) OR `{"tool_name":"Read","tool_input":{"file_path":123}}` (non-string value)
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-read fails closed on malformed stdin

- **WHEN** `codebus hook check-read` receives stdin that does not parse as JSON OR stdin that is empty
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (fail-closed default — the subcommand SHALL NEVER silently allow on parse failure)
