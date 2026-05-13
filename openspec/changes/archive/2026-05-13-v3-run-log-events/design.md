## Context

A `v3-goal-library` 2026-05-13 archive 完，verb library 已經把 stream events 透過 `on_event: impl FnMut(VerbEvent)` 暴露給 caller。CLI thin wrappers 透過 closure 把 events 渲染到 stdout；GUI（未來 C change `v3-app-workspace-goal`）會透過同一 callback emit Tauri event。但 events 本身只是「**transient**」— A 沒持久化任何 stream event。

CLI 現況的持久化只有 RunLog（per-run summary，jsonl 一行）：

- `<vault>/.codebus/log/runs-YYYY-MM-DD.jsonl` — 一行包 `goal / mode / model / effort / 時戳 / tokens / wiki_changed / lint counts`
- 沒有 outcome 欄位（succeeded 隱含）
- Cancel 路徑（A 既有實作）**跳過 RunLog 寫入** — 取消的 run 在 jsonl 不存在

GUI（C change）需要的兩件事：

1. **Goals overview row 列出 cancelled run** — 需要 cancelled 也進 RunLog
2. **Completed-goal detail view 重建 stream history** — 需要 stream events 持久化

完整討論結論 + Q1~Q7 + 8 個 Decisions + 未來 PII positional log 擴展路徑見 `docs/2026-05-13-v3-run-log-events-discussion.md`。

## Goals / Non-Goals

**Goals:**

- RunLog schema 加 `outcome: String` 欄位（合法值 `succeeded` / `failed` / `cancelled`），serde default 給舊 jsonl forward-compat
- Cancel 路徑改為先寫 RunLog 再 return Err — GUI Goals overview 能列 cancelled row
- 新增 EventsSink trait + EventsJsonlSink + EventsNullSink — 三個 spawn verb 內部 live append events.jsonl
- envelope 格式 `{ts, event}` 帶 wall-clock timestamp + 序列化 VerbEvent（Banner / Stream / Lifecycle 三類）
- 檔名 slug 規則 `events-<started_at-with-colon-as-dash>.jsonl`（避 Windows 限制）
- log.sink: none opt-out 一視同仁關掉 runs.jsonl + events.jsonl（CLI 一致）
- GUI override 由 caller 在 LogConfig 載入後強制 `sink: jsonl`，verb library 純消費 effective config
- 未來 PII positional log 走 `VerbLifecycleEvent::PiiFinding` 變體擴展，本 change 不擋路

**Non-Goals:**

- 不加 RunLog `pii_*` summary 欄位（YAGNI；events.jsonl 已能透過 PiiSummary banner 反推）
- 不加 `session_id` 欄位 — 留給後續 `v3-chat-verb` change
- 不引入 events.jsonl rotation / size limit（v1 always at most 1 running goal，單檔 < 1MB 預估）
- 不為 GUI override 行為加 `force_events` 參數到 verb library — caller-side override 更乾淨
- 不改 cancel 不 auto-commit 行為
- 不改 CLI 對外 stdout / stderr / exit code — 全部 byte-equivalent，既有 27+ integration test 不需改
- 不加新 yaml 欄位（`log.events_sink` 之類）— events sink 跟 runs sink 共用 `log.sink` discriminator
- 不引入 tokio / async I/O — 沿用 std sync 寫入

## Decisions

### EventsSink trait 與既有 LogSink 並列，不擴 LogSink

選擇理由：

- `LogSink::write_run(RunLog)` 是「per-run summary」語意（一次調用，傳整份 summary）
- events 是「per-event live append」語意（同一 run 內 N 次調用，傳單個 envelope）
- 兩者 lifecycle 不同、testable 路徑不同；合進同 trait 會逼 NullSink 多一個沒用的 method
- 同 module `log::events` 下放 trait + 兩個 impls，跟既有 `log/sink.rs` + `log/sinks/{null,jsonl}_sink.rs` 結構平行

Alternatives considered：

- 擴 LogSink 加 `write_event` method — 駁回，NullSink 兩個 method 都 no-op 違反 0-cost 假設
- 單一 `RunPersistence` 大 trait 包 RunLog + events — 駁回，太大、testable surface 模糊

### envelope 格式 `{ts, event: VerbEvent}`，三類 event 都寫

選擇理由：

