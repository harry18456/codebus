## Why

實測 `goal` 流程在 `D:/side_project/uv`（1289 檔, 26 MiB）冷啟要 39s、熱啟要 6s，但用戶從按 enter 到看到 LLM 第一行輸出之間完全沒有 UI 線索 —— 不知道是在 sync raw、還是在做 PII 掃描、還是在等 Claude CLI 連線。後處理階段（enrich → stale → lint → fix loop → auto_commit）同樣是黑盒，stream 結束後的「靜默」沒人知道是已完成還是 fix loop 還在跑。

目前 `Banner` enum 只有 `Start / Goal / Done / Hint` 四種（codebus-core/src/render/event_renderer.rs 第 18–27 行），不覆蓋這些階段。

## What Changes

- 擴充 `Banner` enum 加入階段性 banner：`SyncStart`、`SyncDone { files, mib, elapsed_ms }`、`PiiSummary { scanner, scanned, hits, action }`、`LintStart`、`LintDone { errors, warns, elapsed_ms }`、`FixIterStart { i, max }`、`FixIterDone { i, fixed, remaining, elapsed_ms }`、`CommitDone { sha7 }`
- `TerminalRenderer::render_banner` 對應每個新 variant 印一行（沿用既有 emoji/symbol mode 規則）
- goal 命令的執行檔案在 `sync_repo_to_raw_with_scanner` / lint / `lint_and_fix` / `auto_commit` 各階段前後 `Instant::now()` + `.elapsed()` + `render_banner`
- fix 命令的執行檔案在每個 fix iteration 前後 emit `FixIterStart` / `FixIterDone`
- PII 階段的計數：擴 `sync_repo_to_raw_with_scanner` 回傳值或加 callback，把「scanned / hits / action」帶回呼叫端供 banner 使用（避免在 raw_sync 內部直接 println）
- 行為層 UX 變更，**不動 spec contract 的功能語意**，只擴 `terminal-output` capability 的 banner 集合

## Non-Goals

- **不動 `LogSink` / `RunLog` schema**：stage timing 不寫進 `RunLog.stages`，不持久化到 jsonl —— 這部分留給未來 token-tracking change，避免今天倉促定 RunLog schema
- **不做 incremental sync / cache 優化**：本 change 只顯化既有耗時，不改變 sync 行為
- **不引入新 telemetry 後端**：純 stdout，不接 OTel / 不寫 metrics 檔
- **不變更 emoji mode 解析鏈**：複用既有 5-level priority chain
- **不改 `--check` flow 的 banner**：本 change scope 限於 `--goal` flow（含其內部 fix loop）
- **不為 `query` flow 加 banner**：query 流程沒有 raw_sync / lint / fix 等階段，加了反而 UX noise

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `terminal-output`: 既有「Render lifecycle banners」requirement 涵蓋四個 banner（start/goal/done/hint），本 change 新增 stage banner 集合（sync/pii/lint/fix/commit）作為新的 requirement，並擴充 Banner enum。emoji/symbol 模式規則沿用既有 5-level priority chain，不動。

## Impact

- Affected specs: `terminal-output`（新增 stage banner requirement）
- Affected code:
  - Modified: codebus-core/src/render/event_renderer.rs（擴 `Banner` enum）
  - Modified: codebus-core/src/render/renderers/terminal.rs（`render_banner` match 新 variant、新增格式化函式）
  - Modified: codebus-core/src/fs/raw_sync.rs（回傳值或 callback 帶出 scanned/hits 計數）
  - Modified: codebus-cli/src/commands/goal.rs（量 elapsed + 各階段 render_banner）
  - Modified: codebus-cli/src/commands/fix.rs（fix iteration 前後 render_banner）
  - Modified: codebus-cli/src/commands/query.rs（測試 stub renderer 補 no-op match arm 以 cover 新 variant）
- Affected dependencies: 無
