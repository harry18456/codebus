## Why

A `v3-goal-library`（2026-05-13 archived）讓 GUI 能 reuse CLI 的 verb orchestration，但 GUI 還缺兩個 run-log 側基建才能 ship `v3-app-workspace-goal`（C）的完整 UX：

1. **Cancelled run 在 Goals overview 看不到** — A 的 cancel 路徑跳過 RunLog 寫入，導致 user 取消的 goal 在歷史列表「消失」。Design doc §4.2.3 要求 cancelled row 跟 succeeded / failed 一樣可見、可點開 detail view。
2. **Completed-goal detail view 看不到 timeline** — A 的 stream events 只 live render 到 stdout（CLI 端透過 closure），沒持久化。Design doc §4.3.4 sub-state B 畫了「Stream history collapse ▼」可展開區 — 目前沒資料來源。Cancel UX（discussion §D3）的 partial timeline 與 interrupted-run 偵測（GUI 啟動時 scan 孤兒 events 檔）也同樣靠這份持久化。

完整討論結論見 `docs/2026-05-13-v3-run-log-events-discussion.md`（8 個 Decision + interface depth check + 未來 PII positional log 擴展路徑）。

## What Changes

- **RunLog schema 加 `outcome: String` 欄位** — 合法值 `"succeeded"` / `"failed"` / `"cancelled"`；serde default `"succeeded"` 給舊 jsonl rows forward-compat
- **Cancelled 路徑寫 RunLog**（**行為變更** vs A 既有實作）— `verb::goal::run_goal` / `verb::fix::run_fix` 在 `VerbError::Cancelled` 路徑改為先寫一筆 `outcome: "cancelled"` 的 RunLog 再 return Err；跳過 auto-commit 行為不變
- **新增 `EventsSink` trait** 與既有 `LogSink` 並列（per-event live append 跟 per-run summary 不同 lifecycle）
- **新增 `EventsJsonlSink` impl** — 寫 `<vault>/.codebus/log/events-<started_at_slug>.jsonl`；slug 規則：`started_at` RFC 3339 字串中 `:` 改 `-`（避 Windows 限制，例 `events-2026-05-13T03-25-11Z.jsonl`）；BufWriter + flush per write 確保 crash resilience
- **新增 `EventsNullSink` impl** — opt-out 路徑 no-op
- **events.jsonl 內容格式：每行 envelope** `{"ts": "RFC3339", "event": <serialized VerbEvent>}` — append 時 wall clock 時戳；Banner / Stream / Lifecycle 三種 VerbEvent 都序列化
- **`SinkConfig` 加 events 端對應**（forward-compat：`log.sink: jsonl` → 同時開 runs.jsonl + events.jsonl；`log.sink: none` → 兩者都關，CLI user 一致 opt-out）
- **verb library function 改寫 RunLog + events 寫入路徑** — `verb::{goal,query,fix}::run_*` 內部多走一段 EventsSink 寫入；CLI thin wrapper 不變；GUI thin wrapper（C change 之後才有）可透過 caller-side LogConfig override 強制寫
- **events.jsonl 寫入失敗為非致命** — 同既有 RunLog 策略：stderr `warning: events-log` prefix，不改變 verb exit code

## Non-Goals

- **不加 PII positional log** — 「遮罩了 codebase 哪些位置」UX 留給後續 change；本 change 不在 events.jsonl envelope 加 `VerbLifecycleEvent::PiiFinding` 變體（驗證過未來擴展路徑是純 enum 加 variant，本 change 不擋路）
- **不加 RunLog 的 PII summary 欄位**（`pii_matches` 等）— YAGNI；events.jsonl 已能透過 PiiSummary banner 反推
- **不加 session_id 欄位** — 那是後續 `v3-chat-verb` change 的 schema 擴展（一個 optional 欄位 forward-compat）
- **不改 SinkConfig 的 yaml schema**（`log.sink: jsonl | none`）— events sink 跟著 runs sink 一起開 / 關，不另開 yaml 欄位
- **不引入 events.jsonl 的 cross-file rotation / size limit** — single goal 估計 < 1000 events，單檔可控；rotation 等真有人 ship long-running interactive session 再考慮
- **不為 GUI override 行為加參數到 verb library** — 由 caller（C change 屆時的 GUI thin wrapper）在 LogConfig 載入後強制覆蓋 sink 成 `Jsonl`；verb library 不知道誰呼叫
- **不改 cancel 不 auto-commit 行為** — A 既有 contract 保留
- **不改 CLI 對外行為** — stdout banner / stream render 不變；新增的 events.jsonl 是 side-effect 檔，user 不需要看

