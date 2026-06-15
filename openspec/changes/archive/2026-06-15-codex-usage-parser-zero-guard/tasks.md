## 1. 測試先行

- [x] [P] 1.1 為 Provider-Declared Token Usage Semantics 在 codebus-core/src/log/sink.rs 新增 Cumulative 測試，證明 Empty cumulative snapshots do not replace accumulated totals：先套用非零 Usage，再套用全 0 Usage，accumulator 保持非零值；以 `cargo test -p codebus-core cumulative --lib` 驗證。
- [x] [P] 1.2 為 Codex Stream Parsing 在 codebus-core/src/stream/codex_parser.rs 新增 parser 測試，證明 Codex usage warning fires only when no expected field decodes：renamed / unknown usage 欄位會產生 exactly one warning 且仍 emit 全 0 Usage；至少一個預期欄位成功解碼時不產生 missing-fields warning；以 `cargo test -p codebus-core codex_parser --lib` 驗證。
- [x] 1.3 保留 codex 0.136 既有欄位 mapping 行為，確認 `turn_completed_maps_usage` 仍逐位驗證 input_tokens、cached_input_tokens、output_tokens、reasoning_output_tokens；以 `cargo test -p codebus-core turn_completed_maps_usage --lib` 驗證。

## 2. 實作

- [x] [P] 2.1 在 apply_token_usage 的 Cumulative 分支實作空快照 guard：只有任一 normalized token count 非零時才 last-wins 覆蓋，Delta 分支維持 field-wise sum；以 1.1 的 sink 測試與既有 cumulative latest snapshot 測試驗證。
- [x] [P] 2.2 在 codex parser 實作 missing usage fields warning：public `parse_codex_line(raw: &str) -> Vec<StreamEvent>` 與 `AgentBackend::parse_stream_line` signature 不變，內部 helper 提供 warning callback 或等價 warning flag 供測試；以 1.2 與 1.3 的 parser 測試驗證。

## 3. 驗證

- [x] 3.1 執行 focused Rust tests，覆蓋 sink cumulative guard、codex parser warning、既有 `turn_completed_maps_usage`；以 `cargo test -p codebus-core cumulative --lib && cargo test -p codebus-core codex_parser --lib && cargo test -p codebus-core turn_completed_maps_usage --lib` 驗證。
- [x] 3.2 執行 Spectra 檢查，確認 proposal/design/spec/tasks 一致且 change 可進入 apply；以 `spectra analyze codex-usage-parser-zero-guard --json` 與 `spectra validate codex-usage-parser-zero-guard` 驗證。
