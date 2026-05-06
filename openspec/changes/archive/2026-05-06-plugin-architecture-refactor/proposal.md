## Why

未來 roadmap（PII filter、Multi-LLM provider、token tracking、lint feedback loop）涉及 **5 個延伸點** — 每個都會增刪實作（多家 LLM、多種 PII scanner、多條 lint rule、多種輸出格式、多種 log sink）。現況只有 `LlmProvider` trait，且選擇邏輯硬寫在 `codebus-cli/src/main.rs`：每加一個 provider 就要編輯 main.rs、加 if/else 分支。這違反了「**加 plugin = 加一個檔 + 註冊一行**」的擴展性目標。

PII filter 預計做 multi-provider（regex / presidio / cloud），若直接用硬寫方式落地，做完立刻要重構成 plugin pattern — 雙倍工。**先把骨架立好、後續所有 next stops 工程量直接砍 30-50%**。

額外、TS 0.1.0 的 `~/.codebus/config.yaml` user-level 預設（emoji 等）在 Rust port 期間被砍掉、是 regression — 此重構順便補回。

## What Changes

採 **trait + 顯式 factory match** 的 plugin pattern，5 個 domain 各自獨立：

- **`LlmProvider`**（已有 trait）→ 抽 `factory.rs`、現有 `claude_cli` 搬進 `providers/` 子資料夾、main.rs 改 call factory
- **`PiiScanner`**（NEW）→ trait + factory + 預設 `Null` scanner（不掃）+ `RegexBasic` impl 備用，default 行為不變
- **`LintRule`**（NEW）→ trait + factory + 把現有 6+ 條 rule 各自拆檔，default 行為不變（仍跑全部 rule）
- **`EventRenderer`**（NEW）→ trait + 抽出現有 terminal renderer 為 `TerminalRenderer` impl，default 行為不變
- **`LogSink`**（NEW）→ trait + 預設 `Null` sink + `JsonLines` impl 備用（給未來 token tracking 用），default 行為不變

新增 **統一 config.yaml**（`~/.codebus/config.yaml`）— 5 個 plugin domain + emoji 共用一份；CLI flag 與環境變數仍可 override。

決策摘要（design.md 詳述）：

- Register：手動 factory match（`match cfg.kind { ... }`），不用 inventory / linkme 等 magic 自動註冊
- Cargo features：預設帶常用 + 輕量 impl（regex / json sink / terminal）；僅 truly heavy deps 切 feature（AWS Comprehend SDK、custom ML）
- Trait 同步性：`LlmProvider` 維持 async（網路 I/O 必要），其他四個（`PiiScanner` / `LintRule` / `EventRenderer` / `LogSink`）都是 sync
- Crate 結構：5 個 plugin domain 全進 `codebus-core` 單一 crate（不拆 sub-crate）
- 預設行為中性：除了 `~/.codebus/config.yaml` 重新支援（restore TS regression），其餘 user-visible 行為與 0.2.0 完全一致

## Non-Goals (optional)

- **不加任何新 LLM provider impl**：本 change 只重構 LLM 層架構，Anthropic API direct / OpenAI / Ollama 是後續 #2 的 scope
- **不啟用 PII 掃描功能**：本 change ship `PiiScanner` trait 與兩個 impl（`Null` 預設、`RegexBasic` 備用），但 `raw_sync` 不接 scanner、行為與 0.2.0 一致；實際啟用是 #1 PII filter 的 scope
- **不改 CLI 行為**：`init` / `goal` / `query` / `check` 4 個指令的 args / stdout / exit code 不動
- **不改 vault 格式**：`.codebus/wiki/**` 結構保留
- **不啟用 LogSink 寫檔**：`LogSink` trait 與 `JsonLines` impl 進來，但未接到 goal command（接是 #4 token tracking 的 scope）
- **不導入第三方 plugin / 動態載入**：本 change 是 compile-time plugin（factory match），不支援 .so / dylib runtime loading
- **不拆 sub-crate**：plugin domain 全在 `codebus-core` 內；分 crate 是未來真有 codebus-app 想單引特定 domain 才考慮
- **不導入 inventory / linkme**：手動 factory match 換 readability + 0 新 dep，自動註冊 magic 留給 third-party plugin 出現時再考慮
- **不重設計 lint 規則本身**：6+ 條既有 rule 一條不刪一條不加，僅做檔案結構重組
- **不對 EventRenderer 加 Tauri impl**：`TerminalRenderer` 是 day-1 唯一 impl；`TauriEmitter` 是 Phase E（codebus-app）的 scope
- **不改 config 檔位置**：沿用 `~/.codebus/config.yaml`（TS 0.1.0 既定路徑），不引入 per-project `.codebus/config.yaml` override（可未來再加）

## Alternatives Considered (optional)

