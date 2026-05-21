## MODIFIED Requirements

### Requirement: PII Image Read Hook Installation

The `codebus init` subcommand SHALL ensure `<vault_root>/.claude/settings.json` contains a `hooks.PreToolUse` entry whose `matcher` field equals `"Read"` and whose hook command invokes `codebus hook check-read` as a `command`-type hook, in addition to the Bash matcher entry required by `Fix Bash Hook Installation`. The same write-if-missing semantics from `Fix Bash Hook Installation` SHALL apply at the file level: if `<vault_root>/.claude/settings.json` already exists, init SHALL NOT modify it. Existing vaults predating this requirement SHALL be upgraded via release-note guidance (manual JSON snippet insertion or re-init at a new location), NOT by automatic in-place migration.

The hook entry SHALL be installed unconditionally — its runtime behavior is gated at hook-invocation time by the `hooks.read_image_block` config key defined in this requirement, NOT by conditional install-time logic. This SHALL allow `~/.codebus/config.yaml` to be the single source of truth: changing the config key takes immediate effect for all existing vaults without requiring re-init or per-vault edits to `settings.json`.

The `codebus hook check-read` subcommand SHALL read `~/.codebus/config.yaml` at the start of every invocation and SHALL consult a boolean configuration key `hooks.read_image_block`. The key resolution rules SHALL be:

- When the config file does not exist, OR the file fails to parse as YAML, OR the `hooks` section is absent, OR the `read_image_block` key is absent, OR the key is a non-boolean value: the subcommand SHALL behave as if the key were `true` (fail-safe to block).
- When `hooks.read_image_block` is the boolean `false`: the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON to stdout, regardless of the stdin contents or `tool_input.file_path` extension. The subcommand SHALL NOT execute the blocklist comparison or the fail-closed stdin checks in this branch.
- When `hooks.read_image_block` is the boolean `true`: the subcommand SHALL execute the stdin/stdout contract defined in the following paragraphs (the contract is unchanged from before this modification).

When `hooks.read_image_block` resolves to `true`, the `codebus hook check-read` subcommand SHALL implement the following stdin/stdout contract:

- **Input** (stdin): a single JSON object matching Claude Code's PreToolUse hook input schema. The relevant fields are `tool_name` (expected `"Read"`) and `tool_input.file_path` (the absolute or vault-relative file path the agent intends to read).
- **Block**: when `tool_input.file_path` is a non-empty string whose extension matches any of the following blocklist members (compared ASCII case-insensitively after stripping the directory portion using either `/` or `\` as separator): `png`, `jpg`, `jpeg`, `gif`, `webp`, `bmp`, `tiff`, `tif`, `pdf`, `ico`, `heic`, `heif`, `avif`. The subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` identifies the blocked path.
- **Allow**: when `tool_input.file_path` is a non-empty string whose extension is not in the blocklist (including no extension, `.md`, `.rs`, `.svg`, `.txt`, `.json`, `.yaml`, `.toml`, and any other text-bearing extension), the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON to stdout.
- **Fail-closed**: when stdin is empty, fails to parse as JSON, lacks `tool_input.file_path`, contains a non-string `file_path`, or contains an empty-string `file_path`, the subcommand SHALL exit with status zero AND SHALL print a block decision JSON. The subcommand SHALL NEVER silently allow on parse failure.
- **Cross-platform extension comparison**: the extension match SHALL be ASCII case-insensitive on all platforms (Windows, macOS, Linux). This SHALL deliberately diverge from the OS-split case-sensitivity used by `is_codebus_binary` in `Fix Bash Hook Installation`, because file extensions are conventionally case-insensitive across all operating systems and a POSIX case-sensitive match would let `screenshot.PNG` bypass the blocklist on Linux.
- **Path separator handling**: the implementation SHALL strip the directory portion using either `/` or `\` as a path separator before extracting the extension, so `/repo/img.png`, `C:\repo\img.png`, and `C:/repo/img.png` all extract the same extension.

The hook installer SHALL emit the Read matcher entry as a sibling of the existing Bash matcher entry in the same `hooks.PreToolUse` array, not as a nested structure.

The `hooks.read_image_block` key SHALL belong to a top-level `hooks` namespace in `~/.codebus/config.yaml`, parallel to existing top-level namespaces (`pii`, `lint`, `quiz`, `goal`, `log`, `app`, `claude_code`). The default value SHALL be `true`. The starter config file written by `codebus init` (when no global config exists) SHALL include a documented `hooks` section with `read_image_block: true` and inline commentary describing the trade-off.

#### Scenario: Init writes Read matcher entry alongside Bash entry on fresh vault

- **WHEN** `codebus init` runs against a repository with no existing `<vault_root>/.claude/settings.json`
- **THEN** the system SHALL create `<vault_root>/.claude/settings.json` AND the file content SHALL parse as JSON AND the `hooks.PreToolUse` array SHALL contain both a Bash matcher entry invoking `codebus hook check-bash` AND a Read matcher entry invoking `codebus hook check-read`

#### Scenario: Init does not overwrite existing settings.json for Read hook migration

- **WHEN** `codebus init` runs against a vault where `<vault_root>/.claude/settings.json` already exists with only the Bash matcher entry (from a prior init)
- **THEN** the system SHALL NOT modify the existing file AND its byte-content SHALL be identical before and after init

#### Scenario: hook check-read blocks blacklisted image extensions

- **WHEN** `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` ends with any blocklist extension AND `hooks.read_image_block` resolves to `true`
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

- **WHEN** `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` is `/repo/assets/img.png` OR `C:\repo\assets\img.png` OR `C:/repo/assets/img.png` AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-read allows non-image extensions

- **WHEN** `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` is `wiki/modules/uv-lib.md` OR `codebus-core/src/agent/claude_cli.rs` OR `Makefile` AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-read fails closed on missing or invalid file_path

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Read","tool_input":{}}` (no `file_path` field) OR `{"tool_name":"Read","tool_input":{"file_path":""}}` (empty string) OR `{"tool_name":"Read","tool_input":{"file_path":123}}` (non-string value) AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-read fails closed on malformed stdin

- **WHEN** `codebus hook check-read` receives stdin that does not parse as JSON OR stdin that is empty AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (fail-closed default — the subcommand SHALL NEVER silently allow on parse failure)

#### Scenario: hook check-read with read_image_block disabled allows all reads

- **WHEN** `~/.codebus/config.yaml` contains `hooks.read_image_block: false` AND `codebus hook check-read` receives stdin JSON with any `tool_input.file_path` value (image extension, text extension, or even malformed JSON)
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON, regardless of the stdin contents

#### Scenario: Missing config file resolves read_image_block to true (fail-safe block)

- **WHEN** `~/.codebus/config.yaml` does not exist AND `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON to stdout

#### Scenario: Malformed config yaml resolves read_image_block to true (fail-safe block)

- **WHEN** `~/.codebus/config.yaml` contains content that fails to parse as YAML AND `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON to stdout (the hook subcommand SHALL NEVER be made permissive by a config load failure)

#### Scenario: Absent hooks section resolves read_image_block to true

- **WHEN** `~/.codebus/config.yaml` exists and parses successfully but contains no `hooks` section AND `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON (preserves pre-toggle default behavior — no migration friction)