- `ts`：append 時 wall clock（不是 invoke 內 stream 的 timestamp）— GUI replay 看時序、interrupted run 偵測也需要外部 ts 給 GUI 端可靠時序
- `event`：serde-serialized VerbEvent（已 derive Serialize via Stream from v3-goal-library）— Banner / Stream / Lifecycle 三類都序列化
- Banner 寫進去：GUI 想顯示 "同步 N 個檔案 (2.3 MiB)" 等 milestone 訊息，必須能讀到 banner payload
- Lifecycle 寫進去：未來 PII positional log 走 `VerbLifecycleEvent::PiiFinding` 變體擴展，這個位置已預留

Alternatives considered：

- 只存 StreamEvent — 駁回，丟失 banner / lifecycle 訊息
- 不加 envelope，直接 dump VerbEvent serde — 駁回，無外部 ts 給 GUI 用
- 加 sequence number — YAGNI，ts 已單調遞增（同秒衝突極罕見且 GUI 不依賴 strict order）

### 檔名 slug 規則：started_at 全字串 `:` → `-`

選擇理由：

- `started_at` 由 A `agent::invoke()` 用 `SecondsFormat::Secs` 構造，格式固定 `YYYY-MM-DDTHH:MM:SSZ`
- Windows 不允許檔名含 `:`，必須換掉
- 換成 `-` 是最少代換 + 仍可逆（GUI 可從檔名反推 started_at）
- 例：`2026-05-13T03:25:11Z` → `events-2026-05-13T03-25-11Z.jsonl`
- 同秒衝突理論可能但 v1 always at most 1 running goal（roadmap 明文）— 不會撞

Alternatives considered：

- URL-encode（`%3A`）— 駁回，醜且不必要
- 加 random suffix — YAGNI，沒實際衝突
- 用 UUID 當檔名 + 內部欄位記 started_at — 駁回，破壞 GUI 從檔名直接 group by started_at 的方便性

### Cancel 路徑寫 RunLog（行為變更）

選擇理由：

- A 既有：`VerbError::Cancelled` → return Err，跳過 RunLog 寫入
- 本 change：寫一筆 `outcome: "cancelled"` 的 RunLog 後再 return Err
- 理由：GUI Goals overview list 要列出 cancelled run（design doc §4.2.3）— 沒 RunLog 就沒這個 row
- tokens / lint counts 反映 partial state（accumulate 到 cancel 時點的累積值）
- auto_commit 仍然跳過（既有 contract）— `outcome: "cancelled"` 但沒 commit

Alternatives considered：

- 不寫 RunLog，GUI 從 events.jsonl 推 cancelled — 駁回，GUI Goals overview list 是 grep RunLog，多繞一層
- 寫 RunLog 但 outcome 用 `"interrupted"` 細分 — driver kill vs user cancel 對 user 沒差，留閉集合 3 值簡單
- 寫 RunLog 也 auto-commit half-baked wiki — 駁回，A 的 cancel-no-commit 是明確設計（partial writes 留在 working tree，user 自己決定）

### log.sink: none 對 events sink 一視同仁；GUI override 由 caller 做

選擇理由：

- CLI user 想 opt-out log 動機合理（自己有 pipeline）— `none` 同時關 runs + events 行為一致
- GUI Goals overview / detail view 依賴 events.jsonl 為唯一資料來源 — 不能讓 user 砍自己腳
- 解決：GUI thin wrapper（C change 屆時）在 `load_verb_log_config()` 載完後，**強制** override `sink` 成 `Jsonl`（如果原本是 `Null`）再傳進 verb function
- verb library 一視同仁消費 effective LogConfig — 不知道誰呼叫

Alternatives considered：

- verb library 加 `force_events: bool` 參數 — 把政策推進 library，library 不該知道誰呼叫
- yaml 加 `log.events_sink` 獨立欄位 — 兩個 sink 各自 opt-out，破壞 CLI user 一致 opt-out 預期
- GUI 寫到別的目錄繞開 — 增加複雜度且 GUI / CLI 看到的歷史不同

### Crash resilience：每個 event 寫 + flush

選擇理由：

- BufWriter::write_all + immediate flush 確保字節抵達 OS page cache
- 不保證 fsync（會太慢），但 GUI 在 process 崩 / kill 後讀回 events.jsonl 拿到完整資訊（只丟最後極少數 events，由 OS page cache → disk lag）
- v1 always at most 1 running goal，writer 競爭不存在

Alternatives considered：

- 全程 BufWriter 不 flush，process exit drop 時 flush — 駁回，cancel / panic 路徑會丟資料
- 每 N events flush 一次 — 增加複雜度，event 間隔本來 > 100ms，per-event flush cost 不重要
- 用 `sync_all`（fsync）強保證 — 太慢，OS 崩潰才需要這層；v1 不值得

### Write failure 為 non-fatal

選擇理由：