- **`inventory` / `linkme` 自動註冊**：每個 impl 加 `submit!` 巨集自動進註冊表，factory 不用 match。被否決：codebus 的 plugin 數量可預期 ≤ 5-10 個 / domain，linker 編譯期都看得到、不需要 runtime discovery；多一個 dep + reader 看不到有哪些選項要全文搜，違反 "boring is good"
- **拆 sub-crate**（`codebus-llm-providers` / `codebus-pii` / ...）：clean separation 的 SOTA。被否決：現階段 codebase 太小（總 ~3K LOC）、拆 crate 反而拖慢 incremental build；workspace 已 3 crate（core/cli/app）夠清楚；真要拆等到 codebus-app 想單獨依賴某 plugin domain 時再做
- **全 async trait**：所有 plugin 都用 `async_trait`、避免「為何 LLM 是 async 別的不是」的不對稱。被否決：除了 LLM、其他 plugin 都是 CPU/local-IO bound，async 心智負擔（lifetime + Send + Pin<Box<dyn Future>>）大於收益；HTTP-based PII scanner 可以在 impl 內 spawn_blocking 包起來
- **靠 cargo features 切換實作（不用 trait object）**：例如 `--features llm-anthropic` 編出來的 binary 內部 `LlmProvider` 是 `AnthropicProvider` 具體型別、無 dynamic dispatch。被否決：失去 runtime 切換能力（user 不能 `config.yaml` 改 provider 而不重編）、無法支援 mock provider 做整合測試
- **每個 domain 一份獨立 config.yaml**（`~/.codebus/llm.yaml` / `pii.yaml` / ...）：domain 隔離。被否決：`Cargo.toml` 是 monorepo config 的 SOTA、所有 sub-feature 一檔；多檔 clutter `~/.codebus/`；user 心智「打開一個檔調整 codebus」更直覺

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `terminal-output`：擴展「Load global config tolerantly」requirement 以涵蓋新增的 plugin section（`llm`、`pii`、`lint`、`render`、`log`）— 既有 phase-2 forward-compat 條款本就要求 unknown field 靜默忽略，現在這些 section 從 unknown 升級為 known，scenarios 補上對應 plugin section 的解析行為（仍維持 tolerant：未認得的子欄位忽略、認得但值非法 warn）

## Impact

- Affected specs:
  - Modified: openspec/specs/terminal-output/spec.md（MODIFY「Load global config tolerantly」requirement，新增 5 個 plugin section 的解析 scenarios）
- Affected code:
  - New: codebus-core/src/llm/factory.rs
  - New: codebus-core/src/llm/providers/mod.rs
  - New: codebus-core/src/pii/mod.rs
  - New: codebus-core/src/pii/provider.rs
  - New: codebus-core/src/pii/factory.rs
  - New: codebus-core/src/pii/scanners/mod.rs
  - New: codebus-core/src/pii/scanners/null_scanner.rs
  - New: codebus-core/src/pii/scanners/regex_basic.rs
  - New: codebus-core/src/wiki/lint/mod.rs（升級成資料夾、原 lint.rs 拆解進來）
  - New: codebus-core/src/wiki/lint/rule.rs
  - New: codebus-core/src/wiki/lint/factory.rs
  - New: codebus-core/src/wiki/lint/rules/mod.rs
  - New: codebus-core/src/wiki/lint/rules/page_size.rs
  - New: codebus-core/src/wiki/lint/rules/unexpected_file.rs
  - New: codebus-core/src/wiki/lint/rules/duplicate_slug.rs
  - New: codebus-core/src/wiki/lint/rules/missing_nav.rs
  - New: codebus-core/src/wiki/lint/rules/root_page.rs
  - New: codebus-core/src/wiki/lint/rules/broken_wikilink.rs
  - New: codebus-core/src/wiki/lint/rules/frontmatter_integrity.rs
  - New: codebus-core/src/render/mod.rs
  - New: codebus-core/src/render/event_renderer.rs
  - New: codebus-core/src/render/factory.rs
  - New: codebus-core/src/render/renderers/mod.rs
  - New: codebus-core/src/render/renderers/terminal.rs
  - New: codebus-core/src/log/mod.rs
  - New: codebus-core/src/log/sink.rs
  - New: codebus-core/src/log/factory.rs
  - New: codebus-core/src/log/sinks/mod.rs
  - New: codebus-core/src/log/sinks/null_sink.rs
  - New: codebus-core/src/log/sinks/jsonl_sink.rs
  - New: codebus-core/src/config/mod.rs
  - New: codebus-core/src/config/loader.rs
  - New: codebus-core/src/config/schema.rs
  - Modified: codebus-core/Cargo.toml（新增 cargo features：pii-presidio, pii-aws, llm-openai, llm-anthropic-api, log-otel；default lean）
  - Modified: codebus-core/src/lib.rs（新增 pub mod pii / render / log / config）
  - Modified: codebus-core/src/llm/mod.rs（從 re-export 單一 ClaudeCliProvider 改為 re-export factory + providers module）
  - Modified: codebus-core/src/llm/claude_cli.rs（搬進 providers/ 子資料夾，用 git mv 保持 history）
  - Modified: codebus-core/src/wiki/mod.rs（lint module 從單檔升資料夾、re-export 不變）
  - Removed: codebus-core/src/wiki/lint.rs（拆解到 lint/ 資料夾）
  - Modified: codebus-cli/Cargo.toml（serde_yaml 加 dep、用於讀 config.yaml）
  - Modified: codebus-cli/src/main.rs（移除硬寫 ClaudeCliProvider::new()、改 call factory；加 config.yaml load 並 merge 進 RenderOptions / 各 plugin 的 config）
  - Modified: codebus-cli/src/ui.rs（render_event / print_lint_report 改實作 EventRenderer trait）
  - Modified: codebus-cli/src/commands/goal.rs（接 LogSink、為未來 #4 token tracking 鋪路；本 change 預設 Null sink，行為不變）
  - Modified: codebus-cli/src/commands/query.rs（同上）
  - Modified: codebus-cli/src/commands/check.rs（lint output 改透過 EventRenderer / Terminal impl）
