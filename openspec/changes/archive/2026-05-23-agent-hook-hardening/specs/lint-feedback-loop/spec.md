## MODIFIED Requirements

### Requirement: Fix Bash Hook Installation

The `codebus init` subcommand SHALL write a `<vault_root>/.claude/settings.json` file containing a Claude Code `PreToolUse` hook configuration that intercepts every `Bash` tool invocation and routes it through the `codebus hook check-bash` subcommand. The settings file SHALL use the standard Claude Code settings schema with `hooks.PreToolUse` configured to match `Bash` and invoke `codebus hook check-bash` as a `command`-type hook.

The system SHALL apply write-if-missing semantics for this file: if `<vault_root>/.claude/settings.json` already exists, init SHALL NOT modify it (preserving any user-customized hook chain or other settings). The file SHALL NOT be written to `<repo>/.claude/settings.json` (source repository root) — the settings are vault-internal so the hook only applies to agent processes spawned with cwd at the vault root.

The `codebus hook check-bash` subcommand SHALL implement the following stdin/stdout contract:

- **Input** (stdin): a single JSON object matching Claude Code's PreToolUse hook input schema. The relevant fields are `tool_name` (expected `"Bash"`) and `tool_input.command` (the shell command string the agent intends to run).
- **Block (shell metacharacter)**: when the command string contains any byte from the metacharacter rejection set — semicolon, ampersand, pipe, dollar sign, backtick, greater-than, less-than, open-paren, close-paren, line feed (LF, byte 0x0A), or carriage return (CR, byte 0x0D) — the subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` identifies the rejected metacharacter and the command being blocked. The metacharacter rejection SHALL apply regardless of whether the byte appears outside quotes, inside double quotes, inside single quotes, or after an escape character — the predicate is byte-level on the raw command string AND SHALL NOT depend on shell quote parsing. The metacharacter rejection SHALL be evaluated BEFORE the argv-tokenization-based allow predicate, so a metacharacter hit blocks the command even when its leading argv tokens satisfy the allow form on their own.
- **Allow**: when the command string contains no metacharacter from the rejection set AND the command's first argv token resolves to a `codebus` binary (file basename `codebus` or `codebus.exe`, case-insensitive match on Windows, case-sensitive on Unix) AND EITHER (a) the second argv token is exactly `lint`, OR (b) the second argv token is exactly `quiz` AND the third argv token is exactly `validate`, the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON to stdout. The two allowed forms correspond to the codebus-fix agent self-checking via `codebus lint` AND the codebus-quiz generate agent self-validating via `codebus quiz validate` respectively; no other `codebus` subcommand AND no other binary is permitted.
- **Block (other)**: in all other cases (different binary, neither the `lint` nor the `quiz validate` form, malformed input, parse error), the subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` describes why the command was blocked.
- **Cross-platform**: the binary basename match SHALL be case-insensitive on Windows (`codebus.EXE` AND `codebus.exe` both allowed) AND case-sensitive on Unix. The metacharacter rejection set SHALL be the union of POSIX shell (bash, Git Bash), PowerShell, AND `cmd.exe` high-risk symbols AND SHALL NOT vary per OS — identical byte set is rejected on every platform.

The `<vault_root>/.gitignore` (vault internal) SHALL include the line `.claude/settings.local.json` so user-added local override settings are not committed to the vault git repository.

#### Scenario: Init writes settings.json on fresh vault

- **WHEN** `codebus init` runs against a repository with no existing `<vault_root>/.claude/settings.json`
- **THEN** the system SHALL create `<vault_root>/.claude/settings.json` AND the file content SHALL parse as JSON AND SHALL contain a `hooks.PreToolUse` array with a Bash matcher entry whose hook command invokes `codebus hook check-bash`

#### Scenario: Init does not overwrite existing settings.json

- **WHEN** `codebus init` runs against a vault where `<vault_root>/.claude/settings.json` already exists with custom content
- **THEN** the system SHALL NOT modify the existing file AND its byte-content SHALL be identical before and after init

