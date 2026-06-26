## Why

codebus 在 spawn agent CLI（claude / codex）子程序時，子程序**完整繼承父 shell 的所有環境變數**，且 spawn 路徑全 repo `env_clear` 零命中。這代表使用者環境裡的機密——`GITHUB_TOKEN`、`AWS_*`、`KUBECONFIG`、各種 `*_TOKEN` / `*_KEY`——以及 codebus 自己注入的 provider API key，全都對 agent 可見。PII scanner 只掃**檔案內容**、不掃 env，所以這些機密落在掃描盲區之外；在 codex `workspace-write` 沙箱下，agent 驅動的 shell 與 subagent 可直接讀出這些值。這是 codebus 隔離姿態（agent 只讀 PII 遮罩過的 mirror）的一個未補破口。

## What Changes

- 在 `env_overrides.rs` 新增一個**跨平台 allowlist passthrough** 共用 helper（claude 與 codex 共用單一名單，避免兩份漂移）：從父 env 讀取、只保留 agent CLI 子程序執行所必需的系統變數（通用 + `cfg(windows)` + `cfg(unix)` 三組）。
- **claude spawn 路徑**（`compose_claude_cmd`）：在 `Command::new` 之後、provider 注入之前呼叫 `Command::env_clear()`，再依序注入 allowlist passthrough + 既有的 `EnvOverrides`（system 0 個 / azure 3 個）。
- **codex spawn 路徑**（`build_command`）：同樣 `env_clear()` + allowlist passthrough，再保留既有的 Azure key 注入（`CODEBUS_CODEX_AZURE_KEY`）。
- 注入順序固定為 `env_clear → allowlist passthrough → provider overrides`，確保 provider key 仍到位。
- **BREAKING（行為）**：system profile 子程序不再「逐字繼承父 env」，改為只繼承 allowlist 名單；任何不在名單內的父 env 變數（含機密）對 agent 不可見。父 shell 的 env 本身完全不動（`env_clear` 只作用於 child `Command`）。
- 強化既有 `scoped_env_injection.rs` 測試 + 擴充 mock binary 的 env dump 清單，以斷言「父注入的 sentinel 機密在 child 缺席、`PATH` / provider key 在 child 存在」。

## Capabilities

### New Capabilities

(none) — 本變更不新增 spec capability；env scrub 行為擴充既有兩個 capability。

### Modified Capabilities

- `claude-code-config`: 「Scoped Environment Injection At Spawn」requirement 從「system profile 子程序逐字繼承父 env」改為「`env_clear` + 跨平台 allowlist passthrough，再疊加 profile 注入」。
- `codex-backend`: 新增「Spawn Environment Scrub」requirement——codex `build_command` 在注入 Azure key 前先 `env_clear` + 套用同一份 allowlist passthrough。

## Impact

- Affected specs: `claude-code-config`（MODIFIED requirement）、`codex-backend`（ADDED requirement）
- Affected code:
  - New: (none)
  - Modified:
    - codebus-core/src/agent/env_overrides.rs（新增 `passthrough_env()` allowlist helper；更新 module doc 與 `for_system` doc，不再宣稱「inherits parent verbatim」）
    - codebus-core/src/agent/claude_cli.rs（`compose_claude_cmd` 加 `env_clear` + passthrough）
    - codebus-core/src/agent/codex_backend.rs（`build_command` 加 `env_clear` + passthrough）
    - codebus-cli/tests/scoped_env_injection.rs（強化既有兩個測試為 scrub 斷言；新增 codex backend 對應測試）
    - codebus-cli/tests/bins/mock_claude.rs（擴充 env dump 清單以涵蓋 sentinel + `PATH` + codex key）
    - openspec/specs/claude-code-config/spec.md（MODIFIED requirement + scenario）
    - openspec/specs/codex-backend/spec.md（ADDED requirement + scenario）
    - docs/security.md（隔離姿態文件同步：env 繼承 → env scrub + allowlist）
  - Removed: (none)
- 風險：allowlist 漏列某個 agent 必需的系統 env → agent 子程序 spawn 失敗。unit test 不一定抓得到（測試程序本身帶著這些 env），必須在 apply 後以真實 claude + 真實 codex 各跑一次 goal/query 實機驗證。
