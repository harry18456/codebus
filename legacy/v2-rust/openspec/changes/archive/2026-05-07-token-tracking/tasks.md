## 1. TokenUsage schema 規範化

- [x] 1.1 改寫 codebus-core/src/log/sink.rs 的 `TokenUsage` struct：`cache_read_tokens` / `cache_write_tokens` 改 `Option<u64>`、新增 `reasoning_tokens: Option<u64>`、新增 `extras: serde_json::Value`，每個 Option 欄位加 `#[serde(default, skip_serializing_if = "Option::is_none")]`、`extras` 加 `#[serde(default, skip_serializing_if = "serde_json::Value::is_null")]`；先寫單元測試覆蓋 spec scenarios「Provider without cache concept produces None for cache fields」與「TokenUsage with all None cache fields serializes without those keys」再實作；保留原有 input_tokens / output_tokens 為 u64。實作 spec requirement「Normalized TokenUsage schema」

## 2. RunLog 補欄位

- [x] 2.1 改寫 codebus-core/src/log/sink.rs 的 `RunLog` struct 加 `mode: String`、`model: Option<String>`、`effort: Option<String>` 三個欄位（model / effort 加 `#[serde(default, skip_serializing_if = "Option::is_none")]`）；先寫測試驗 「mode=goal/query 序列化正確」「model=Some=>欄位出現、None=>欄位省略」再實作。實作 spec requirement「RunLog carries mode model effort fields」

## 3. StreamEvent::Usage variant + parser 抽取

- [x] 3.1 在 codebus-core/src/stream/parser.rs 的 `StreamEvent` enum 加 `Usage(TokenUsage)` variant；在 `parse_claude_stream_line` 處理 `{"type": "result", ...}` 行時，從 `result.usage` 物件抽出 `input_tokens` / `output_tokens` / `cache_creation_input_tokens` / `cache_read_input_tokens` 四欄位映射成 normalized TokenUsage（cache_creation_input_tokens → cache_write_tokens，cache_read_input_tokens → cache_read_tokens）、整個原始 usage object 塞進 `extras`、emit `Usage` event；先寫單元測試覆蓋 spec scenario「Claude CLI invocation populates all four anthropic fields」「Claude CLI provider emits Usage before Done」「Stream without usage data omits Usage event」再實作。實作 spec requirement「Provider stream parser emits StreamEvent::Usage」

## 4. SinkConfig::Jsonl 移除 retention_days，dir 仍 Option

- [x] 4.1 在 codebus-core/src/log/factory.rs 的 `SinkConfig::Jsonl` variant 移除 `retention_days` 欄位、`dir` 維持 `Option<PathBuf>`；改寫 `build_sink` 對 Jsonl arm：當 `dir` 為 None 時不再回 `SetupError`、改回 `Err(SinkError::Setup("jsonl sink requires a vault-resolved dir; this should not happen in practice"))` 因為 caller (run flow) 應在 build_sink 之前先把 None 解析成具體 vault path（task 6 處理）；既有測試 `jsonl_round_trips_with_dir_and_retention` 對應改寫；新增測試「Jsonl 不再有 retention_days 欄位」「dir: None 時 build_sink 仍回 Err（spec 上要求 caller resolve 後再 build）」

## 5. Loader 對應更新

- [x] 5.1 在 codebus-core/src/config/loader.rs 的 `parse_log` 移除 `retention_days` sub-field 解析（該欄位現在算 unknown sub-field、走既有 silently ignore 路徑）；既有測試 `log_section_selects_sink` 對應改寫只驗 dir 解析；新增測試覆蓋 spec scenarios「Log section selects jsonl sink with explicit dir」「Log section selects jsonl sink without dir defaulting to vault」「Log section retention_days is silently ignored」（在 loader 層 retention_days 直接被 forward-compat unknown sub-field 路徑吞掉、不出 warning）。實作 spec requirement「Load global config tolerantly」對 log section 的更新

## 6. JsonlSink 檔名按 UTC date 切

- [x] 6.1 改寫 codebus-core/src/log/sinks/jsonl_sink.rs 的 `JsonlSink::write_run`：用 `entry.started_at` 解析 UTC date（chrono 已是依賴）、組檔名 `runs-YYYY-MM-DD.jsonl`、append 到該檔（`OpenOptions::new().create(true).append(true)`）；先寫單元測試覆蓋 spec scenarios「First run of a UTC day creates a new file」「Subsequent run on same UTC date appends to existing file」「Run crossing UTC midnight writes to file matching started_at」。實作 spec requirement「Jsonl files rotate by UTC date」

## 7. run_goal 累計 Usage events 並寫 sink

