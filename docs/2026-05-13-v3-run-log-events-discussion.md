# v3-run-log-events — 動工前討論結論

> 2026-05-13 spectra-discuss session 紀錄。對應 `/spectra-propose v3-run-log-events` 動工前 reread。
>
> 上游：`docs/v3-app-roadmap.md` §Sequence（B row）、`docs/2026-05-12-v3-app-workspace-goal-discussion.md` §D2 / D2.1（B 範圍由那次 discuss 預定義）。

## TL;DR

v3-app-roadmap §Sequence B `v3-run-log-events` 補完 v3-goal-library（A）未動的兩個 run-log 側基建：

1. **RunLog 加 `outcome` 欄位**（`succeeded` / `failed` / `cancelled`）— 讓 cancelled run 也能進 RunLog 而不只跳過寫入
2. **新增 per-run events.jsonl 持久化** — `<vault>/.codebus/log/events-<started_at_slug>.jsonl`，append 每個 `VerbEvent`（Banner / Stream / Lifecycle 三類），讓 GUI 之後（C `v3-app-workspace-goal`）可以重建 completed goal 的 timeline + cancel UX 的 partial timeline。

設計 provider-agnostic（StreamEvent / TokenUsage 早 normalized）；對未來 PII positional log 留好擴展路徑。

## 範圍前提

A `v3-goal-library` archive 後（2026-05-13），verb library 已經把 stream events 透過 `on_event: impl FnMut(VerbEvent)` 暴露給 caller，events.jsonl 的 sink 注入點自然落在 verb function 構造的 closure 內（callback wrap）。RunLog 已經由 verb library 內部寫；本 change 只擴 schema 不換 caller。

## 觸發點

`docs/2026-05-11-app-ux-flow-design.md` §4.3.4 sub-state B 畫了 completed goal detail view 含「Stream history collapse ▼」可展開區。

實機看 CLI 持久化：

- `openspec/specs/run-log/spec.md` 的 RunLog schema 只記 summary（goal text / mode / model / effort / 時戳 / tokens / wiki_changed / lint counts）
- Stream events（thought / tool calls / banner）只 live render 到 stdout，**沒持久化**
- Cancel 路徑現在（v3-goal-library 既有行為）整段跳過 RunLog 寫入 → cancelled run 在 Goals overview 列表會「消失」

要支援 §4.3.4 sub-state B 的 UX，必須補這兩處。

## 設計問答

### Q1：Log 設計會因為不同 AI provider 受影響嗎？

**不會。** 已驗證 provider-agnostic：

| 層 | Provider 敏感度 | 證據 |
|---|---|---|
| `TokenUsage` struct | 無 | `log/sink.rs:14-42` doc 明寫 "normalized across providers"；`extras` 是 vendor-specific 逃生口 |
| `RunLog` struct | 無 | 沒 provider 識別欄位（`mode` 是 verb，`model` 是 alias） |
| `StreamEvent` enum | shape 無 | parser.rs:43 抽象 4 變體（Thought / ToolUse / ToolResult / Usage） |
| `parse_claude_stream_line` | **Claude 專屬** | parser.rs:50；未來 OpenAI/Gemini 寫自己的 parser 產出同 shape |

events.jsonl 結構 provider-neutral；換 provider 時只有 `tokens.extras` 內容反映新 provider 的 wire format。

### Q2：log 包含 PII 嗎？

**目前沒。** PII 處理在 raw mirror sync time（不是 agent invoke time）：

- `sync_with_scanner` 把 PII 過的內容寫進 `<vault>/.codebus/raw/code/`，agent 看到的已是 mask 過版本
- agent stream events 因此通常不含 PII（除非 agent 自己捏造）
- PiiSummary 是 transient stdout banner，沒持久化

本 change 維持現狀（YAGNI），但驗證了未來加 PII positional log 是純擴展 — 詳見「未來擴展路徑」段落。

### Q3：events.jsonl 寫入頻率？

**Live append（per-event）**：

