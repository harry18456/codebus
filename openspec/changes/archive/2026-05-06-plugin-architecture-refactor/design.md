## Context

CodeBus 0.2.0 剛完成 TS → Rust 重寫（archived `2026-05-05-rust-rewrite`），架構是 Cargo workspace + 3 crate（codebus-core / cli / app placeholder）。`codebus-core` 內部按 domain 分（`wiki/`、`vault/`、`stream/`、`fs/`、`git/`、`llm/`、`schema/`），其中只有 `llm/` 已經有 trait 抽象（`LlmProvider`），但：

- 只有一個 impl（`ClaudeCliProvider`），在 `codebus-core/src/llm/claude_cli.rs`
- 選擇邏輯硬寫在 `codebus-cli/src/main.rs:215` 跟 `:277`：`ClaudeCliProvider::new()`
- 沒有 factory、沒有 config 驅動

接下來 README roadmap 列出的 next stops 涉及大量擴展：

```
1. PII filter (multi-provider: regex / presidio / aws / 自訂 ML)
2. Multi-LLM provider (Anthropic API direct / OpenAI / 本地)
3. Restore ~/.codebus/config.yaml (regression from TS 0.1.0)
4. Token usage & log tracking
5. Lint feedback loop
6. Query gap detection
7. Disk preflight
8. Multi-platform binary release + CI
```

其中 #1 / #2 / #4 / #5 都會新增 / 刪除 / 切換實作。若不先把 plugin pattern 鋪好，每個 next stop 開發都要動 main.rs 加 if/else 分支，技術債 compound。

額外、`codebus-cli/src/ui.rs::render_event` 也是 plugin 點 — Phase E（codebus-app Tauri shell）需要把同一個 `StreamEvent` 流推到 webview，現在硬寫 println 那條路徑是阻塞。

## Goals / Non-Goals

**Goals:**

- 5 個 plugin domain 各自獨立、加新 impl 阻力極低（加檔 + factory 加 1 條 match arm 即完成）
- 所有 plugin 的 config 統一來自 `~/.codebus/config.yaml`，CLI flag / 環境變數可 override
- 預設行為與 0.2.0 完全一致（除了 `~/.codebus/config.yaml` 重新支援這個 regression-fix）
- 為 #1 PII filter 與 #2 Multi-LLM provider 鋪好骨架，後續 propose 時不必再做架構性改動
- 為 Phase E codebus-app（Tauri）保留 hook 點：`EventRenderer` trait 讓 Tauri 直接掛 `TauriEmitter` impl
- 為 #4 Token tracking 鋪好骨架：`LogSink` trait 讓寫 jsonl / OTel / Null 都是同一介面

**Non-Goals:**

- **不加任何新 LLM provider 實作**（Anthropic API direct / OpenAI / Ollama 是 #2 的 scope）
- **不啟用 PII 掃描**（trait + impl 進來，但 `raw_sync` 不接 scanner、行為與 0.2.0 一致）
- **不啟用 LogSink 寫檔**（trait + impl 進來，但 goal command 用 `Null` sink）
- **不導入動態載入 / 第三方 plugin**（compile-time match factory 即可）
- **不改變 vault 格式或 CLI 行為契約**
- **不重設計 lint 規則本身**（6+ 條既有 rule 一條不刪一條不加，僅做檔案結構重組）

## Decisions

### Register 機制：手動 factory match，不用 inventory / linkme

**選擇**：每個 domain 一個 `factory.rs`，內部就是顯式 `match cfg.kind { ... }` 對應到具體 provider 的 constructor。

```rust
// codebus-core/src/llm/factory.rs
pub fn build_provider(cfg: ProviderConfig) -> Result<Box<dyn LlmProvider>, ProviderError> {
    match cfg.kind {
        ProviderKind::ClaudeCli => Ok(Box::new(providers::claude_cli::ClaudeCliProvider::new(cfg)?)),
        // 加新 provider = 上面 ProviderKind enum 加 variant + 這裡加 arm
        // 兩處改動、一目了然
    }
}
```

**為什麼**：
- codebus 的 plugin 數量可預期 ≤ 5-10 個 / domain，linker 編譯期都看得到、不需要 runtime discovery
- 顯式 match 對 reader 友善 — 看到 enum 就知道全部選項，不用全文搜「誰 submit 了」
- 0 new dep、boring is good

**Alternatives considered**：

