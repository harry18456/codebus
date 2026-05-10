## Why

v3 CLI 輸出目前是純文字 11 行 `✓ ...` progress 串、無 emoji、無 color、無 clickable wikilink，跟 v2 的「上車舞」公車意象（`🚌 來囉來囉~ CodeBus 駛入...` → `🎉 掰掰~下車囉！`）差距明顯。`codebus lint` 輸出找到 issue 時也只是純文字 `wiki/concepts/foo.md` 路徑，user 無法直接 click 進 Obsidian。本 change 補回 v2 的 banner 系列與終端 polish（color + OSC 8 hyperlink），讓 default UX 帶 codebus 品牌感、讓 lint 結果可直接點擊修正，同時保留 `--debug` 既有 verbose lines 作為 troubleshooting 入口。

## What Changes

- **新增 v2 Banner 系列**（在 `codebus-core/src/render/` 新模組）：覆蓋 init / goal / query / fix / lint 五個 verb 的開場 / 階段 / 收場訊息，共 10 條 banner 變體：
  - `Start { repo_path }` — `🚌 來囉來囉~ CodeBus 駛入 <path>...`
  - `Goal { goal_text }` — `🎯 任務目標：<goal>`
  - `SyncStart` — `🔄 同步 source → raw/code...`
  - `SyncDone { files, mib, elapsed_ms }` — `✓ 同步完成 (<files> 檔, <mib> MiB, <ms> ms)`
  - `PiiSummary { scanner, scanned, hits, action }` — `🛡 PII：<scanner>, scanned <N>, hits <N>, action <warn|skip|mask>`
  - `LintStart` — `🔍 lint 中...`
  - `LintDone { errors, warns, elapsed_ms }` — `✓ lint：<E> errors, <W> warnings (<ms> ms)`
  - `CommitDone { sha7 }` — `📌 commit <abc1234>`
  - `Done { wiki_path }` — `🎉 掰掰~下車囉！wiki 已生成於 <path>`
  - `Hint { wiki_path }` — `💡 請用 Obsidian 開 <path>`
- **default 輸出由 banner 系列驅動**（modify `cli` capability）：init / goal / query / fix subcommand 的 stdout progress 行**從現有 11 條 `✓ ...`（init）與類似明細行（goal/query/fix）改為呼叫對應 banner 序列**。banner 數量遠少於現有 progress line（init 從 11 行 → 5-7 條 banner），讓 default 輸出更聚焦。
- **`--debug` mode 行為不變**：既有 `[debug] xxx` lines 全部保留作為 troubleshooting 細節層；debug mode 同時印 banner + `[debug]` lines（不是兩套並行 UI，而是 debug 在 banner 上加細節層）。
- **lint 輸出 polish**（modify `lint-feedback-loop` capability）：
  - text format 新增 emoji ↔ symbol fallback：clean header 從 `ok` ↔ `✅`、issue header 從 `#` ↔ `🔍`、error mark 從 `x` ↔ `✗`、warn mark 從 `!` ↔ `⚠`
  - text format 在 issue 行加 ANSI color：`error:` 紅色、`warn: ` 黃色（issue body 文字不上色，路徑 header 不上色）
  - text format 的 `wiki/<path>` 路徑行 wrap 成 OSC 8 hyperlink，URL 為 `obsidian://open?vault=<vault_id>&file=<rel-from-wiki>`；vault id 從 Obsidian config 讀取（**新增** `obsidian_register::lookup_vault_id(wiki_path) -> io::Result<Option<String>>` 函式，port 自 v2 同名 helper）；找不到 vault id 時降級為「無 hyperlink，純文字 path」
  - JSON format **不變** — JSON 永遠純機器格式，不可含 emoji / ANSI / OSC 8（既有 `lint-feedback-loop` spec 已禁，本 change 維持）
- **環境感知**（在新 render 模組內封裝）：
  - `NO_COLOR` env 設定時關閉 ANSI color（社群標準）
  - 非 TTY（stdout 被 redirect/pipe）時關閉 emoji + color + OSC 8（避免污染管線輸出）
  - 偵測由 `std::io::IsTerminal` + `supports-hyperlinks` crate 完成
- **Stdio::inherit() 不動**：spawn 的 claude agent stdout/stderr 仍直接 passthrough；banner 在 spawn 之前/之後印（codebus 自己的 stdout）。本 change 不改 `codebus_core::agent::claude_cli::invoke` 任何 stdio 行為。

## Non-Goals

- **不做 5-level emoji priority chain**：roadmap §4 #10 原列的 `--emoji on|off` flag、`--no-emoji` flag、`NO_EMOJI` env、`config.yaml emoji:` 欄位、TTY auto-detect 5 段優先序全部 drop。只留 `NO_COLOR` env + TTY auto-detect 兩段（emoji 跟 color 共用同一個 TTY 偵測；非 TTY 兩者皆關）
- **不做 v2 stream event renderer**：v2 用 `Stdio::piped()` parse claude stream-json 自 render thought / tool / observation event；v3 path D 走 `Stdio::inherit()` passthrough，agent 自帶 UI，codebus 不插手。本 change 不變更此架構
- **不做 RunLog / token usage 追蹤**：v2 的 `<vault>/.codebus/logs/runs.jsonl`（goal text / mode / model+effort / 時戳 / token usage / wiki_changed / lint counts）需要 stream-json parsing 才能撈，與 stdio 架構耦合，獨立 follow-up `v3-run-log` change 處理
- **不做 banner 內 color**：banner 只用 emoji，不上 ANSI color；color 集中在 lint issue body 的 `error:` / `warn: ` 標籤
- **lint JSON format 不動**：machine-readable 契約已 ship，加 emoji / color 會破壞 agent 消費

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `cli`: init / goal / query / fix subcommand 的預設 stdout 輸出從個別 `✓ ...` progress 行改為 banner 序列；既存 `[debug]` 行為不變
- `lint-feedback-loop`: text format 新增 emoji fallback、ANSI color、OSC 8 hyperlink；JSON format 不變；新增 `NO_COLOR` env 與 TTY 偵測契約

## Impact

- Affected specs: `cli` (modified), `lint-feedback-loop` (modified)
- Affected code:
  - New:
    - codebus-core/src/render/mod.rs
    - codebus-core/src/render/banner.rs
    - codebus-core/src/render/options.rs
    - codebus-core/src/render/lint_text.rs
  - Modified:
    - codebus-core/src/lib.rs
    - codebus-core/src/vault/obsidian_register.rs
    - codebus-core/src/wiki/lint/output.rs
    - codebus-cli/src/commands/init.rs
    - codebus-cli/src/commands/goal.rs
    - codebus-cli/src/commands/query.rs
    - codebus-cli/src/commands/fix.rs
    - codebus-cli/src/commands/lint.rs
