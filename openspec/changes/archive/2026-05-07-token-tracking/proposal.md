## Why

LLM 成本對用戶是真錢，但 codebus 目前沒有任何 token usage 紀錄 —— 用戶跑了 10 次 goal 不知道燒了多少、`config-tagged-enum-refactor` 那次 archive 也沒辦法回溯該 change 共消耗多少 token、月底想做 cost analysis 沒資料來源。

既有基礎建設大半已備好但沒接上：

- `LogSink` trait 存在（codebus-core/src/log/sink.rs）
- `RunLog` struct 存在，含 `goal` / `started_at` / `finished_at` / `tokens: TokenUsage` / `wiki_changed` / `lint_*`
- `TokenUsage` struct 存在但 Anthropic-shaped（`cache_read_tokens: u64` / `cache_write_tokens: u64` 都非 Option）
- `JsonlSink` impl 已存在
- `SinkConfig::Jsonl` 在 `config-tagged-enum-refactor` 裡剛變 tagged enum
- 但 `run_goal` / `run_query` 都 `let _ = log_sink;` —— 沒接

「就差最後一段接線」。同時要為 `#2 multi-LLM` 預留 schema —— 不能只支援 ClaudeCli 的 token 形態，否則加 OpenAI / Anthropic API direct 時會二次破壞性遷移用戶的 jsonl 紀錄。

## What Changes

- **TokenUsage schema 規範化**：`cache_read_tokens` / `cache_write_tokens` 從 `u64` 改 `Option<u64>`（None 代表「該 provider 無此概念」，譬如 OpenAI / Ollama），新增 `reasoning_tokens: Option<u64>`（給 o-series / extended thinking 用），新增 `extras: serde_json::Value` 逃生口保留 vendor-specific 原始 JSON
- **新增 `StreamEvent::Usage(TokenUsage)` variant**：作為 provider-agnostic 抽取通道。各 provider 在自己的 stream parser 內把 wire-format-specific token 資訊翻譯成 normalized `TokenUsage` 後 emit。Consumer 端不知道是哪家
- **本 change 只實作 ClaudeCli 那家的抽取**：解析 stream-json 結尾 `result.usage` 物件的 `input_tokens` / `output_tokens` / `cache_creation_input_tokens` / `cache_read_input_tokens` 四個欄位、整個 usage object 同時塞進 extras
- **RunLog 補三個 optional 欄位**：`mode: String`（"goal" / "query"）、`model: Option<String>`、`effort: Option<String>`
- **接線**：`run_goal` / `run_query` 累計 stream 內所有 `StreamEvent::Usage` events、組 `RunLog`、呼叫 `log_sink.write_run(&run_log)`
- **main.rs 真正建構 sink**：用 `build_sink(sink_config_from(cfg))` 替換 hardcoded `NullSink::new()`
- **BREAKING：`SinkConfig::default()` 從 `Null {}` 改成 `Jsonl { dir: None }`**：跟 `goals.jsonl` precedent 一致 —— codebus 自動在 vault 內 track per-run metadata，不要求 opt-in。理由：兩者都寫在 `.codebus/` 下、都是 codebus 已 own 的範圍、都不污染用戶 source repo；保持兩種 metadata 的 default 行為不一致只是反射性的「don't break 0.x」，沒實質保護。用戶要關 telemetry 顯式設 `log: { sink: null }`
- **預設 jsonl dir**：用戶寫 `log: { sink: jsonl }` 沒給 `dir`、或完全沒 `log:` section 時，自動用 `<repo>/.codebus/logs/`
- **jsonl 檔名按 UTC date 切**：`runs-YYYY-MM-DD.jsonl`，每天一個檔
- **`.codebus/.gitignore` 加 `logs/`**：避免 nested vault git 把 runs.jsonl 當待 commit 的變動
- **BREAKING：移除 `SinkConfig::Jsonl.retention_days`**：經評估不需要 enforce（jsonl 五年 < 20 MB、磁碟壓力可忽略；用戶手動 / cron 清理即可；其他工具如 Cargo / npm 也不主動刪）。schema 留垃圾欄位反而誤導，直接砍

## Non-Goals

