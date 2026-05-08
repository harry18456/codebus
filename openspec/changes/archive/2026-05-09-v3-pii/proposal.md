## Why

v3-init #2 archive 把 raw_sync 留在 byte-for-byte 複製（原始碼直接寫進 `.codebus/raw/code/`），secrets / API keys / emails 原封不動進 vault；vault 是 Claude Code agent 讀的對象（goal / query / fix skill），等於把 credential 餵 LLM。v3-init proposal:9 已明確留出 #3 v3-pii 接上 PII filter，v2 也有完整 `RegexBasicScanner` 實作可 carry。

## What Changes

- 新增 capability `pii-filter`：`PiiScanner` trait + `PiiMatch` / `PiiSeverity` / `OnHit` 型別 + `NullScanner`（test fixture，零 match）+ `RegexBasicScanner`（v2 builtin 4 條 regex：`aws-access-key` / `anthropic-api-key` / `email` / `ipv4`）。
- 修改 capability `vault`：Raw Mirror requirement 從「NullScanner mode（無 PII redaction）」切到「default 走 RegexBasic，`OnHit::Warn` 模式 — 仍 mirror file 內容，每個 match 印一行 stderr warn」。
- raw_sync 內部 hardcode 構造 `RegexBasicScanner::new(&[])`（不接 config 入口）；`patterns_extra` / `on_hit` 等 config 覆蓋是 #8 v3-config 的事，本 change scope 排除。
- `codebus init` stdout 進度行多一條 PII match count（例如 `✓ raw mirror: 142 files, 89 KiB, 3 PII matches warned`）。
- raw_sync 函數重命名：`sync_with_null_scanner` → `sync_with_scanner`（吃 `&dyn PiiScanner`）— #3 後行為已不是 null。

## Capabilities

### New Capabilities

- `pii-filter`: PII scanner 抽象與內建實作（trait、`RegexBasicScanner` builtin 4 條 regex、`NullScanner` 占位）

### Modified Capabilities

- `vault`: Raw Mirror requirement 從 NullScanner 模式切到 RegexBasic + `OnHit::Warn` default；raw mirror summary 新增 PII match count 欄位
- `cli`: Init Subcommand Behavior 的 raw mirror 進度行 SHALL 包含 PII match count（含零 count）

## Impact

- Affected specs:
  - New: `pii-filter`
  - Modified: `vault`, `cli`
- Affected code:
  - New:
    - codebus-core/src/pii/mod.rs
    - codebus-core/src/pii/provider.rs
    - codebus-core/src/pii/scanners/mod.rs
    - codebus-core/src/pii/scanners/null_scanner.rs
    - codebus-core/src/pii/scanners/regex_basic.rs
  - Modified:
    - codebus-core/src/lib.rs
    - codebus-core/src/vault/raw_sync.rs
    - codebus-cli/src/commands/init.rs
  - Removed: 無