- 跟既有 RunLog `RunLog Write Failure Is Non-Fatal` 一致：stderr `warning: ...` prefix + verb 繼續正常 exit
- events.jsonl 寫失敗代表磁碟 / permission 問題，verb 仍然完成了 agent 工作，user 不希望因此被斷掉
- GUI 看不到 events.jsonl 會 fallback 到「只能看 RunLog summary」— 降級而非 fail

Alternatives considered：

- 寫失敗 → return VerbError — 駁回，跟 RunLog 不一致 + 影響 user-visible behavior
- 寫失敗 → panic — 駁回，明顯過頭

### EventsJsonlSink 在 verb function 內部建構

選擇理由：

- 沿用 A 既有的 `write_run_log(sink_cfg, &run_log)` pattern：verb function 內部 `build_events_sink(sink_cfg)` 拿一個 sink，跑完 run 結束（或 cancel 觸發 return）前 sink 自然 drop
- 不引入 caller 構造 + 傳 `&mut dyn EventsSink` 給 verb — 增加 API surface 且不對應 RunLog 模式
- on_event closure 內部 fan-out：先寫 events sink，再 forward 給 caller closure

Alternatives considered：

- Caller 構造 sink + 傳 reference — 增加 caller 端模板碼 + 不對應 RunLog 模式
- 把 events sink 也走 `LogConfig` 但獨立 build 函數 `build_events_sink` — 採用（同一 `SinkConfig` discriminator，獨立 factory）

## Implementation Contract

### Behavior

- **CLI 對外行為 byte-equivalent**：stdout banner / stream render / stderr error / exit code 與 A archive 後完全一致
- **RunLog jsonl 多一個欄位**：`"outcome":"succeeded"` / `"outcome":"failed"` / `"outcome":"cancelled"` 出現在每一筆新寫的 row
- **Cancel 路徑：寫一筆 RunLog**（行為變更）— goal / fix 在 cancel 觀察後 emit RunLog with `outcome: "cancelled"`、partial tokens、lint counts 為 0（cancel 發生在 fix loop 前）或反映 partial（cancel 發生在 fix loop 後）；不 auto-commit
- **events.jsonl 新檔案類別**：每次 spawn verb（goal/query/fix）都會在 `<vault>/.codebus/log/events-<slug>.jsonl` 寫一份；`sink: none` 時不寫
- **per-event live append**：每個 VerbEvent 一行 envelope，flush 在 process 結束前可被 GUI / file-tail 讀回

### Interface / data shape

**RunLog 新欄位**（`codebus-core/src/log/sink.rs`）：

```
pub struct RunLog {
    pub goal: String,
    pub mode: String,
    // ... 既有欄位 ...
    pub wiki_changed: bool,
    pub lint_error_count: usize,
    pub lint_warn_count: usize,
    #[serde(default = "default_outcome")]
    pub outcome: String,  // "succeeded" / "failed" / "cancelled"
}

fn default_outcome() -> String { "succeeded".to_string() }
```

**EventsSink trait**（`codebus-core/src/log/events/sink.rs`）：

```
pub struct EventEnvelope {
    pub ts: String,            // RFC 3339 wall-clock at append
    pub event: VerbEvent,      // serialized via existing serde
}

pub trait EventsSink: Send + Sync {
    fn name(&self) -> &str;
    fn write_event(&mut self, envelope: &EventEnvelope) -> Result<(), LogError>;
    fn flush(&mut self) -> Result<(), LogError> { Ok(()) }
}
```

**EventsJsonlSink**（`codebus-core/src/log/events/jsonl_sink.rs`）：

```
pub struct EventsJsonlSink {
    dir: PathBuf,
    file: Option<BufWriter<File>>,  // lazy create on first write
    target_path: Option<PathBuf>,
}

impl EventsJsonlSink {
    pub fn new(dir: PathBuf, started_at: &str) -> Self {
        let slug = started_at.replace(':', "-");
        let target_path = dir.join(format!("events-{slug}.jsonl"));
        Self { dir, file: None, target_path: Some(target_path) }
    }
}

impl EventsSink for EventsJsonlSink {
    fn name(&self) -> &str { "jsonl" }

    fn write_event(&mut self, envelope: &EventEnvelope) -> Result<(), LogError> {
        if self.file.is_none() {
            std::fs::create_dir_all(&self.dir)?;
            let f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.target_path.as_ref().unwrap())?;
            self.file = Some(BufWriter::new(f));
        }
        let mut line = serde_json::to_string(envelope)?;
        line.push('\n');
        let writer = self.file.as_mut().unwrap();
        writer.write_all(line.as_bytes())?;
        writer.flush()?;  // per-write flush for crash resilience
        Ok(())
    }
}
```