- Crash resilient — 寫到一半 OS crash 仍能 partial 讀回
- GUI 可 file-tail 即時更新 detail view 的 streaming row（mirror §4.2.3）
- BufWriter flush per write 確保字節抵達 disk（cost 可忽略 — event 間隔通常 > 100ms）

替代方案（batch flush at end）被駁回：cancel UX 直接用 partial events.jsonl 偵測 interrupted run（§D3）需要中間就 visible。

### Q4：`outcome` schema 設計

**`outcome: String`，default `"succeeded"`**（serde default 對舊 jsonl forward-compat）。

合法值：`"succeeded"` / `"failed"` / `"cancelled"`。

- 跟 RunLog 既有 `mode: String` 風格一致（不引入新 enum marshalling 成本）
- 舊 jsonl 缺欄位走 default → reader 不爆
- Enum 型 schema（`Outcome::Succeeded` 等）只有 type-safety 微優，沒換的價值

**Cancelled 路徑也寫 RunLog**（這是行為變更）：

- v3-goal-library 既有 verb::*::run_* 的 cancel 路徑跳過 RunLog 寫入
- 本 change 改成寫一筆 `outcome: "cancelled"` 但 tokens / lint counts 反映 partial 狀態；跳過 auto-commit 行為不變
- 理由：GUI Goals overview list 要列出 cancelled run（mirror §4.2.3 那個 row），需要 RunLog 有那一筆

### Q5：events.jsonl 內容格式

**每行一個 envelope**：

```jsonl
{"ts":"2026-05-13T03:25:11Z","event":{"kind":"banner","banner":{"kind":"sync_start"}}}
{"ts":"2026-05-13T03:25:12Z","event":{"kind":"lifecycle","lifecycle":{"kind":"spawn_start","verb":"goal"}}}
{"ts":"2026-05-13T03:25:13Z","event":{"kind":"stream","stream":{"kind":"thought","text":"I'll read..."}}}
{"ts":"2026-05-13T03:25:14Z","event":{"kind":"stream","stream":{"kind":"tool_use","name":"Read","input":{...}}}}
```

- `ts`：append 時 wall clock（RFC 3339 secs precision），給 GUI replay 時序用
- `event`：serde-serialized `VerbEvent`（已 derive Serialize）
- Banner / Stream / Lifecycle 三類全寫 — GUI 之後可能要顯示 banner 訊息（"同步 N 個檔案"）+ lifecycle 進度 + stream 詳細

替代方案（只存 Stream）被駁回：banner 訊息有意義，丟掉等於失敗。

### Q6：檔名 slug 規則

**`events-<started_at>.jsonl`，`:` 換 `-`**：

- 例：started_at = `2026-05-13T03:25:11Z` → `events-2026-05-13T03-25-11Z.jsonl`
- `started_at` 由 `agent::invoke()` 用 `SecondsFormat::Secs` 構造，不會有 `.123` fractional
- Windows 不允許 `:` 在檔名 — 因此換 `-`
- 同秒衝突（理論）：v1 always at most 1 running goal（roadmap 明文），實務不會撞

### Q7：GUI opt-out override 機制

**Caller-side override，verb library 不知道誰呼叫**：

- CLI：載 `~/.codebus/config.yaml` 後傳進 verb（`log.sink: none` 則兩者都關）
- GUI（C change 才會碰）：載完 yaml 後在 caller-side **強制 override `sink` 成 `Jsonl`** 再傳進 verb
- verb library 一視同仁消費 effective LogConfig — sink config 決定寫不寫

替代方案（library 加 `force_events: bool` 參數）被駁回：把「我是 CLI / GUI」政策推進 library 不乾淨。

## 介面深度檢查（Interface depth check）

新 storage abstraction（events.jsonl 是新檔案類別 + 新寫入語意），檢查觸發。