## Capabilities

### New Capabilities

- `events-log`: events.jsonl 持久化 capability — `EventsSink` trait、`EventsJsonlSink` / `EventsNullSink` impls、envelope schema `{ts, event}`、檔名 slug 規則、寫入失敗非致命策略、與 LogConfig 的 sink-discriminator 共用 yaml schema 的綁定規則

### Modified Capabilities

- `run-log`: RunLog struct 加 `outcome: String` 欄位（serde default `"succeeded"`）；新增 outcome 值定義（`succeeded` / `failed` / `cancelled` 三值閉集合）；新增 outcome forward-compat 規則（舊 jsonl row 缺欄位 reader 不爆）
- `verb-library`: `verb::goal::run_goal` 與 `verb::fix::run_fix` 的 cancel 路徑改為先寫 RunLog（`outcome: "cancelled"`）再 return `Err(VerbError::Cancelled)`；既有「cancel 不 auto-commit」行為保留；所有三個 `verb::*::run_*` 在 `agent::invoke` 之外多走一段 EventsSink 寫入

## Impact

- Affected specs:
  - 新建 `openspec/specs/events-log/spec.md`
  - 修改 `openspec/specs/run-log/spec.md`
  - 修改 `openspec/specs/verb-library/spec.md`
- Affected code:
  - New:
    - codebus-core/src/log/events/mod.rs
    - codebus-core/src/log/events/sink.rs
    - codebus-core/src/log/events/jsonl_sink.rs
    - codebus-core/src/log/events/null_sink.rs
  - Modified:
    - codebus-core/src/log/sink.rs（RunLog 加 outcome 欄位 + serde default）
    - codebus-core/src/log/mod.rs（pub mod events; pub use 公開 EventsSink 等）
    - codebus-core/src/log/factory.rs（build_events_sink 對應 build_sink 模式，跟同一 SinkConfig 串）
    - codebus-core/src/log/verb_log.rs（load_verb_log_config 路徑沿用；resolve_sink_dir 也應用到 events sink dir）
    - codebus-core/src/verb/goal.rs（cancel 路徑寫 RunLog；on_event 同時送 caller 與 events sink）
    - codebus-core/src/verb/query.rs（on_event 同時送 caller 與 events sink；query 沒 cancel-skip-RunLog 路徑因為 query 本來就寫 RunLog）
    - codebus-core/src/verb/fix.rs（cancel 路徑寫 RunLog；on_event 同時送 caller 與 events sink）
  - Removed: (none)
- Affected dependencies: 無新增 Cargo crate
- Test coverage 影響：
  - 既有 codebus-core verb / log / jsonl_sink unit tests 全綠是驗收門檻
  - 既有 codebus-cli integration tests（cli_routing / goal_flow / query_flow / fix_flow / scoped_env_injection）全綠 — CLI stdout 沒變、RunLog 多了 outcome 欄位（既有 test 不檢查就是新增欄位 forward-compat 不破）
  - 新增 events_log unit tests：envelope 序列化 round-trip、slug 規則、live append + flush 驗 crash partial readable、寫入失敗 stderr warning + 不致命
  - 新增 verb cancel 路徑寫 RunLog 的 unit test（goal + fix cancel 後 `outcome: "cancelled"` RunLog 落地）