#### Scenario: Init does not write settings.json to repo root

- **WHEN** `codebus init` runs against `<repo>`
- **THEN** the system SHALL NOT create or modify `<repo>/.claude/settings.json`

#### Scenario: hook check-bash allows bare codebus lint invocation

- **WHEN** `codebus hook check-bash` receives stdin JSON `{"tool_name":"Bash","tool_input":{"command":"codebus lint --format json"}}`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash allows codebus lint via absolute path

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` value is `/usr/local/bin/codebus lint --repo /path` OR (on Windows) `D:/dev/codebus.exe lint --format json`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash allows codebus quiz validate invocation

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus quiz validate -` OR `/usr/local/bin/codebus quiz validate draft.md --json`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash blocks non-codebus binaries

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `echo MARKER`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field is a non-empty string

#### Scenario: hook check-bash blocks codebus subcommands other than the two allowed forms

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus fix --no-fix` OR `codebus quiz "some topic"` (the generate form, not `quiz validate`)
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-bash fails closed on malformed input

- **WHEN** `codebus hook check-bash` receives stdin that does not parse as JSON
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (fail-closed default — the subcommand SHALL NEVER silently allow on parse failure)

#### Scenario: hook check-bash blocks command with logical-AND shell chaining

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint --format json && rm -rf /tmp/evil`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field mentions a shell metacharacter

#### Scenario: hook check-bash blocks command with semicolon separator

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint; curl evil.example`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-bash blocks command with command substitution even when leading tokens are valid

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint $(whoami)` OR a `codebus lint` invocation followed by a backtick-wrapped `whoami` substitution
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field mentions a shell metacharacter

