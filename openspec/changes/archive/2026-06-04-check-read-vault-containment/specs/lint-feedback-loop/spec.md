## ADDED Requirements

### Requirement: Vault Containment Read Gate

The `codebus hook check-read` subcommand SHALL enforce a vault-root containment boundary on the agent's read target path BEFORE the image / sensitive-path denylist defined in `PII Image Read Hook Installation`. This boundary is the primary read security gate; the denylist is retained only as in-vault defense-in-depth. Containment SHALL apply to the read target path of the `Read`, `Glob`, AND `Grep` tools.

The boundary SHALL be gated by a boolean configuration key `hooks.read_path_containment` in the top-level `hooks` namespace of `~/.codebus/config.yaml`, parallel to `hooks.read_image_block` AND independent of it. The default value SHALL be `true`. Key resolution SHALL be fail-safe: when the config file does not exist, OR fails to parse as YAML, OR the `hooks` section is absent, OR the `read_path_containment` key is absent, OR the key is a non-boolean value, the subcommand SHALL behave as if the key were `true`. The `read_image_block` key SHALL NOT enable or disable containment, AND `read_path_containment` SHALL NOT enable or disable the denylist; the two gates are independent.

The read target path SHALL be resolved from the PreToolUse stdin JSON by `tool_name`: when `tool_name` is `"Read"`, the target path is `tool_input.file_path`; when `tool_name` is `"Glob"` or `"Grep"`, the target path is `tool_input.path`. For `Glob` AND `Grep`, an absent or empty `tool_input.path` SHALL denote the implicit search root (the agent process working directory, which is the vault root) AND SHALL be treated as in-vault — the subcommand SHALL NOT block a `Glob` or `Grep` invocation solely because it omits `tool_input.path`.