- **`inventory` crate 自動註冊**：每個 impl 加 `submit!` 巨集自動進註冊表，factory 不用 match。否決理由：codebus 規模還沒到「不知道有哪些 impl」的程度；多一個 dep + 多一層 magic；linker 細節 debug 起來更麻煩
- **`linkme` distributed slice**：類似 inventory 但用 linker section 機制。同上否決
- **生成式（build.rs 掃 providers/ 自動生 factory）**：build complexity 不值得

### Trait 同步性：LLM async、其他 sync

**選擇**：

| Trait | sync / async | 原因 |
|---|---|---|
| `LlmProvider` | **async**（既有） | 網路 I/O、token streaming 必要 |
| `PiiScanner` | sync | regex 是 CPU、Presidio HTTP 在 impl 內 spawn_blocking 包起來 |
| `LintRule` | sync | 純 fs walk + struct 比對，純 CPU |
| `EventRenderer` | sync | println / Tauri emit 是 fire-and-forget，不需 await |
| `LogSink` | sync | file write 加 `BufWriter`，OTel 內部自己 batch |

**為什麼**：
- async trait 心智負擔大（lifetime、`Send`、`Pin<Box<dyn Future>>`），對純 CPU/local-IO 的 plugin 是純成本
- 大部分 plugin impl 是 sync-friendly 的場景
- 真要 cloud HTTP，可在 sync trait 內用 `tokio::runtime::Handle::current().block_on()` 或 `spawn_blocking` 包起來，封裝在 impl 內、trait 不用感知

**Alternatives considered**：

- **全 async**：對稱、無心智落差。否決：同上心智成本、且 99% impl 不需要
- **全 sync（含 LLM）**：要在 main.rs 做 sync→async 跨度。否決：`LlmProvider` 已 async、breaks 現有 contract

### Cargo features：default lean、heavy dep 切 feature

**選擇**：

```toml
[features]
default = []  # Tier 0：lean default 不帶任何重型 dep
# Tier 1（輕量、預設仍編出來、用 config 切換）：
#   - regex_basic PII scanner（regex crate）
#   - claude_cli LLM provider（已有，零新 dep）
#   - jsonl LogSink（serde_json，已有）
#   - terminal renderer（已有）

# Tier 2（中等 dep、cargo install 時用 --features 帶）：
llm-anthropic-api = ["dep:reqwest", "dep:eventsource-stream"]
llm-openai = ["dep:async-openai"]
pii-presidio = ["dep:reqwest"]

# Tier 3（重型 dep、~50MB+ 編譯產物）：
pii-aws = ["dep:aws-sdk-comprehend"]
log-otel = ["dep:opentelemetry", "dep:opentelemetry-otlp"]

# 便利集合：
all-llm = ["llm-anthropic-api", "llm-openai"]
all-pii = ["pii-presidio", "pii-aws"]
all = ["all-llm", "all-pii", "log-otel"]
```

**為什麼**：
- `cargo install codebus`（無 features）開箱即用、binary 小、編譯快
- 進階用戶 `cargo install codebus --features all-llm,pii-presidio` 仍簡單
- AWS SDK 真的很大（~50MB 編譯產物 + 數十個 transitive dep），預設帶會懲罰所有人

**factory 對未編譯 feature 的處理**：

```rust
match cfg.kind {
    ProviderKind::ClaudeCli => Ok(Box::new(...)),

    #[cfg(feature = "llm-openai")]
    ProviderKind::OpenAi => Ok(Box::new(providers::openai::Provider::new(cfg)?)),

    #[cfg(not(feature = "llm-openai"))]
    ProviderKind::OpenAi => Err(ProviderError::FeatureNotCompiled {
        feature: "llm-openai",
        hint: "rebuild with: cargo install codebus --features llm-openai",
    }),
}
```

**Alternatives considered**：

- **全包進 default**：cargo install 開箱所有 provider 可用。否決：AWS SDK / OTel 真的太大
- **只給 trait、impl 全外部**：codebus-core 只 ship 抽象、各 provider 是獨立 crate。否決：split-crate 對 0.x 規模過頭、cross-crate 編譯反而慢

### 統一 `~/.codebus/config.yaml` schema

**選擇**：所有 plugin domain + 既有 emoji 設定統一在一個 yaml：

