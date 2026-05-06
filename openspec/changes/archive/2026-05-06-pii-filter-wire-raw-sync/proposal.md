## Why

`raw/code/` 是 agent 看得到的整份 source mirror。若 user repo 內有 hardcoded secret（AWS access key、Anthropic API key）、個資（email、IPv4）或 user 自定義敏感字串，目前會直接送進 LLM context — v1-archive 曾有 sanitizer、rust-rewrite 期間沒 port 回來，是 README roadmap #1（security blocker）。剛 archive 的 plugin-architecture-refactor 已 ship `PiiScanner` trait + `NullScanner`（default）+ `RegexBasicScanner`（4 條 builtin pattern）+ factory + `ScannerConfig`，骨架完整；這個 change 只缺把 scanner 接進 `raw_sync`、實作 `on_hit` 三模式、加 spec scenarios + lock-in tests。

## What Changes

- `raw_sync` 在 mirror 每個 text 檔前先過 `PiiScanner::scan(content, rel_path)`；命中時依 `on_hit` 模式處理：
  - **`warn`**：寫一行 `warning: PII match in <rel_path>: <pattern_name> at offset N` 到 stderr、檔案仍 mirror（內容不變）
  - **`skip`**：整檔不 mirror、stderr 標記 `skipped: <rel_path> (reason: pii hit <pattern_name>)`，agent 之後 Glob raw/code/ 看不到此檔
  - **`mask`**：matched 字串原地替換為 `[REDACTED:<pattern_name>]`（保留檔名、長度可能略異），mirror 替換後內容
- `raw_sync` 函式 signature 擴展接受 `&dyn PiiScanner` + `OnHit`；既有 `sync_repo_to_raw(repo, raw_dir)` 仍 work（內部呼叫新形態、傳 `NullScanner` + `OnHit::Warn` 預設）— 保 backward compat 給未直接從 config 來的呼叫者
- `commands/goal.rs` 與 `commands/query.rs` 都不 sync raw（query 不寫 raw），所以只有 goal 的 raw_sync 路徑要接 cfg.pii
- `main.rs::run_goal_cmd` 從 `cfg.pii` build scanner via `pii::build_scanner`，注入 goal flow 的 raw_sync 呼叫
- 二進位／非 UTF-8 檔不掃（`String::from_utf8` 失敗即 fall through 走原 mirror 路徑），避免假命中
- 預設行為與 0.2.0 完全一致：user 沒設 `~/.codebus/config.yaml`、或 `pii` section 不存在、或 `pii.scanner: null` → `NullScanner` → 0 命中 → raw mirror byte-equal 既有行為

## Non-Goals (optional)

- **不加新 scanner impl**：`Presidio` / `Aws` / 未來 ML scanner 仍 reserved 給後續 change；本次只接 `RegexBasic`（與 `Null`）
- **不擴展 builtin pattern set**：4 條既有（aws-access-key / anthropic-api-key / email / ipv4）；user 透過 `patterns_extra` 加自訂 regex
- **不掃 `wiki/` 內容**：wiki 是 agent 寫出、agent 自我責任；本次只掃 raw mirror，避免雙重掃描成本與假命中
- **不持久化掃描結果**：warn output 只走 stderr，不入 `RunLog` / 不寫 jsonl（`LogSink` 也都還沒 wire）
- **不掃 binary**：UTF-8 解析失敗即 fall through；既有 `MAX_FILE_BYTES = 5 MiB` 篩選保留
- **不重新設計 `OnHit` 語意**：plugin-architecture-refactor 已定 `Warn / Skip / Mask` 三 variants，本次原地接
- **不對 stderr 做 i18n**：warning text 一律 en-us；spec scenarios 只 pin 關鍵字（`PII match`、`skipped`、`pattern_name` 出現於行內）
- **不引入新 cargo dep**：`regex` 已在 tree

## Capabilities

### New Capabilities

- `pii-filter`: PII scanning before raw_sync mirror — defines the contract for `PiiScanner` invocation timing, the three `OnHit` modes (warn / skip / mask) and their externally-observable effects, the binary-file fall-through, and the opt-in path via `~/.codebus/config.yaml` `pii` section

### Modified Capabilities

- `wiki-ingest`: add a requirement that `--goal` flow's raw_sync invokes the configured PII scanner before mirroring each text file; default config keeps current 0.2.0 behavior

## Impact

- Affected specs:
  - New: openspec/specs/pii-filter/spec.md
  - Modified: openspec/specs/wiki-ingest/spec.md
- Affected code:
  - Modified: codebus-core/src/fs/raw_sync.rs (signature extension + scanner invocation + OnHit dispatcher)
  - Modified: codebus-cli/src/commands/goal.rs (accept ScannerConfig + OnHit, pass into sync_repo_to_raw)
  - Modified: codebus-cli/src/main.rs (build PiiScanner from cfg.pii, plumb to run_goal)
  - New tests: inline `#[cfg(test)] mod tests` in codebus-core/src/fs/raw_sync.rs covering all three OnHit modes, binary-file fall-through, multi-pattern hits, ScannerConfig from config layer