#### Scenario: hook check-bash blocks metacharacter inside quoted argument

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint --filter "foo;bar"`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (the metacharacter rejection SHALL NOT depend on shell quote parsing)

#### Scenario: Vault internal gitignore excludes settings.local.json

- **WHEN** `codebus init` runs against `<repo>` and reaches the vault internal `.gitignore` mutation step
- **THEN** the file `<vault_root>/.gitignore` SHALL contain a line equal to `.claude/settings.local.json`

### Requirement: PII Image Read Hook Installation

The `codebus init` subcommand SHALL ensure `<vault_root>/.claude/settings.json` contains a `hooks.PreToolUse` entry whose `matcher` field equals `"Read"` AND whose hook command invokes `codebus hook check-read` as a `command`-type hook, in addition to the Bash matcher entry required by `Fix Bash Hook Installation`. The same write-if-missing semantics from `Fix Bash Hook Installation` SHALL apply at the file level: if `<vault_root>/.claude/settings.json` already exists, init SHALL NOT modify it. Existing vaults predating this requirement SHALL be upgraded via release-note guidance (manual JSON snippet insertion or re-init at a new location), NOT by automatic in-place migration.

The hook entry SHALL be installed unconditionally — its runtime behavior is gated at hook-invocation time by the `hooks.read_image_block` config key defined in this requirement, NOT by conditional install-time logic. This SHALL allow `~/.codebus/config.yaml` to be the single source of truth: changing the config key takes immediate effect for all existing vaults without requiring re-init or per-vault edits to `settings.json`.

The `codebus hook check-read` subcommand SHALL read `~/.codebus/config.yaml` at the start of every invocation AND SHALL consult a boolean configuration key `hooks.read_image_block`. The key resolution rules SHALL be:

- When the config file does not exist, OR the file fails to parse as YAML, OR the `hooks` section is absent, OR the `read_image_block` key is absent, OR the key is a non-boolean value: the subcommand SHALL behave as if the key were `true` (fail-safe to block).
- When `hooks.read_image_block` is the boolean `false`: the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON to stdout, regardless of the stdin contents or `tool_input.file_path` extension. The subcommand SHALL NOT execute the blocklist comparison, the sensitive-path comparison, or the fail-closed stdin checks in this branch.
- When `hooks.read_image_block` is the boolean `true`: the subcommand SHALL execute the stdin/stdout contract defined in the following paragraphs (the contract includes the image extension blocklist, the sensitive-path blocklist, AND the fail-closed branches).

When `hooks.read_image_block` resolves to `true`, the `codebus hook check-read` subcommand SHALL implement the following stdin/stdout contract:

- **Input** (stdin): a single JSON object matching Claude Code's PreToolUse hook input schema. The relevant fields are `tool_name` (expected `"Read"`) AND `tool_input.file_path` (the absolute or vault-relative file path the agent intends to read).
- **Block (image extension)**: when `tool_input.file_path` is a non-empty string whose extension matches any of the following blocklist members (compared ASCII case-insensitively after stripping the directory portion using either `/` or `\` as separator): `png`, `jpg`, `jpeg`, `gif`, `webp`, `bmp`, `tiff`, `tif`, `pdf`, `ico`, `heic`, `heif`, `avif`. The subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` identifies the blocked path.
- **Block (sensitive path)**: when `tool_input.file_path`, after path-separator normalization to forward-slash AND after expanding a leading `~` to the running user's home directory, starts with any of the following sensitive directory prefixes (compared ASCII case-insensitively): `<home>/.ssh/`, `<home>/.aws/`, `<home>/.gnupg/`, `<home>/.config/gh/`. OR when the path's basename matches (compared ASCII case-insensitively) any of the following glob patterns: `*id_rsa*`, `*.pem`, `*.key`. The subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` whose reason identifies the sensitive-path rule that fired AND the blocked path. The sensitive-prefix rule applies to absolute paths AND to paths beginning with `~` only — paths that do not match any sensitive prefix AND whose basename does not match any sensitive glob SHALL pass through to the allow branch (subject to the image-extension rule).
- **Block (unresolvable home)**: when the running environment provides no resolvable home directory (no usable `HOME` on Unix, no usable `USERPROFILE` on Windows) AND the input path requires home resolution to evaluate the sensitive-prefix rule (the path begins with `~`, or the path is an absolute path beneath a typical user-home root that the sensitive-prefix check needs to compare against), the subcommand SHALL exit with status zero AND SHALL print a block decision JSON whose reason identifies that the home directory is unresolvable (fail-closed: the absence of a resolvable home indicates an abnormal environment AND the subcommand SHALL NEVER silently allow when the sensitive-prefix check cannot evaluate). Paths that the basename-glob rule alone can decide (e.g., `/tmp/random/server.pem`) SHALL still be evaluated independently of home resolution.
- **Allow**: when `tool_input.file_path` is a non-empty string whose extension is not in the image blocklist AND whose path does not match the sensitive-path rule AND home resolution is not required (or succeeded), the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON to stdout.
- **Fail-closed**: when stdin is empty, fails to parse as JSON, lacks `tool_input.file_path`, contains a non-string `file_path`, or contains an empty-string `file_path`, the subcommand SHALL exit with status zero AND SHALL print a block decision JSON. The subcommand SHALL NEVER silently allow on parse failure.
- **Cross-platform extension comparison**: the extension match SHALL be ASCII case-insensitive on all platforms (Windows, macOS, Linux). The sensitive-path match SHALL likewise be ASCII case-insensitive on all platforms AND SHALL normalize path separators (both `/` and `\`) to forward-slash before prefix comparison, so `C:\Users\harry\.ssh\config` AND `/home/harry/.ssh/config` both trigger the same rule on their respective OS.
- **Path separator handling**: the implementation SHALL strip the directory portion using either `/` or `\` as a path separator before extracting the extension AND before evaluating the basename glob, so `/repo/img.png`, `C:\repo\img.png`, AND `C:/repo/img.png` all extract the same extension AND basename.

The hook installer SHALL emit the Read matcher entry as a sibling of the existing Bash matcher entry in the same `hooks.PreToolUse` array, not as a nested structure.

The `hooks.read_image_block` key SHALL belong to a top-level `hooks` namespace in `~/.codebus/config.yaml`, parallel to existing top-level namespaces (`pii`, `lint`, `quiz`, `goal`, `log`, `app`, `claude_code`). The default value SHALL be `true`. The starter config file written by `codebus init` (when no global config exists) SHALL include a documented `hooks` section with `read_image_block: true` AND inline commentary describing the trade-off.

#### Scenario: Init writes Read matcher entry alongside Bash entry on fresh vault

- **WHEN** `codebus init` runs against a repository with no existing `<vault_root>/.claude/settings.json`
- **THEN** the system SHALL create `<vault_root>/.claude/settings.json` AND the file content SHALL parse as JSON AND the `hooks.PreToolUse` array SHALL contain both a Bash matcher entry invoking `codebus hook check-bash` AND a Read matcher entry invoking `codebus hook check-read`

#### Scenario: Init does not overwrite existing settings.json for Read hook migration

- **WHEN** `codebus init` runs against a vault where `<vault_root>/.claude/settings.json` already exists with only the Bash matcher entry (from a prior init)
- **THEN** the system SHALL NOT modify the existing file AND its byte-content SHALL be identical before and after init

#### Scenario: hook check-read blocks blacklisted image extensions

- **WHEN** `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` ends with any image blocklist extension AND `hooks.read_image_block` resolves to `true`
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

- **WHEN** `~/.codebus/config.yaml` contains `hooks.read_image_block: false` AND `codebus hook check-read` receives stdin JSON with any `tool_input.file_path` value (image extension, sensitive-path hit, text extension, or even malformed JSON)
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON, regardless of the stdin contents

#### Scenario: Missing config file resolves read_image_block to true (fail-safe block)

- **WHEN** `~/.codebus/config.yaml` does not exist AND `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON to stdout