- **不做 budget cap / max_budget_usd**：需要 model 對應的 token 價目表（hardcode 過時、API 抓引入新依賴）；token tracking 落地後 token 數據先有了，價目表是另一個 motivating feature
- **不做 fallback retry**：依賴 trait-level 重試策略，留給 #2 multi-LLM 階段跟 tool abstraction 一起設計
- **不做 per-mode model**（譬如 query 預設 haiku / goal 預設 sonnet）：跟 token tracking 概念上互補但職責分離；用戶有 cost 數據後可自行決策、是否做成自動化是另一個 change
- **不為其他 provider 寫 usage 抽取**：AnthropicApi / OpenAI / Ollama 各自的 wire format 解析等 #2 multi-LLM 落地時各自負責；本 change 只實作 ClaudeCli。但 schema 與 StreamEvent::Usage variant **這次就敲定**，避免未來二次遷移
- **不做 retention enforcement**：見 What Changes 最後一條
- **不持久化 fix loop 內部每次 iteration 的 token usage**：fix loop 多次呼叫 LLM 是同一個 goal 的 sub-iterations，本次累計到 RunLog.tokens 就好，不展開成每 iter 一筆 jsonl entry（避免 jsonl 被 fix-loop iteration 塞爆）
- **不在 logs.jsonl 內 cross-reference goals.jsonl**：兩個 jsonl 並存、職責不同（goals.jsonl = goal text + commit metadata；logs/runs-DATE.jsonl = 執行成本/結果）。未來分析工具靠 `started_at` timestamp 對齊就可
- **不為 `--no-log` flag 開 per-invocation 取消**：用戶要關 telemetry 用 `log: { sink: null }` 即可；單次取消的 UX 訴求未驗證
- **不動 wiki-ingest spec 對 fix loop 內部行為的描述**：本 change 是 LogSink 接線，fix loop 行為不變

## Capabilities

### New Capabilities

- `token-tracking`: LLM token usage 抽取 + RunLog 持久化。包含 TokenUsage normalized schema、StreamEvent::Usage variant 契約、ClaudeCli 抽取行為、run_goal / run_query 累計與寫 sink 流程、預設 vault-local jsonl 位置、檔名命名規則。

### Modified Capabilities

- `terminal-output`: 既有「Load global config tolerantly」requirement 的 `log` section 描述更新 —— 移除 `retention_days` mention（欄位已移除）、新增 `dir` 為 optional 與 default fallback 到 `<vault>/.codebus/logs/` 的描述。
- `wiki-ingest`: 既有「Run ingest flow on --goal invocation」requirement 補充：goal flow 完成時呼叫 `log_sink.write_run(&RunLog { mode: "goal", model, effort, tokens, ... })` 的契約。
- `wiki-query`: 既有 query 流程同理補上 log_sink.write_run 的契約。

## Impact

- Affected specs:
  - 新建 `token-tracking`
  - MODIFIED `terminal-output`（log section + retention_days 移除）
  - MODIFIED `wiki-ingest`（goal flow 寫 sink 契約）
  - MODIFIED `wiki-query`（query flow 寫 sink 契約）
- Affected code:
  - Modified: codebus-core/src/log/sink.rs（TokenUsage 加 Option / extras / reasoning_tokens；RunLog 加 mode / model / effort）
  - Modified: codebus-core/src/stream/parser.rs（StreamEvent 加 Usage variant；parse_claude_stream_line 從 result event 抽取）
  - Modified: codebus-core/src/llm/providers/claude_cli.rs（流動上不需要動 — 抽取邏輯放在 stream/parser 那層因為它已經負責 stream-json → StreamEvent 的轉換；這裡列出來是為了 reviewer 確認沒漏 wire 點）
  - Modified: codebus-core/src/log/factory.rs（SinkConfig::Jsonl 移除 retention_days；dir 變 Option<PathBuf> 加 default fallback semantics 文件）
  - Modified: codebus-core/src/log/sinks/jsonl_sink.rs（檔名按 UTC date 切；JsonlSink::new 接受 dir 直接用、不再要求 retention_days）
  - Modified: codebus-core/src/config/loader.rs（parse_log 移除 retention_days 解析）
  - Modified: codebus-core/src/config/schema.rs（YAML schema 對應更新）
  - Modified: codebus-cli/src/commands/goal.rs（累計 Usage events、組 RunLog、寫 sink）
  - Modified: codebus-cli/src/commands/query.rs（同上）
  - Modified: codebus-cli/src/main.rs（build_sink 接線取代 NullSink；新增 sink_config_from helper 對齊既有 provider/scanner mapper 結構）
  - Modified: codebus-cli/src/commands/init.rs（init 時把 `logs/` 寫進 `<vault>/.gitignore` 或 `<vault>/.git/info/exclude`）
- Affected dependencies: 無新增
