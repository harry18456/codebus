## Why

codebus claude-path 的 PreToolUse `check-read` hook 目前是 denylist（只擋 image 副檔名 + `*id_rsa*`/`*.pem`/`*.key` basename + `~/.ssh` 等 home prefix），而非把讀取限制在 vault 內。這留下兩個破口：

- **F1（MEDIUM）**：絕對路徑 Read 可讀母 repo 未經 PII 遮罩的 source，以及 denylist 外的憑證（如 `~/.kube`、`~/.docker/config.json`、`~/.env`、`~/.netrc`）。
- **F2（LOW）**：`check-read` 的 PreToolUse matcher 只 match `Bash`/`Read`，完全沒覆蓋 `Glob`/`Grep`；agent 可用 `Grep` content 模式讀到 `check-read` 對 `Read` 封鎖的檔案內容。F2 已於 2026-06-04 在真實 codebus wrapper + Windows native Claude Code 2.1.162 + codebus 3.0.0 實機對抗驗證確認（同檔 probe_secret.pem：Read 被擋、Grep -content 回 sentinel）。

兩者同根（denylist 而非 vault allowlist）、同一修改點，合併為一個 containment 修法解決才有 CP 值。

## What Changes

- 把 `check-read` 決策從 denylist 改為 **vault-root containment allowlist**：取 Read 的 `tool_input.file_path` 或 Glob/Grep 的 `tool_input.path`，canonicalize 後要求落在 canonicalize 過的 vault root 內才放行，否則 block。
- **覆蓋 Glob/Grep**：`codebus init` 寫的 settings.json 的 `hooks.PreToolUse` 新增 `Glob` 與 `Grep` matcher entry（route 到 `codebus hook check-read`），讓 hook 對 search 工具生效。
- 既有 image/sensitive denylist **保留為 vault 內 defense-in-depth**，不移除。
- 新增 `hooks.read_path_containment` config key（預設 `true`），作為 containment gate 的獨立開關與 emergency escape hatch，與既有 `hooks.read_image_block` 分離（後者繼續只 gate denylist）。
- `vault-gate-integrity` lint rule 隨 REQUIRED_HOOKS 自動連動 → 既有 vault 跑 `codebus lint` 會被 flag 缺少 Glob/Grep gate，作為 migration 偵測機制。
- **BREAKING（vault gate 行為）**：新 vault 的 settings.json 多兩個 matcher；既有 vault 因 write-if-missing 不會自動升級，需依 release note 補（手動 JSON 片段或於新位置 re-init）。

## Non-Goals

詳見 design.md 的 Goals / Non-Goals 段。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `lint-feedback-loop`: MODIFY `PII Image Read Hook Installation`（matcher 擴及 Glob/Grep、Input 增 `path` 欄位、containment 前置於 denylist）、MODIFY `Vault Gate Integrity Check`（REQUIRED_HOOKS 增 Glob/Grep → 缺 matcher 的 flag 場景由兩條擴為四條）、ADD `Vault Containment Read Gate`（containment 行為 + `read_path_containment` key + Windows 邊界與誤擋場景）。

## Impact

- Affected specs: lint-feedback-loop
- Affected code:
  - Modified:
    - codebus-cli/src/commands/hook.rs
    - codebus-core/src/vault/settings.rs
    - codebus-core/src/wiki/lint/rules/vault_gate_integrity.rs
    - codebus-cli/tests/lint_flow.rs
  - New:
    - (無新檔；皆為既有檔修改與設定鍵新增)
  - Removed:
    - (無)