#### Scenario: Malformed config yaml resolves read_image_block to true (fail-safe block)

- **WHEN** `~/.codebus/config.yaml` contains content that fails to parse as YAML AND `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON to stdout (the hook subcommand SHALL NEVER be made permissive by a config load failure)

#### Scenario: Absent hooks section resolves read_image_block to true

- **WHEN** `~/.codebus/config.yaml` exists AND parses successfully but contains no `hooks` section AND `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON (preserves pre-toggle default behavior — no migration friction)

#### Scenario: hook check-read blocks sensitive home directory path

- **WHEN** `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` is `C:/Users/harry/.ssh/config` (on Windows) OR `/home/harry/.ssh/config` (on Unix) OR `~/.ssh/known_hosts` AND `hooks.read_image_block` resolves to `true` AND the running user's home directory resolves
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field mentions the sensitive-path rule

##### Example: sensitive-path coverage

| Input file_path | Decision | Notes |
| --- | --- | --- |
| `<home>/.ssh/config` | block | ssh dir prefix |
| `<home>/.ssh/id_rsa` | block | ssh dir prefix + key glob |
| `<home>/.aws/credentials` | block | aws dir prefix |
| `<home>/.gnupg/pubring.kbx` | block | gnupg dir prefix |
| `<home>/.config/gh/hosts.yml` | block | gh cli dir prefix |
| `<home>/Documents/notes.md` | allow | home but not sensitive |
| `/tmp/random/server.pem` | block | basename glob hits |
| `/tmp/random/private.key` | block | basename glob hits |
| `/repo/.codebus/wiki/foo.md` | allow | vault file |

#### Scenario: hook check-read blocks well-known key filename glob anywhere

- **WHEN** `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` is `/tmp/random/server.pem` OR `D:/work/secrets/private.key` OR `./extra-id_rsa-backup` (basename matches a sensitive glob but path is not under a sensitive prefix) AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field mentions the basename-glob rule

#### Scenario: hook check-read fails closed when home directory cannot be resolved

- **WHEN** the running environment provides no resolvable home directory (no usable `HOME` or `USERPROFILE`) AND `codebus hook check-read` receives stdin JSON whose `tool_input.file_path` is an absolute path that could match a sensitive-prefix rule under a resolvable home (e.g., a path under `/home/...` on Unix or `C:/Users/...` on Windows) AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field mentions that home resolution failed