```yaml
# ~/.codebus/config.yaml
emoji: auto

llm:
  provider: claude_cli         # claude_cli | anthropic_api | openai | ollama_local
  # provider-specific（serde untagged enum 自動 dispatch）
  binary_path: claude          # only for claude_cli
  timeout_secs: 1800
  # api_key: ...               # only for anthropic_api / openai

pii:
  scanner: null                # null | regex_basic | presidio | aws
  on_hit: warn                 # warn | skip | mask
  patterns_extra:              # regex_basic 額外 pattern
    - 'INTERNAL-\d{6}'

lint:
  page_size_overrides:
    "wiki/synthesis": 10240    # 加大門檻
  disabled_rules: []           # 黑名單規則
  custom_rules_dir: null       # 未來 user-defined rules

render:
  format: terminal             # terminal | json_lines | (tauri 是 codebus-app 用)

log:
  sink: null                   # null | jsonl | otel
  retention_days: 30
```

**為什麼**：
- `Cargo.toml` 是 monorepo config 的 SOTA，一檔對 user 更直覺
- serde untagged enum + section + sub-config 在 Rust 已成熟模式

**Alternatives considered**：

- **多檔（`~/.codebus/llm.yaml` / `pii.yaml` / ...）**：domain 隔離。否決：clutter user home dir、心智「改設定要找哪個檔」變難
- **TOML 或 JSON**：YAML 已是 Obsidian / GitHub Actions / 大部分 dev tool 預設、user 熟悉

### 預設行為中性、僅一處 user-visible 改動

**選擇**：除以下一項，所有 plugin 預設 impl 行為與 0.2.0 一致：

- ✅ **`~/.codebus/config.yaml` 重新支援**（emoji 等 user-level 預設恢復為 5 級優先序：CLI flag > `--no-emoji` sugar > `NO_EMOJI` env > config.yaml > auto-detect）
- ✅ PiiScanner default = `Null`（不掃，與 0.2.0 raw_sync 行為一致）
- ✅ LintRule 全部仍跑（與 0.2.0 lint 結果一致）
- ✅ EventRenderer default = `TerminalRenderer`（與 0.2.0 `render_event` 輸出 byte-equal）
- ✅ LogSink default = `Null`（不寫 log，與 0.2.0 行為一致）
- ✅ LlmProvider default = `ClaudeCli`（與 0.2.0 一致）

**為什麼**：refactor 應該是 100% behavior-neutral 才好做 conformance；`~/.codebus/config.yaml` 是補回 TS 0.1.0 regression、不算「新加 feature」、且該 file 預設不存在仍走 auto/CLI flag

### Crate 結構：5 個 plugin domain 全在 codebus-core

**選擇**：

```
codebus-core/src/
├─ lib.rs
├─ config/             ← NEW: ~/.codebus/config.yaml loader + schema
├─ llm/                ← 既有，重構
│  ├─ provider.rs      (trait，既有)
│  ├─ factory.rs       NEW
│  └─ providers/       NEW: 子資料夾
│     ├─ mod.rs
│     └─ claude_cli.rs (從 codebus-core/src/llm/claude_cli.rs git mv 進來)
├─ pii/                NEW
│  ├─ provider.rs      (trait)
│  ├─ factory.rs
│  └─ scanners/
│     ├─ null_scanner.rs
│     └─ regex_basic.rs
├─ wiki/lint/          ← 從單檔 lint.rs 升資料夾
│  ├─ mod.rs           (re-export)
│  ├─ rule.rs          (LintRule trait)
│  ├─ factory.rs
│  └─ rules/           (拆現有 lint.rs 為每條 rule 一檔)
│     ├─ page_size.rs
│     ├─ unexpected_file.rs
│     ├─ duplicate_slug.rs
│     ├─ missing_nav.rs
│     ├─ root_page.rs
│     ├─ broken_wikilink.rs
│     └─ frontmatter_integrity.rs
├─ render/             NEW
│  ├─ event_renderer.rs (trait)
│  ├─ factory.rs
│  └─ renderers/
│     └─ terminal.rs
├─ log/                NEW
│  ├─ sink.rs          (trait)
│  ├─ factory.rs
│  └─ sinks/
│     ├─ null_sink.rs
│     └─ jsonl_sink.rs
└─ ...（vault / stream / fs / git / schema 不動）
```

**為什麼**：codebus-core 總 LOC 預估 ~6-8K（plugin 全部進來後），仍是「small crate」級；split crate 反而拖慢 incremental build；workspace 已 3 crate（core/cli/app）夠清楚

**Alternatives considered**：

- **拆 `codebus-llm` / `codebus-pii` / `codebus-lint` 三個 sub-crate**：clean separation。否決：現階段太小、cross-crate 編譯成本 + 版本同步成本過頭；真要拆等到 codebus-app 想單獨依賴某 plugin domain 時再做（feature flag 也可以解）

