## Why

F4 與威脅 C 是 PR #1 review 抓到、已記錄但未實作的兩個 latent 安全 issue。F4 是 `codebus hook check-bash` 的 allow predicate 只 token 比對前 2-3 token，shell metacharacter 串接（`&&`、`;`、`$()` 等）可繞過 sandbox。威脅 C 是 `codebus hook check-read` 只擋 image 副檔名，可被 agent 用來讀 `~/.ssh/`、`~/.aws/` 等使用者敏感檔。Windows codex PoC（2026-05-23）進一步 confirm 同樣的 read 威脅在 codex path 也存在——codex `workspace-write` 設計上允許讀 workspace 外任意檔——需要 soft constraint 文字配套。修這兩個 hook 並補 codex AGENTS.md 可以一次把 `lint-feedback-loop` 與 `skill-bundles` 兩條 spec 的 sandbox 條款補完整，避免 spec 跟 code 各拖一條 round-trip。

## What Changes

- **Bash hook（F4 + D5）**：`hook.rs::check_bash` 在現有 `is_allowed_bash_command` 內加 raw command 字串的 shell metacharacter 拒絕（黑名單 `; & | $ \` > < ( ) \n \r`）。`lint-feedback-loop` spec 的 Fix Bash Hook Installation 條款補對應 Allow precondition 與新增 3 個 Scenario。
- **Read hook（威脅 C-claude）**：`hook.rs::check_read` 在現有 image extension blocklist 之外新增敏感路徑黑名單（家目錄下 `.ssh/`、`.aws/`、`.gnupg/`、`.config/gh/`，與 glob `*id_rsa*`、`*.pem`、`*.key`）。`lint-feedback-loop` spec 的 PII Image Read Hook Installation 條款補對應 path blocklist 條款與新增 3 個 Scenario。路徑判斷與既有的 image extension 一樣 ASCII case-insensitive，並同時處理 `/` 與 `\` 兩種 separator。`~` / `%USERPROFILE%` 解析以 hook subcommand 啟動時的 home 目錄為準，未解析到 home 時 fail-closed（block）。
- **Codex AGENTS.md（威脅 C-codex soft constraint）**：`skill_bundle/mod.rs` 中 codex AGENTS.md 模板加一段聲明：codex sandbox 設計上允許讀 workspace 外任意檔，但 codebus agent 工作範圍僅限 vault，不主動讀使用者 home 下的敏感檔（`~/.ssh/`、`~/.aws/`、`~/.gnupg/` 等）。`skill-bundles` spec 的 Codex Instruction Materialization 條款補對應材料化內容契約（hard-coded literal 段落，與 SKILL.md 同 byte-identical 雙寫精神）。

## Non-Goals (optional)

- 不動 codex path 的架構級隔離（`writable_roots` 對 Mac/Linux 實機驗證、Windows ACL/chmod、container 化、sandbox-of-sandbox）——已記入後續 backlog 追蹤，本 change 不處理。
- 不擴 Read hook 為「環境變數 token 名感知」（如 `*_TOKEN`、`*_API_KEY` 出現在 path）——檔案路徑判斷複雜度與誤殺率不對等，不為了完整性犧牲簡單性。
- 不改 codex backend 的 sandbox 旗標（`-s workspace-write`、`--ignore-rules` 等）——PoC 驗證行為正確，這層不需要動。
- 不引入 shell parser 做引號感知——簡單性 > 完整性，agent 對 `codebus lint` / `codebus quiz validate` 沒有 use case 需要在引號內塞 metacharacter；引號內 metacharacter 一律 reject 是可接受的權衡。
- 不擴展 Bash hook 允許更多 codebus 子命令——本 change 僅補強現有兩個 allow form（`codebus lint *` 與 `codebus quiz validate *`）的拒絕條款。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `lint-feedback-loop`: Bash hook allow predicate 補 metacharacter rejection；Read hook image blocklist 擴展為包含敏感路徑黑名單。
- `skill-bundles`: Codex Instruction Materialization 條款補「AGENTS.md 對 agent read 行為的 soft constraint 文字」要求。

## Impact

- Affected specs:
  - `lint-feedback-loop`（Fix Bash Hook Installation 段 + PII Image Read Hook Installation 段）
  - `skill-bundles`（Codex Instruction Materialization 段）
- Affected code:
  - Modified:
    - codebus-cli/src/commands/hook.rs
    - codebus-core/src/skill_bundle/mod.rs
- Tests:
  - codebus-cli/src/commands/hook.rs 既有 unit test 模組擴增（metachar block 約 6 條、敏感路徑 block 約 6 條、cross-platform path separator 約 2 條）
  - codebus-core/src/skill_bundle/mod.rs 既有 test 擴增（AGENTS.md 含 soft constraint 文字確認 1-2 條）
- 不影響：codex backend 旗標、claude_cli backend、PII filter scanner、vault 同步邏輯、既有 SKILL.md 內容
- 跨平台：所有改動均字串級（無 OS-specific syscall），Windows / macOS / Linux 行為一致。home 目錄解析用 Rust `dirs` crate 既有依賴（hook.rs 已透過 `HooksConfig` 間接使用 default_config_path）。