When `hooks.read_path_containment` resolves to `true` AND a non-empty target path is present, the subcommand SHALL decide containment by canonical comparison: it SHALL canonicalize the vault root AND canonicalize the target path (resolving a relative target path against the vault root first), applying the same canonicalization to both operands so platform-specific forms (Windows `\\?\` verbatim prefixes, drive-letter case, 8.3 short names, UNC paths, AND `/` versus `\` separators) normalize identically. When the canonicalized target path equals the vault root OR is a descendant of it, the subcommand SHALL allow the path to proceed to the denylist stage. When the canonicalized target path is NOT within the vault root, the subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` whose reason identifies the vault-containment rule AND the blocked path.

When `hooks.read_path_containment` resolves to `false`, the subcommand SHALL NOT perform the containment comparison AND SHALL pass the target path directly to the denylist stage (which remains governed by `hooks.read_image_block`).

The vault root used for containment SHALL be obtained from the PreToolUse stdin `cwd` field (the agent working directory, which codebus sets to the vault root; empirically confirmed to be present AND to equal the vault root), with the hook subprocess working directory as an equivalent fallback when the stdin `cwd` field is absent, AND without introducing a new persistent config field. This requirement constrains the observable containment behavior; the named source is the resolved sourcing mechanism.

#### Scenario: Containment blocks an out-of-vault absolute Read path

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Read","tool_input":{"file_path":"<abs-path-outside-vault>"}}` where the path canonicalizes outside the vault root (e.g., the parent source repository, `~/.kube/config`, or `~/.env`) AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field identifies the vault-containment rule

#### Scenario: Containment blocks an out-of-vault Grep path

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Grep","tool_input":{"pattern":"SECRET","path":"<abs-dir-outside-vault>"}}` where the path canonicalizes outside the vault root AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field identifies the vault-containment rule

#### Scenario: Containment allows an in-vault relative Read path

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Read","tool_input":{"file_path":"raw/code/src/main.rs"}}` resolving under the vault root AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL NOT print a containment block decision AND the path SHALL proceed to the denylist stage

#### Scenario: Containment allows an in-vault absolute path from the fix workflow

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Read","tool_input":{"file_path":"<abs-vault>/wiki/modules/auth.md"}}` where the absolute path lies under the vault root (as produced by the `codebus lint` issue paths the fix workflow consumes verbatim) AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL NOT print a containment block decision AND the path SHALL proceed to the denylist stage

#### Scenario: Glob or Grep omitting path is treated as in-vault

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Grep","tool_input":{"pattern":"foo"}}` OR `{"tool_name":"Glob","tool_input":{"pattern":"**/*.md"}}` with no `path` field AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL NOT print a containment block decision AND SHALL NOT fail closed on the absent `path` (the implicit search root is the vault root)

#### Scenario: Containment disabled passes an out-of-vault path through

- **WHEN** `~/.codebus/config.yaml` contains `hooks.read_path_containment: false` AND `codebus hook check-read` receives stdin JSON whose resolved target path canonicalizes outside the vault root
- **THEN** the subcommand SHALL NOT print a containment block decision (the target path proceeds to the denylist stage, which remains governed by `hooks.read_image_block`)

#### Scenario: Containment key fail-safe resolves to true

- **WHEN** `~/.codebus/config.yaml` does not exist, OR fails to parse as YAML, OR lacks a `hooks` section, OR lacks the `read_path_containment` key, OR sets it to a non-boolean value, AND `codebus hook check-read` receives stdin JSON whose resolved target path canonicalizes outside the vault root
- **THEN** the subcommand SHALL behave as if `hooks.read_path_containment` were `true` AND SHALL print a containment block decision

#### Scenario: In-vault path with Windows separator and drive-case variance is allowed

- **WHEN** on Windows, `codebus hook check-read` receives a target path under the vault root expressed with backslash separators or a differently-cased drive letter than the canonicalized vault root (e.g., target `d:\repo\.codebus\wiki\x.md` against vault root `D:\repo\.codebus`) AND `hooks.read_path_containment` resolves to `true`
- **THEN** the subcommand SHALL treat the path as in-vault AND SHALL NOT print a containment block decision (both operands normalize under one canonicalization)

## MODIFIED Requirements

### Requirement: PII Image Read Hook Installation

The `codebus init` subcommand SHALL ensure `<vault_root>/.claude/settings.json` contains `hooks.PreToolUse` entries whose `matcher` fields equal `"Read"`, `"Glob"`, AND `"Grep"` respectively, each routing to `codebus hook check-read` as a `command`-type hook, in addition to the Bash matcher entry required by `Fix Bash Hook Installation`. The same write-if-missing semantics from `Fix Bash Hook Installation` SHALL apply at the file level: if `<vault_root>/.claude/settings.json` already exists, init SHALL NOT modify it. Existing vaults predating this requirement SHALL be upgraded via release-note guidance (manual JSON snippet insertion or re-init at a new location), NOT by automatic in-place migration; the `Vault Gate Integrity Check` requirement provides the detection signal for such vaults.

The `codebus hook check-read` subcommand SHALL intercept the `Read`, `Glob`, AND `Grep` tools. The read target path SHALL be resolved by `tool_name`: `tool_input.file_path` for `Read`, AND `tool_input.path` for `Glob` or `Grep`. The `Vault Containment Read Gate` requirement SHALL be evaluated FIRST on this target path; the image / sensitive-path denylist defined below SHALL apply only after containment has allowed the path through, AND serves as in-vault defense-in-depth.

The hook entries SHALL be installed unconditionally — their runtime behavior is gated at hook-invocation time by the `hooks.read_image_block` (denylist) AND `hooks.read_path_containment` (containment) config keys, NOT by conditional install-time logic. This SHALL allow `~/.codebus/config.yaml` to be the single source of truth: changing a config key takes immediate effect for all existing vaults without requiring re-init or per-vault edits to `settings.json`.

The `codebus hook check-read` subcommand SHALL read `~/.codebus/config.yaml` at the start of every invocation AND SHALL consult the boolean configuration key `hooks.read_image_block` for the denylist stage. The key resolution rules SHALL be:

- When the config file does not exist, OR the file fails to parse as YAML, OR the `hooks` section is absent, OR the `read_image_block` key is absent, OR the key is a non-boolean value: the subcommand SHALL behave as if the key were `true` (fail-safe to block).
- When `hooks.read_image_block` is the boolean `false`: the subcommand SHALL NOT execute the image-extension blocklist, the sensitive-path blocklist, or the Read fail-closed stdin checks. The denylist stage is skipped; containment is unaffected (governed independently by `hooks.read_path_containment`).
- When `hooks.read_image_block` is the boolean `true`: the subcommand SHALL execute the denylist stdin/stdout contract defined in the following paragraphs (the image extension blocklist, the sensitive-path blocklist, AND the Read fail-closed branches).

When `hooks.read_image_block` resolves to `true`, the denylist stage of `codebus hook check-read` SHALL implement the following stdin/stdout contract on the resolved target path:

- **Input** (stdin): a single JSON object matching Claude Code's PreToolUse hook input schema. The relevant fields are `tool_name` (`"Read"`, `"Glob"`, or `"Grep"`) AND the corresponding target-path field (`tool_input.file_path` for `Read`, `tool_input.path` for `Glob`/`Grep`).
- **Block (image extension)**: when the target path is a non-empty string whose extension matches any of the following blocklist members (compared ASCII case-insensitively after stripping the directory portion using either `/` or `\` as separator): `png`, `jpg`, `jpeg`, `gif`, `webp`, `bmp`, `tiff`, `tif`, `pdf`, `ico`, `heic`, `heif`, `avif`. The subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` identifies the blocked path.
- **Block (sensitive path)**: when the target path, after path-separator normalization to forward-slash AND after expanding a leading `~` to the running user's home directory, starts with any of the following sensitive directory prefixes (compared ASCII case-insensitively): `<home>/.ssh/`, `<home>/.aws/`, `<home>/.gnupg/`, `<home>/.config/gh/`. OR when the path's basename matches (compared ASCII case-insensitively) any of the following glob patterns: `*id_rsa*`, `*.pem`, `*.key`. The subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` whose reason identifies the sensitive-path rule that fired AND the blocked path.
- **Block (unresolvable home)**: when the running environment provides no resolvable home directory (no usable `HOME` on Unix, no usable `USERPROFILE` on Windows) AND the target path requires home resolution to evaluate the sensitive-prefix rule, the subcommand SHALL exit with status zero AND SHALL print a block decision JSON whose reason identifies that the home directory is unresolvable (fail-closed). Paths the basename-glob rule alone can decide (e.g., `/tmp/random/server.pem`) SHALL still be evaluated independently of home resolution.
- **Allow**: when the target path is a non-empty string whose extension is not in the image blocklist AND whose path does not match the sensitive-path rule AND home resolution is not required (or succeeded), the subcommand SHALL exit with status zero AND SHALL NOT print a denylist decision JSON to stdout.
- **Fail-closed (Read missing path)**: when `tool_name` is `"Read"` AND the stdin lacks `tool_input.file_path`, contains a non-string `file_path`, or contains an empty-string `file_path`, the subcommand SHALL exit with status zero AND SHALL print a block decision JSON. A `Glob` or `Grep` invocation that omits `tool_input.path` SHALL NOT be failed closed by this branch (its implicit search root is the vault root, governed by `Vault Containment Read Gate`).
- **Fail-closed (malformed stdin)**: when stdin is empty OR fails to parse as JSON, the subcommand SHALL exit with status zero AND SHALL print a block decision JSON. The subcommand SHALL NEVER silently allow on parse failure.
- **Cross-platform extension comparison**: the extension match SHALL be ASCII case-insensitive on all platforms. The sensitive-path match SHALL likewise be ASCII case-insensitive on all platforms AND SHALL normalize path separators (both `/` and `\`) to forward-slash before prefix comparison, so `C:\Users\harry\.ssh\config` AND `/home/harry/.ssh/config` both trigger the same rule on their respective OS.
- **Path separator handling**: the implementation SHALL strip the directory portion using either `/` or `\` as a path separator before extracting the extension AND before evaluating the basename glob, so `/repo/img.png`, `C:\repo\img.png`, AND `C:/repo/img.png` all extract the same extension AND basename.

The hook installer SHALL emit the `Read`, `Glob`, AND `Grep` matcher entries as siblings of the existing `Bash` matcher entry in the same `hooks.PreToolUse` array, not as a nested structure.

The `hooks.read_image_block` key SHALL belong to a top-level `hooks` namespace in `~/.codebus/config.yaml`, parallel to existing top-level namespaces (`pii`, `lint`, `quiz`, `goal`, `log`, `app`, `claude_code`) AND to the `hooks.read_path_containment` key. The default value SHALL be `true`. The starter config file written by `codebus init` (when no global config exists) SHALL include a documented `hooks` section with `read_image_block: true` AND `read_path_containment: true` AND inline commentary describing each trade-off.

#### Scenario: Init writes Read, Glob, and Grep matcher entries alongside Bash on fresh vault

- **WHEN** `codebus init` runs against a repository with no existing `<vault_root>/.claude/settings.json`
- **THEN** the system SHALL create `<vault_root>/.claude/settings.json` AND the file content SHALL parse as JSON AND the `hooks.PreToolUse` array SHALL contain a Bash matcher entry invoking `codebus hook check-bash` AND `Read`, `Glob`, AND `Grep` matcher entries each invoking `codebus hook check-read`

#### Scenario: Init does not overwrite existing settings.json for Read hook migration

- **WHEN** `codebus init` runs against a vault where `<vault_root>/.claude/settings.json` already exists with a prior matcher set
- **THEN** the system SHALL NOT modify the existing file AND its byte-content SHALL be identical before and after init

#### Scenario: hook check-read blocks blacklisted image extensions

- **WHEN** `codebus hook check-read` receives stdin JSON whose resolved in-vault target path ends with any image blocklist extension AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field is a non-empty string identifying the blocked path

##### Example: extension blocklist coverage

| Input target path | Decision | Notes |
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

- **WHEN** `codebus hook check-read` receives stdin JSON whose resolved in-vault target path is `<vault>/assets/img.png` expressed with `/`, `\`, or mixed separators AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-read allows non-image extensions

- **WHEN** `codebus hook check-read` receives stdin JSON whose resolved in-vault target path is `wiki/modules/uv-lib.md` OR `raw/code/agent/claude_cli.rs` OR `Makefile` AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-read fails closed on missing or invalid Read file_path

- **WHEN** `codebus hook check-read` receives stdin JSON `{"tool_name":"Read","tool_input":{}}` (no `file_path`) OR `{"tool_name":"Read","tool_input":{"file_path":""}}` (empty string) OR `{"tool_name":"Read","tool_input":{"file_path":123}}` (non-string) AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-read fails closed on malformed stdin

- **WHEN** `codebus hook check-read` receives stdin that does not parse as JSON OR stdin that is empty AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (fail-closed default — the subcommand SHALL NEVER silently allow on parse failure)

#### Scenario: hook check-read with read_image_block disabled skips the denylist

- **WHEN** `~/.codebus/config.yaml` contains `hooks.read_image_block: false` AND `codebus hook check-read` receives stdin JSON with any in-vault `tool_input.file_path` value (image extension, sensitive-path hit, text extension, or malformed JSON)
- **THEN** the subcommand SHALL NOT print any denylist `decision` JSON, regardless of the stdin contents (containment, governed independently by `hooks.read_path_containment`, is unaffected by this key)

#### Scenario: Missing config file resolves read_image_block to true (fail-safe block)

- **WHEN** `~/.codebus/config.yaml` does not exist AND `codebus hook check-read` receives stdin JSON whose in-vault `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON to stdout

#### Scenario: Malformed config yaml resolves read_image_block to true (fail-safe block)

- **WHEN** `~/.codebus/config.yaml` contains content that fails to parse as YAML AND `codebus hook check-read` receives stdin JSON whose in-vault `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON to stdout (the hook subcommand SHALL NEVER be made permissive by a config load failure)

#### Scenario: Absent hooks section resolves read_image_block to true

- **WHEN** `~/.codebus/config.yaml` exists AND parses successfully but contains no `hooks` section AND `codebus hook check-read` receives stdin JSON whose in-vault `tool_input.file_path` ends in `.png`
- **THEN** the subcommand SHALL behave as if `hooks.read_image_block` were `true` AND SHALL print a block decision JSON

#### Scenario: hook check-read blocks sensitive key basename inside the vault

- **WHEN** `codebus hook check-read` receives stdin JSON whose resolved in-vault target path has a basename matching `*id_rsa*`, `*.pem`, or `*.key` (e.g., a key file that slipped into `raw/code/`) AND `hooks.read_image_block` resolves to `true`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field mentions the basename-glob rule

### Requirement: Vault Gate Integrity Check

The lint subsystem SHALL verify that the vault PreToolUse gate configuration at `<vault-root>/.claude/settings.json` still installs the hooks codebus relies on to sandbox the claude-path agent: a `Bash` matcher routing to `codebus hook check-bash`, AND `Read`, `Glob`, AND `Grep` matchers each routing to `codebus hook check-read`. This check SHALL read exactly that single file; it SHALL NOT scan, traverse, or read any other path outside the `wiki/` subtree, AND SHALL NOT broaden lint into a general vault-structure validator. The check is a detection signal only — it SHALL NOT modify, restore, or rewrite the settings file (the Lint Read-Only Invariant continues to hold).

The required hook set (the matcher → command pairs `Bash` → `codebus hook check-bash`, `Read` → `codebus hook check-read`, `Glob` → `codebus hook check-read`, AND `Grep` → `codebus hook check-read`) SHALL be sourced from the same definition that `codebus init` uses to author the default settings file, so the linter AND the installer cannot drift.

The check SHALL emit a lint issue with `severity: error` AND the stable kebab-case rule identifier `vault-gate-integrity` when ANY of the following holds: the settings file is absent; the file does not parse as JSON; `hooks.PreToolUse` is missing or is not an array; OR any one of the four required hook entries (`Bash` → check-bash, `Read` → check-read, `Glob` → check-read, `Grep` → check-read) is absent. The issue `message` SHALL identify which condition failed (which required hook is missing, or that the file is absent / unparseable). When ALL four required hook entries are present, the check SHALL emit NO `vault-gate-integrity` issue, regardless of any additional user-added matcher entries, hook commands, or top-level keys present in the file (preserving the write-if-missing user-customization contract).

The issue path for a `vault-gate-integrity` finding SHALL be the settings file location: in `text` format it SHALL render as the vault-relative path `.claude/settings.json` verbatim, WITHOUT the `wiki/` prefix that the text format applies to wiki-subtree issue paths; in `json` format the issue `path` SHALL be the absolute filesystem path of the settings file. This finding SHALL be counted in the `error_count` totals like any other error-severity issue.

#### Scenario: Intact gate produces no issue

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Bash` → `codebus hook check-bash`, `Read` → `codebus hook check-read`, `Glob` → `codebus hook check-read`, AND `Grep` → `codebus hook check-read` PreToolUse hook entries
- **THEN** the lint result SHALL NOT contain any issue whose `rule` is `vault-gate-integrity`

#### Scenario: Emptied PreToolUse array is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` parses as JSON but whose `hooks.PreToolUse` array has been rewritten to empty
- **THEN** the lint result SHALL contain one `error`-severity issue whose `rule` is `vault-gate-integrity` per missing required hook — i.e., four such issues when all of the `Bash`, `Read`, `Glob`, AND `Grep` gates are absent — AND each issue `message` SHALL identify the specific missing gate

#### Scenario: Missing Bash gate hook is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Read`, `Glob`, AND `Grep` check-read entries but not the `Bash` → `codebus hook check-bash` entry
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue whose `message` identifies the missing `Bash` check-bash gate

#### Scenario: Missing Read gate hook is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Bash`, `Glob`, AND `Grep` entries but not the `Read` → `codebus hook check-read` entry
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue whose `message` identifies the missing `Read` check-read gate

#### Scenario: Missing Glob gate hook is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Bash`, `Read`, AND `Grep` entries but not the `Glob` → `codebus hook check-read` entry
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue whose `message` identifies the missing `Glob` check-read gate

#### Scenario: Missing Grep gate hook is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` contains the `Bash`, `Read`, AND `Glob` entries but not the `Grep` → `codebus hook check-read` entry
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue whose `message` identifies the missing `Grep` check-read gate

#### Scenario: User-added settings do not cause a false positive

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` retains all four required hook entries AND also contains additional user-added PreToolUse entries or unrelated top-level keys
- **THEN** the lint result SHALL NOT contain any `vault-gate-integrity` issue

#### Scenario: Absent or unparseable settings file is flagged

- **WHEN** the system runs lint against a vault whose `.claude/settings.json` is absent, OR whose content does not parse as JSON
- **THEN** the lint result SHALL contain a `vault-gate-integrity` error issue

#### Scenario: Gate finding path representation per format

- **WHEN** a `vault-gate-integrity` issue is emitted for a vault rooted at `<abs-vault>/`
- **THEN** in `text` format the issue path SHALL render as `.claude/settings.json` with no `wiki/` prefix AND in `json` format the issue `path` SHALL equal `<abs-vault>/.claude/settings.json` (absolute)

#### Scenario: Gate check never modifies the vault

- **WHEN** the system runs lint against any vault, whether or not the gate is intact
- **THEN** `<vault-root>/.claude/settings.json` SHALL be byte-identical before and after the lint invocation