- [x] 7.1 改寫 codebus-cli/src/commands/goal.rs 的 `run_goal`：在 stream 渲染 loop 內偵測 `StreamEvent::Usage(t)` 累計到 local `accumulated_tokens: TokenUsage`（input/output 直接相加；Option 欄位用 `match (a, b) { (None, None) => None, (Some(a), None) | (None, Some(a)) => Some(a), (Some(a), Some(b)) => Some(a + b) }` 累計；extras 保留最後一個非 null）；同樣在 lint_and_fix 內也要把 Usage events 累計上來（lint_and_fix 簽名擴一個 `accumulated: &mut TokenUsage` 參數），fix loop 共享同一 accumulator；run flow 結尾組 RunLog（mode: "goal", model, effort, tokens, started_at, finished_at, wiki_changed, lint_*）、呼叫 `log_sink.write_run(&run_log)`。實作 spec requirement「Goal and query flows accumulate Usage events into RunLog」與 wiki-ingest「Run ingest flow on --goal invocation」的補充契約

## 8. run_query 累計 Usage events 並寫 sink

- [x] 8.1 改寫 codebus-cli/src/commands/query.rs 的 `run_query`：對應跟 task 7 同樣的累計邏輯（query 沒有 fix loop、只有單次 invoke）；run 結尾組 RunLog（mode: "query", model, effort, tokens, started_at, finished_at, wiki_changed: false, lint_*: 0）、呼叫 `log_sink.write_run`。實作 wiki-query「Run query flow on --query invocation」的補充契約

## 9. main.rs 接 build_sink + 預設 vault dir 解析

- [x] 9.1 在 codebus-cli/src/main.rs 加 `sink_config_from(cfg: &GlobalConfig) -> SinkConfig` mapper（對齊既有 `provider_config_from` / `scanner_config_from` 模式，回傳 `cfg.log.clone().unwrap_or_default()`）；加 `resolve_jsonl_dir(repo: &Path, cfg: SinkConfig) -> SinkConfig` helper：當 SinkConfig 是 `Jsonl { dir: None }` 時改寫成 `Jsonl { dir: Some(<repo>/.codebus/logs/) }`、其他 variant 不動；run_goal_cmd / run_query_cmd / run_fix_cmd 三條路徑將 hardcoded `NullSink::new()` 改用 `build_sink(resolve_jsonl_dir(repo, sink_config_from(cfg)))?`；錯誤路徑跟既有 build_provider 一致 fail-fast 印 stderr。實作 spec requirement「Default jsonl directory falls back to vault-local logs folder」

## 10. init 把 logs/ 加進 nested vault gitignore

- [x] 10.1 在 codebus-cli/src/commands/init.rs 的 vault init 流程結束前，把 `logs/` 寫進 `<repo>/.codebus/.git/info/exclude`（用 git/info/exclude 而非 .gitignore，避免污染用戶可能想自己整理的 .gitignore）；先寫測試驗 init 後該檔包含 `logs/` 行。實作 spec requirement「Logs directory is excluded from nested vault git」

## 11. 整合測試 + 驗收

- [x] 11.1 codebus-cli 加 integration test：mock provider 在 stream 中 emit `StreamEvent::Usage(TokenUsage { input_tokens: 100, output_tokens: 50, ..Default })` 後接 Done、跑 run_query / run_goal、驗 jsonl 檔有一行對應 RunLog 且 tokens 欄位正確。覆蓋 spec scenario「Goal flow with no fix loop writes one RunLog containing one invocation's tokens」「Goal flow with fix loop sums tokens across iterations」（後者 mock 一次 ingest + 兩次 fix iter 的 Usage events、驗總和）
- [x] 11.2 cargo test --workspace 全綠 + cargo clippy --workspace -- -D warnings 無警告；確認既有 304 tests 在 schema 變動後 0 regression
- [x] 11.3 實機 e2e：寫 temp config `log: { sink: jsonl }`（不指定 dir、走 vault-local fallback）、CODEBUS_HOME 指 temp、cargo run --release 對 D:/side_project/uv 跑一次小 query、驗 `<uv>/.codebus/logs/runs-YYYY-MM-DD.jsonl` 出現一行 RunLog 且 tokens 數字非零；同時驗 `git -C uv/.codebus status --porcelain` 不顯示 logs/ 路徑
- [x] 11.4 跑 spectra-audit：審 (a) Option<u64> 累計邏輯在「provider 中途換 (e.g. 從 Some(10) 變 None) 」這個目前不可能但未來可能的場景下行為是否合理（目前實作：保留 Some 並繼續累；audit 確認 future-proof）；(b) 大量 Usage events 累積導致 u64 溢位的可能性（input_tokens 滿載一次約 200K、要 ~9e13 次才 overflow，可忽略，但要在 RunLog 註明 saturating add 防呆）；(c) chrono Local vs UTC 在 jsonl 檔名解析的一致性（spec 強制 UTC、實作 chrono::Utc::now() 確認）
