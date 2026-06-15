## Why

codex token usage parser 目前會把存在但缺少預期 token 欄位的 usage object 視為有效的全 0 Usage event。因 codex 宣告 cumulative token semantics，未來 codex usage 欄位改名時，可能靜默把先前正確的 RunLog.tokens 累計值覆蓋成 0。

## What Changes

- Cumulative token accumulation 改為忽略空的全 0 Usage snapshot，不再無條件覆蓋 accumulator。
- codex turn.completed line 若存在 usage object，但沒有任何預期 usage 欄位可解碼，emit warning。
- 保留 codex 0.136 現行欄位映射：input_tokens、cached_input_tokens、output_tokens、reasoning_output_tokens。
- token usage 維持純 telemetry；不得新增任何以 token count 決定 control-flow 的路徑。
- 不改 RunLog 或 TokenUsage serialization shape。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- agent-backend: Cumulative token usage combination 不得讓空的全 0 snapshot 覆蓋已累積的非零 total。
- codex-backend: Codex stream parsing 在 present usage object 沒有任何可解碼預期 usage 欄位時，必須 emit warning。

## Impact

- Affected specs: agent-backend, codex-backend
- Affected code:
  - Modified: codebus-core/src/stream/codex_parser.rs
  - Modified: codebus-core/src/log/sink.rs
  - New: none
  - Removed: none