**EventsNullSink**：

```
pub struct EventsNullSink;
impl EventsSink for EventsNullSink {
    fn name(&self) -> &str { "null" }
    fn write_event(&mut self, _envelope: &EventEnvelope) -> Result<(), LogError> { Ok(()) }
}
```

**Factory**（`codebus-core/src/log/factory.rs`）：

```
pub fn build_events_sink(
    cfg: &SinkConfig,
    started_at: &str,
) -> Result<Box<dyn EventsSink>, SinkError> {
    match cfg {
        SinkConfig::Null {} => Ok(Box::new(EventsNullSink)),
        SinkConfig::Jsonl { dir: Some(dir) } => {
            Ok(Box::new(EventsJsonlSink::new(dir.clone(), started_at)))
        }
        SinkConfig::Jsonl { dir: None } => Err(SinkError::Setup("dir unresolved".into())),
    }
}
```

**verb function 內部串接**：

`codebus-core/src/verb/goal.rs` 內部（pseudo-code）：

```
let log_cfg = load_verb_log_config()?;
let effective_sink_cfg = resolve_sink_dir(log_cfg, &paths.log);

let started_at_for_sink = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
let mut events_sink = build_events_sink(&effective_sink_cfg, &started_at_for_sink)?;

// on_event closure: fan-out to caller + events sink
let user_on_event = &mut on_event;
let wrapped = |event: VerbEvent| {
    let envelope = EventEnvelope {
        ts: chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        event: event.clone(),
    };
    if let Err(e) = events_sink.write_event(&envelope) {
        eprintln!("warning: events-log write failed (non-fatal): {e}");
    }
    user_on_event(event);
};

// ... call invoke / fix loop with wrapped closure ...

// Cancel path: write RunLog with outcome: cancelled
if cancelled {
    let run_log = RunLog { /* ... */, outcome: "cancelled".into() };
    write_run_log(/* run sink */, &run_log);
    // skip auto_commit (既有行為)
    return Err(VerbError::Cancelled);
}

// Success path
let run_log = RunLog { /* ... */, outcome: "succeeded".into() };
write_run_log(/* run sink */, &run_log);
```

### Failure modes

- **events.jsonl 寫失敗**（disk full / permission / etc.）→ stderr `warning: events-log write failed (non-fatal): {error}` + verb 繼續正常 exit
- **events sink build 失敗**（`SinkError::Setup`）→ stderr `warning: events-log sink build failed (skipping persistence): {error}` + verb 跳過 events 寫入但繼續 run（fallback to no-op events 行為）
- **舊 RunLog jsonl row 缺 `outcome` 欄位** → reader 透過 `#[serde(default = "default_outcome")]` 自動填 `"succeeded"`
- **未知 outcome 值**（reader 端讀到 e.g. `"interrupted"`）→ serde 接受 String 型別，呼叫端決定怎麼解；分析工具看到非閉集合值可警告
- **slug 衝突**（同秒兩 run）→ v1 不應發生（at most 1 running goal）；若真衝突 EventsJsonlSink 因為 `OpenOptions::append(true)` 會 append 進同一檔（兩 runs 的 events 交錯），GUI 端用 envelope `ts` + verb lifecycle 邊界拆 — 接受退化但不 crash
- **`SinkConfig::Jsonl { dir: None }` 傳進 build_events_sink** → 同既有 build_sink 行為，回 SinkError::Setup（caller 必須先 resolve dir）

### Acceptance criteria

- 既有 codebus-core 331+ unit tests 全綠（含 verb / log / agent / wiki::fix / sink 全部）
- 既有 codebus-cli integration tests 全綠且 golden 未改動（CLI stdout / stderr / exit code byte-equivalent；RunLog 多 outcome 欄位 forward-compat）
- 新增 codebus-core events log unit tests：
  - `EventsJsonlSink::new` 構造後 target_path 結尾 `.jsonl`、slug 結尾 `Z`、無 `:` 字符
  - `write_event` 第一次呼叫後檔案存在、line ends with `\n`、parsed JSON 含 `ts` + `event` keys
  - 多次 write 後 line count 對應 envelope 數
  - `EventsNullSink::write_event` 不創檔不 error
  - `build_events_sink` dispatch null / jsonl 對應，`Jsonl { dir: None }` 回 Err
  - `RunLog` outcome serde default：缺欄位 jsonl deserialize 後 `outcome == "succeeded"`
  - `RunLog` outcome round-trip：serialize then deserialize 三個合法值