### Trait shape：object-safe + Box<dyn T>

**選擇**：所有 plugin trait 都 object-safe，factory 回 `Box<dyn T>`。

```rust
pub trait PiiScanner: Send + Sync {
    fn scan(&self, content: &str, path: &str) -> Vec<PiiMatch>;
}

pub trait LintRule: Send + Sync {
    fn name(&self) -> &str;
    fn check(&self, ctx: &VaultContext) -> Vec<LintIssue>;
}

pub trait EventRenderer: Send + Sync {
    fn render(&mut self, event: &StreamEvent);
    fn flush(&mut self) {}  // default no-op
}

pub trait LogSink: Send + Sync {
    fn write_run(&mut self, entry: &RunLog) -> Result<(), LogError>;
}
```

`LlmProvider` 已是這形狀（`Box<dyn LlmProvider>`），延續即可。

**為什麼**：runtime 切換、heterogeneous 註冊表（一個 vec<Box<dyn LintRule>>）、mock 測試都仰賴 dyn trait

**Alternatives considered**：

- **泛型 / monomorphization**：`fn run_lint<R: LintRule>(rule: R)` — 編譯期 dispatch、零 runtime cost。否決：失去 runtime 切換、無法做 vec<Box<dyn>> 統一處理 N 條 rule

## Risks / Trade-offs

- **Risk: refactor 大、容易踩到行為微妙差異（特別 lint output / render byte-equal）**
  → **Mitigation**：(a) 保留 `tests/fixtures/uv-vault-snapshot/check-output.txt` 作 byte-equal gate、(b) 每個 R 階段做完先跑 `cargo test --workspace` 全綠 + uv vault smoke test、(c) `EventRenderer::TerminalRenderer` impl 是把現有 `render_event` 函式整段搬進去、不重寫邏輯

- **Risk: 編譯時間因 cargo features 矩陣變多而退化**
  → **Mitigation**：(a) default features 為空（lean）、(b) CI 只跑 `--features all` 全集合 + default 兩個 case、不跑 N×M 矩陣、(c) 每個 feature gate 只 wrap 必要的 use / impl block，避免大區段 conditional 編譯

- **Risk: config.yaml schema 將來變動的 backward compat 問題**
  → **Mitigation**：(a) 用 serde 的 `#[serde(default)]` 全欄位、未指定走 default、(b) unknown field warn 但不 fail（forward-compat、user 升級 codebus 不會打爆舊 config）、(c) 加 schema version 欄位（`schema: 1`）保留未來 breaking change 的逃生口

- **Risk: 5 個 trait 一次落地、PR 太大難 review**
  → **Mitigation**：分 R1-R6 六個 commit checkpoint（見 Migration Plan），每個 R 完成都跑全 test、可獨立合理 commit

- **Trade-off: 接受手動 factory match 的 1 行 boilerplate 換 readability**
  → 數字上：每加一個 provider = enum 變一行 + match 變一行 = 2 行 boilerplate；換得「打開 factory.rs 一眼看到所有 provider」

- **Trade-off: 接受 Cargo features 帶來的 conditional compilation 複雜度**
  → 為了不讓「我不用 OpenAI 為何要編 OpenAI SDK」的合理 user 抱怨；複雜度集中在 factory.rs 一處、其他 impl 檔本身用整檔 `#[cfg(feature = "...")]` wrap 即可

## Migration Plan

每個 R 階段獨立 commit、跑全 test、再進下一階段：

### R1 — LLM domain：factory + providers/ subfolder

- 加 `codebus-core/src/llm/factory.rs`：`ProviderKind` enum、`ProviderConfig`、`build_provider`
- `git mv codebus-core/src/llm/claude_cli.rs codebus-core/src/llm/providers/claude_cli.rs`
- 改 `codebus-core/src/llm/mod.rs`：新 layout
- 改 `codebus-cli/src/main.rs`：兩處 `ClaudeCliProvider::new()` 改 call factory
- 跑 `cargo test --workspace` 全綠

### R2 — PII domain：trait + factory + Null + RegexBasic

- 加 `codebus-core/src/pii/{mod.rs,provider.rs,factory.rs}`
- 加 `codebus-core/src/pii/scanners/{null_scanner.rs,regex_basic.rs}`
- 加 unit tests（regex_basic 測幾個 known pattern：API key、email、IP）
- **不**接到 raw_sync — 行為仍與 0.2.0 一致
- 跑 `cargo test --workspace` 全綠