| 問題 | 答案 |
|---|---|
| **Seam location** | `codebus_core::log::events` 新 module。新 trait `EventsSink { fn write_event(&mut self, env: &EventEnvelope) -> Result<(), LogError> }`。實作 `EventsJsonlSink` + `EventsNullSink`。 |
| **Adapter count** | 1：`EventsJsonlSink`（live append + per-write flush）。`EventsNullSink` 是 no-op opt-out 路徑。 |
| **Depth** | 真實 behavior：(a) `started_at` 取 slug + lazy file create；(b) BufWriter + flush per write 保 crash resilience；(c) write 失敗走 stderr warning 不致命（同 RunLog 策略 `RunLog Write Failure Is Non-Fatal`）；(d) Banner / Stream / Lifecycle 都序列化（每種有 serde `kind` tag） |
| **Deletion test** | 砍掉 events.jsonl → GUI completed-goal detail view「Stream history」沒資料 + interrupted run 偵測（C change §D3）壞 + cancel UX partial timeline 失效。**非 pass-through**。 |

## Decisions（最終結論）

1. **RunLog schema 擴 `outcome: String`**（default `"succeeded"` via serde default）— forward-compat for 舊 jsonl
2. **Cancelled 路徑寫 RunLog**（行為變更 vs v3-goal-library 既有）— `outcome: "cancelled"`，跳過 auto-commit 行為不變
3. **新增 `EventsSink` trait** 與 `LogSink` 並列（per-event live append 跟 per-run summary lifecycle 不同）
4. **EventsJsonlSink** 寫 `<vault>/.codebus/log/events-<started_at_slug>.jsonl`，envelope `{ ts, event: VerbEvent }`
5. **Slug 規則** `:` → `-`，secs precision，無 fractional
6. **GUI opt-out override** 由 caller 端做（CLI 讀 yaml；GUI 載完強制 override 成 Jsonl）
7. **Banner / Stream / Lifecycle 三種 VerbEvent 都寫**（GUI 需要完整 timeline，包含 banner 訊息與 lifecycle 進度）
8. **PII positional log 不在本 change**（YAGNI；但驗證未來擴展路徑乾淨 — 見下節）

## 未來擴展路徑（不在本 change）

### PII positional log — 「遮罩了 codebase 哪些位置」

當 GUI 要 ship 這個 UI：

| 步驟 | 動作 |
|---|---|
| 1 | `SyncSummary` 加 `pii_findings: Vec<PiiFinding>`（path + 既有 `PiiMatch` 欄位 pattern_name / start / end / severity / matched_text） |
| 2 | `VerbLifecycleEvent` 加新 variant `PiiFinding { path, pattern_name, start, end, severity, action: OnHit }`（純 enum 擴展，spec 已預留「MAY 加 variant」） |
| 3 | `verb::goal::run_goal` 在 sync 階段 per match emit `VerbEvent::Lifecycle(PiiFinding { .. })`，自動進 events.jsonl |
| 4 | GUI 讀 events.jsonl filter PiiFinding 變體 → 顯示「被遮罩的位置」列表，或在 wiki preview 對映回 raw/code 標 highlight |

資料現成在 `PiiMatch`（`pii/provider.rs:9-23`），`sync_with_scanner` hot path 已經傳 path — 整條路徑零新基建。

**RunLog 不必加 PII positional 欄位** — 單行 jsonl 變肥；positional 走 events.jsonl 更乾淨。RunLog 若之後要 grep summary（counts），再開另一條小 change 加 `pii_matches: usize` 等 3 個欄位。

### 第二個 AI provider

events.jsonl 結構不變；新 provider 寫自己的 stream parser 產出同 `StreamEvent` shape；`tokens.extras` 換內容。本 change 不擋路。

## 待 confirm 動工點

1. 上面 8 個 Decisions OK？
2. Cancel 路徑寫 RunLog（行為變更）— 接受嗎？需要在 v3-goal-library archive 後的 verb::*::run_* impl 補一條 cancelled 路徑寫 RunLog 的修正
3. Change name `v3-run-log-events` 沿用？

確認 → `/spectra-propose v3-run-log-events`。