- 新增 codebus-core verb cancel-writes-RunLog unit tests：
  - `verb::goal::run_goal` 在 cancel signal flip 後，回 `Err(VerbError::Cancelled)` 前已呼叫 `write_run_log` 一次（mock LogSink 計數），entry 的 outcome 為 `"cancelled"`，wiki_changed reflect partial state
  - `verb::fix::run_fix` cancel 路徑同上
  - `verb::query::run_query` 因為 query 本來就寫 RunLog，cancel 路徑保持 outcome `cancelled` 一致
- 新增 codebus-core verb-emits-events unit tests：
  - 三個 `run_*` function 在 mock EventsSink 上能觀察到 SpawnStart / Stream / Banner 等 envelope，順序與 on_event closure 看到的一致
- `cargo build --workspace` 0 new warning（pre-existing baseline 不算）
- Manual e2e on Windows MSVC：對 `D:/side_project/uv` vault 跑 query → 檢查 `<vault>/.codebus/log/events-*.jsonl` 落地、內容 jq 可 parse；跑 goal 含中途 Ctrl+C → 檢查 RunLog 多一筆 `outcome: cancelled` row + events.jsonl 含 partial timeline；對照 refactor 前 baseline stdout byte-equivalent

### Scope boundaries

**In scope:**

- RunLog schema 加 outcome 欄位 + serde default
- `codebus_core::log::events` 新 module + EventsSink trait + EventsJsonlSink + EventsNullSink + EventEnvelope struct
- `codebus_core::log::factory::build_events_sink` 對應 build_sink 模式
- verb function 內部 wrap on_event closure 同時 fan-out 到 events sink
- verb cancel 路徑改為先寫 RunLog 再 return Err
- verb success / failure 路徑都帶 outcome 進 RunLog
- 既有 codebus-cli integration tests 通過 + 新增 events_log unit tests + verb cancel/events unit tests

**Out of scope:**

- PII positional log（VerbLifecycleEvent::PiiFinding 變體 + sync_with_scanner 累積 findings）— 未來 change
- RunLog pii_* summary 欄位 — 未來 change
- session_id 欄位（chat 用）— 未來 `v3-chat-verb` change
- yaml 新增 `log.events_sink` 獨立欄位 — events sink 共用 `log.sink` discriminator
- events.jsonl rotation / size limit
- async / tokio
- CLI 行為任何變化（stdout / stderr / exit code byte-equivalent）
- GUI（Tauri）— 完全是 C / D 之後 change scope

## Risks / Trade-offs

- **Cancel 路徑寫 RunLog 是行為變更** → A archive 後既有測試假設 cancel 不寫 RunLog 的會 break → Mitigation：A 的 cancel 是 GUI-only 路徑（CLI 永遠傳 `None` cancel signal），CLI 端 integration tests 不會 hit 這條 path；新加的 unit test 直接驗 cancel-writes-RunLog 為新的 spec 行為
- **events.jsonl 多檔案佔空間** → 每 run 一檔，user 跑 100 個 goal = 100 個 .jsonl → Mitigation：每檔 < 1 MB 預估，user 想清就 `rm <vault>/.codebus/log/events-*.jsonl`；rotation 等真有人喊不夠再加
- **slug 衝突極罕見但理論存在** → 同秒兩 run（v1 不會但 future multi-goal v2 可能）→ Mitigation：`append(true)` 不 crash、events 交錯；GUI 用 lifecycle 邊界拆；v2 多 goal 真要做時加 random suffix
- **Per-write flush 對 high-throughput agent 可能慢** → events 間隔 < 100ms 時 BufWriter+flush 每次 1-2ms → Mitigation：實測量 < 1% overhead；若觀察 regression 改成 N events flush 一次
- **on_event closure 同時 fan-out 兩個 sink，borrow checker 可能難搞** → Rust 借用：events_sink 是 mut，user closure 也是 mut → Mitigation：在 verb function 內部用 local mut variable + 明確 ownership；如 lifetime 卡，用 `RefCell` 局部處理（不寫進 spec surface）
- **舊 RunLog reader 看到 outcome 缺欄位** → serde default 處理，但若 user 自己的 jq pipeline 沒 update 假設每 row 必有 outcome 可能誤判 → 接受小破壞，README 更新建議讀者用 `// 'succeeded'` fallback
- **events sink build 失敗在 verb function 內部** → spawn 前就回 SinkError 還是繼續 run 但跳過 events？ → Mitigation：選後者（continue + stderr warning），verb 工作不該因 log 基建問題被斷
