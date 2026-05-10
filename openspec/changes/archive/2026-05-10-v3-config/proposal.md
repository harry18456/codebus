## Why

`~/.codebus/config.yaml` 目前只有 `lint.fix.enabled` 一個欄位，user 真的會想調的 PII 行為（要不要掃、自訂 pattern、命中後動作）與 Claude Code 算力分配（每 verb 各自的 model / effort）都還沒接上。同時 `~/.codebus/config.yaml` 從未被主動建立 — user 不知道有 config 可以調。本 change 把這些 knob 補齊並讓 `codebus init` 寫一份 starter config 改善 discoverability。

## What Changes

- **新增 `pii` section**（modify `pii-filter` capability）：
  - `pii.scanner: regex_basic | none`（default `regex_basic`）— `none` 代表整段關掉 PII 掃描，沿用既有 `NullScanner` impl。**註**：使用字串 `none` 而非 YAML null literal，避免「寫了 `scanner: null` 卻被當作 absent field 走 default」的 foot-gun
  - `pii.patterns_extra: []`（default 空 list）— 字串 list，append 到 `RegexBasicScanner` 內建 4 條 pattern
  - `pii.on_hit: warn | skip | mask`（**BREAKING：default 從 `warn` 改為 `mask`**）
    - `warn`：保留現行行為（mirror 檔 + stderr warn 每個 match）
    - `skip`：整個檔案不進 raw mirror（v3-pii 預留枚舉值，本 change 首次落實 raw_sync 行為）
    - `mask`：matched substring 替換為 `[REDACTED:<pattern_name>]` 後再 mirror（v3-pii 預留枚舉值，本 change 首次落實 raw_sync 行為）
- **新增 `claude_code` section**（modify `cli` capability）：
  - `claude_code.{goal,query,fix}.{model, effort}` per-verb 各自一組
  - default：`goal={model: opus, effort: high}` / `query={model: haiku, effort: low}` / `fix={model: sonnet, effort: medium}`
  - spawn `claude -p` 時 forward 為 `--model <X> --effort <Y>` flag（兩 flag spike 確認過存在於 Claude Code v2.1.137 `--help`）
- **`codebus init` 寫 starter `~/.codebus/config.yaml`**（modify `cli` capability）：
  - if-missing 行為：檔案存在則完全不動，避免 clobber user 客製化
  - 內容包含 `pii` + `claude_code` 全部欄位的 default 值與 inline comment 說明
  - primitive 在 `codebus-core/src/config/global_starter.rs`，orchestration 在 `codebus-cli/src/commands/init.rs`，沿用既有「core 寫入 primitive、cli 印 progress 訊息」pattern
- **反轉 `pii-filter` 既有「SHALL NOT read `~/.codebus/config.yaml`」requirement**：從 hardcoded default 改為 config-driven，仍守 default fallback（檔案缺、section 缺、欄位缺均回 default）。

## Non-Goals

- **`lint.disabled_rules` 不做**：5 條 lint rule 都是 wiki 健康度基本功能（duplicate-slug / frontmatter-integrity / broken-wikilink-related / misplaced-root-page / nav-missing），目前無 user 反映想關 rule，0-consumer config 不寫進 spec
- **`lint.custom_rules_dir` 不做**：自訂 rule 機制需要 plugin / DSL / sub-process 三選一，無 second-impl 驗證；違反「single-impl 抽象不寫 spec」原則
- **任何形式的 vendor `provider:` 抽象不做**：codebase 只有 `claude_cli` 一個 impl，重蹈 v3 第一次嘗試（被 `git reset --hard` 退掉）的覆轍；二供應商真要進來時由 `v3-multi-agentic-provider` follow-up change 開
- **6 條 v2 tolerance 不一次補齊**：實作「missing file / missing section / missing field / unknown key forward-compat / unknown discriminator graceful warn」共 5 條；不做 type-mismatch graceful warn（serde_yaml 預設行為已涵蓋常見 case）
- **`~/.codebus/` 目錄主動建立的時機**：僅在 `codebus init` 流程內 if-missing 寫；不為了讀 config 而主動建目錄（讀路徑沒人為 write 是預期行為，loader fallback 至 default）

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `pii-filter`: 反轉「SHALL NOT read `~/.codebus/config.yaml`」requirement；新增 `pii.{scanner, patterns_extra, on_hit}` config schema 與對應 raw_sync 行為（Skip / Mask 首次落實）
- `cli`: goal / query / fix subcommand 加上 `claude_code.{verb}.{model, effort}` 讀取與 `claude -p` flag forwarding；init 加上 starter `~/.codebus/config.yaml` write 步驟

## Impact

- Affected specs: `pii-filter` (modified), `cli` (modified)
- Affected code:
  - New:
    - codebus-core/src/config/pii.rs
    - codebus-core/src/config/claude_code.rs
    - codebus-core/src/config/global_starter.rs
  - Modified:
    - codebus-core/src/config/mod.rs
    - codebus-core/src/agent/claude_cli.rs
    - codebus-core/src/wiki/fix/mod.rs
    - codebus-core/src/vault/raw_sync.rs
    - codebus-cli/src/commands/init.rs
    - codebus-cli/src/commands/goal.rs
    - codebus-cli/src/commands/query.rs
    - codebus-cli/src/commands/fix.rs