### R3 — Lint domain：trait + 拆 6 條 rule 為獨立檔

- 從 `codebus-core/src/wiki/lint.rs` 拆出 6 條 rule 邏輯到 `codebus-core/src/wiki/lint/rules/<rule>.rs`
- 加 `codebus-core/src/wiki/lint/{rule.rs,factory.rs}`
- `lint_wiki()` 改成 iterate `Vec<Box<dyn LintRule>>`、每條 rule call `check()`
- 既有 lint tests 全部留著、應全綠（行為 byte-equal）
- 跑 `cargo test --workspace` 全綠 + uv vault `--check` 跟 fixture byte-equal

### R4 — Render domain：EventRenderer trait + Terminal impl

- 加 `codebus-core/src/render/{mod.rs,event_renderer.rs,factory.rs}`
- 加 `codebus-core/src/render/renderers/terminal.rs`：把 `codebus-cli/src/ui.rs::render_event` 整段邏輯搬進來、改實作 `EventRenderer` trait
- `codebus-cli/src/main.rs`、`commands/{goal,query}.rs` 改用 `&mut dyn EventRenderer`
- ui.rs 的 print_lint_report 同樣改實作 trait（或保留 fn、由 Terminal renderer 內部呼叫）
- 跑 fixture byte-equal smoke

### R5 — Log domain：LogSink trait + Null + JsonLines

- 加 `codebus-core/src/log/{mod.rs,sink.rs,factory.rs}`
- 加 `codebus-core/src/log/sinks/{null_sink.rs,jsonl_sink.rs}`
- `commands/{goal,query}.rs` 接 `&mut dyn LogSink` 參數、預設 `NullSink`、行為與 0.2.0 一致
- 跑 `cargo test --workspace` 全綠

### R6 — Config domain：unified config.yaml

- 加 `codebus-core/src/config/{mod.rs,loader.rs,schema.rs}`
- `codebus-cli/src/main.rs` 改：startup 時 load `~/.codebus/config.yaml`、merge 進 RenderOptions / 各 plugin factory 的 ProviderConfig
- emoji 優先序恢復 5 級（CLI flag > `--no-emoji` > `NO_EMOJI` env > config.yaml > auto）
- 加 spec scenario 到 terminal-output capability
- `tests/fixtures/uv-vault-snapshot/check-output.txt` 仍 byte-equal（無 config.yaml 走 auto，行為與現在一致）
- 跑 `cargo test --workspace` 全綠 + 手動測 config.yaml 寫 `emoji: off` 後跑 codebus 確認沒 emoji

### Rollback 策略

- R1-R6 每個都是獨立 commit、若某階段測試 fail 就 `git reset --hard HEAD~1` 回前一階段
- 整個 plugin-architecture-refactor change 失敗就回到 `267b3c5`（rust-rewrite 完成 commit）
- 已 archive 的 rust-rewrite spec 不動

### Cool-down

- R1-R6 全完成後跑 buddy-gacha smoke test：`init` / `goal` / `check` 三條路徑與 0.2.0 byte-equal
- 對 uv fixture vault 跑 `--check` 與 `tests/fixtures/uv-vault-snapshot/check-output.txt` byte-equal
- 真正執行下一個 next stop（#1 PII filter）前先確認 plugin 架構穩定

## Open Questions

- **`page_size` rule 的 `page_size_overrides` 怎麼解析最直覺？** path glob（`wiki/synthesis/*` → 該資料夾全部）vs path prefix（`wiki/synthesis` → 該資料夾全部）vs file-by-file。傾向 path prefix 簡單、Phase A 重寫 lint 時再 finalize
- **EventRenderer 的 `flush()` 該何時 call？** goal / query 結束時主動 flush、還是 Drop 自動 flush？兩者都不衝突、Drop 較 robust 但 Tauri webview 可能想要明確 flush 信號。R4 開做時再看
- **Config.yaml 的 schema versioning 該 day-1 加還是延後？** 加（`schema: 1`）較負責任、但目前 schema 還在迭代、太早加 version 反而變成 noise。傾向 day-1 不加、待第一次 breaking change 時引入 + lazy migration
- **`Null` scanner / `Null` sink 命名**：`Null` 還是 `Disabled` 還是 `NoOp`？傾向 `Null`（最短）、但 `Disabled` 對 user-facing config（`scanner: null` 看起來像 YAML null literal，可能誤解）。yaml 端用 `disabled` 字串、Rust struct 名用 `NullScanner` — 這個 split 待 R2 finalize
