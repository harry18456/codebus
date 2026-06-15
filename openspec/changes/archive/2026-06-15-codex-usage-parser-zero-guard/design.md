## Context

codebus 將不同 agent backend 的 usage JSONL 正規化為 TokenUsage，再由 agent::invoke 依 backend 宣告的 TokenUsageSemantics 合併，最後寫入 RunLog.tokens。codex backend 宣告 Cumulative，代表每個 Usage event 是目前 invocation 的累計快照；sink 目前採 last-wins 覆蓋。

codex parser 目前在 turn.completed.usage 存在時，用固定欄位 input_tokens、output_tokens、cached_input_tokens、reasoning_output_tokens 取值。input_tokens 與 output_tokens 缺失時會落成 0；若未來 codex 改欄位名，parser 仍會產生全 0 Usage，sink 再把既有累計值覆蓋成 0。這只影響 telemetry，沒有現有 control-flow 依賴 token count。

## Goals / Non-Goals

**Goals:**

- 保留 codex 0.136 usage 欄位映射的逐位行為。
- 防止 Cumulative backend 的空 usage 快照覆蓋既有非零累計值。
- 讓 codex usage 欄位全數無法解碼時可觀測為 warning。
- 保持 agent::invoke 的 provider-agnostic 合併模型。

**Non-Goals:**

- 不改 RunLog、TokenUsage、StreamEvent 的序列化 schema。
- 不新增任何 token count control-flow。
- 不改 Claude / Delta usage 合併路徑。
- 不新增 codex usage 欄位 alias、版本偵測、或 per-field 語意擴張。
- 不把 token extras 視為可合併的 token count 來源。

## Decisions

### Empty cumulative snapshots do not replace accumulated totals

Cumulative 分支只在 addend 帶有任一非零 token count 時覆蓋 accumulator。空快照定義為所有 token count 欄位皆為 0 或 None：input_tokens == 0、output_tokens == 0、cache_read_tokens.unwrap_or(0) == 0、cache_write_tokens.unwrap_or(0) == 0、reasoning_tokens.unwrap_or(0) == 0。TokenUsage.extras 不參與判定，因為 extras 保存原始 JSON 供觀測，不是正規化後的 token count。

合法的第一筆全 0 cumulative usage 不需要特殊處理：accumulator 初始值本來就是全 0，忽略該 addend 與覆蓋成全 0 的 RunLog 結果相同。若同一 invocation 已經累積非零 usage，後續全 0 cumulative 快照不符合 running total 語意，忽略它可保留最後一筆可用累計值。

部分欄位改名或缺失時不做 per-field merge。只要 addend 任何正規化 token count 非零，就仍視為非空 cumulative snapshot 並整體覆蓋。原因是 Cumulative 語意的基本單位是整張快照；把舊欄位與新欄位混合會製造不存在於 provider output 的合成快照。若部分欄位可解但皆為 0，sink 會把它視為空快照而不覆蓋既有值。

替代方案：讓 codex parser 在全欄位 missing 時不 emit Usage。未採用，因為 sink 的防護應適用所有 Cumulative backend，且 parser 保留目前「usage 存在就產生 Usage」的形狀可縮小行為變更。

### Codex usage warning fires only when no expected field decodes

codex parser 對 turn.completed.usage 仍查詢四個既有欄位：input_tokens、output_tokens、cached_input_tokens、reasoning_output_tokens。當 usage object 存在，且四個欄位全部不存在或不是 u64 時，parser emit exactly one stderr warning，prefix 為 warning: codex usage，並繼續依目前映射產生 Usage。warning 不輸出整個 usage JSON，避免把未知 provider payload 原文寫到 stderr。

不觸發 warning 的情況：turn.completed 沒有 usage object；至少一個預期欄位成功解碼成 u64；預期欄位存在且值為 0；非 turn.completed 行。這保留 codex 0.136 的正常路徑，也避免把合法零值誤報成欄位 rename。

測試方式採不改 public parser interface 的內部 warn sink。保留 parse_codex_line(raw: &str) -> Vec<StreamEvent> 與 AgentBackend::parse_stream_line 介面；新增內部 helper 接受 warning callback 或回傳 warning flag，public function 只負責把 warning 寫到 stderr。單元測試以 helper 驗證 warning 觸發條件，不依賴 process-level stderr capture。

## Implementation Contract

Observable behavior:

- Cumulative token usage 合併時，非零快照仍 last-wins；全 0 快照不覆蓋既有 accumulator。
- Delta token usage 合併完全維持 field-wise sum。
- codex turn.completed.usage 使用現行欄位名且含非零數值時，parse_stream_line 回傳的 TokenUsage 與既有 turn_completed_maps_usage test 相同。
- codex turn.completed.usage 存在但四個預期欄位全數無法解碼時，parser 發出一行 warning: codex usage 前綴的 stderr warning，且 Usage event 的正規化 token count 保持全 0 / None，交由 cumulative guard 避免覆蓋既有非零值。

Interfaces and data shape:

- Public parse_codex_line 與 AgentBackend::parse_stream_line signatures 不變。
- TokenUsage、RunLog、StreamEvent serialized JSON shape 不變。
- TokenUsageSemantics enum 不新增 variant。
- apply_token_usage 的呼叫點不新增 provider identity 判斷。

Acceptance criteria:

- codebus-core/src/stream/codex_parser.rs 保留並通過 turn_completed_maps_usage，該 test 的欄位值不變。
- 新增 parser test：usage object 只有 renamed / unknown 欄位時，warning callback 收到 exactly one warning，並仍產生 Usage(TokenUsage::default-like with extras preserved)。
- 新增 parser test：usage object 至少一個預期欄位成功解碼時，不產生 missing-fields warning。
- 新增 sink test：Cumulative 先收到非零 Usage，再收到全 0 Usage，accumulator 保持第一筆非零值。
- 既有 sink test：Cumulative 100 then 250 仍得到 250，不得到 350。
- Cargo test target 至少涵蓋 codex_parser 與 log::sink 單元測試。

Scope boundaries:

- In scope: codebus-core/src/stream/codex_parser.rs parser warning；codebus-core/src/log/sink.rs cumulative empty guard；相關單元測試。
- Out of scope: claude parser、Delta 合併、RunLog schema migration、events-log schema、GUI 顯示、token-driven branching。

## Risks / Trade-offs

- [Risk] 真實 provider 可能在同一 invocation 產生「非零後全 0」的 cumulative 快照。Mitigation: cumulative running total 語意不允許重置；若第一筆就是全 0，忽略與覆蓋結果等價。
- [Risk] 部分欄位 rename 且另一欄位非零時，仍可能以部分快照覆蓋完整舊快照。Mitigation: 此 proposal 只消除全欄位 rename 的靜默歸零；部分 rename 會保留 provider 的當前正規化快照，不合成 per-field 混合值。
- [Risk] stderr warning 增加 noise。Mitigation: warning 只在 usage object 存在且四個已知欄位全數無法解碼時發生；正常 codex 0.136 usage 不觸發。
