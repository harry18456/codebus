# 🚌 CodeBus

> **來囉來囉 ~** 跟 AI 上車探索陌生 codebase，每站發明信片，集結成你自己的程式碼旅遊書。
>
> *Build an LLM-maintained, Obsidian-compatible wiki for any codebase.*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
![Status](https://img.shields.io/badge/status-0.2.0_early-orange)
![Rust](https://img.shields.io/badge/rust-1.85+-blue)

---

## 為什麼有這台車

身為 RD，這幾個情境是不是很熟：

- 接手陌生 repo，從零讀到有 mental model 要花一兩天
- 讀完一輪沒寫筆記，下次回來又得重讀
- 同事問「這 repo 的 X 怎麼運作」，又得 re-discover 一次
- 想開新 feature 前要先弄懂相關模組，但 grep 跟 IDE 跳定義有極限

CodeBus 把「**讀懂的中間態**」強制持久化成有結構的 wiki — 下次（或下個同事）來，看 wiki 就好。

不是自動 doc 產生器，是 **「用 AI 共寫的程式碼旅遊書」**。

---

## 30 秒體驗

```bash
# 先確保 Claude Code CLI 已安裝且 OAuth：https://claude.ai/code
cargo build --release --workspace
alias codebus=$(pwd)/target/release/codebus

cd ~/some/unfamiliar/repo

codebus                                          # 🚏 上車：init vault
codebus --goal "搞懂 auth 模組怎麼運作"           # 🚌 第一站
codebus --goal "搞懂 checkout flow"              # 🚌 第二站
codebus --query "auth 跟 checkout 怎麼互動？"     # 💬 翻明信片問司機
codebus --check                                  # 🔍 車況檢查（純 lint）

# 開 Obsidian 看 .codebus/wiki/
```

---

## 安裝（lean vs fat）

CodeBus 採 plugin 架構 + cargo features — 預設裝最精簡的版本，需要時再帶 feature flags 把進階 provider 編進去。

```bash
# Lean default：claude_cli LLM provider + null/regex_basic PII scanner +
# terminal renderer + null/jsonl log sink。0 額外重型 dep，cargo 編最快。
cargo install codebus

# Fat：直接帶上未來會 ship 的所有 provider 變體。
cargo install codebus --features all-llm,pii-presidio

# 全包（含 AWS Comprehend、OTel — 會多 50MB+ binary footprint）
cargo install codebus --features all
```

可用的 feature flags：

| Tier | Flag | 帶來什麼（impl 後續 PR 補上） |
|---|---|---|
| 2 | `llm-anthropic-api` | 直接走 Anthropic HTTP API（不靠 claude CLI） |
| 2 | `llm-openai` | OpenAI-compat API provider |
| 2 | `pii-presidio` | Microsoft Presidio HTTP scanner |
| 3 | `pii-aws` | AWS Comprehend Detect-PII（heavy SDK） |
| 3 | `log-otel` | OpenTelemetry log export |
| — | `all-llm` | = `llm-anthropic-api` + `llm-openai` |
| — | `all-pii` | = `pii-presidio` + `pii-aws` |
| — | `all` | = `all-llm` + `all-pii` + `log-otel` |

不用某個 provider，不裝 feature 也照樣能跑 — 只是該選項在 `~/.codebus/config.yaml` 裡會 `FeatureNotCompiled` 報錯，提示重 compile。

---

## 公車怎麼開（4 條路線）

| 指令 | 在做什麼 |
|---|---|
| `codebus` (no args) | 🚏 **進站** — 在 repo 建 `.codebus/` vault、寫 schema、init nested git |
| `codebus --goal "..."` | 🚌 **載你去某站** — AI 探索 source、寫 / 改明信片、auto-commit |
| `codebus --query "..."` | 💬 **問司機** — AI 看現有 wiki 回答你的問題（read-only） |
| `codebus --check` | 🔍 **車況檢查** — 純 lint，不叫 LLM |

每站的明信片是一個 markdown file，分 5 種 type folder（Karpathy 5-bucket）：

```
.codebus/wiki/
├─ concepts/      抽象概念、設計原則、mental models
├─ entities/      data structures、schemas、records
├─ modules/       code organization units、libraries、services
├─ processes/     workflows、state machines、有順序的演算法
├─ synthesis/     跨頁面整合的綜述
├─ index.md       路線圖（page catalog with summaries）
└─ log.md         旅行日誌（chronological journal of goals）
```

設計遵循 [Karpathy's "LLM Wiki" pattern](https://gist.github.com/karpathy/3ef7345f9192fe96d11a25fb1c40b35c) — 5 typed folders + cross-page wikilinks + goal-driven incremental growth，不是一次性 RAG。

---

## 為什麼 markdown + Obsidian？

- **工具死了 wiki 還活著** — 純 markdown，沒有 proprietary state
- **Obsidian 開即用** — backlinks / graph view / Dataview 全部免費送
- **手動編輯也行** — AI 寫錯直接改，下次 lint 不會抗議
- **Git 友善** — 每個 goal 結束 nested-git auto-commit、wiki 演化看得到

---

## 路線預告（Roadmap）

### 🚌 Currently boarding（已在跑）

```
[x] 0.2.0 — Rust port complete (from TypeScript prototype)
[x] CLI parity — init / goal / query / check
[x] Karpathy 5-folder taxonomy + Obsidian-compatible wikilinks
[x] Sandbox: --tools whitelist (iter-9 hard-won lesson — 詳見 CLAUDE.md)
[x] Auto-lint after every goal + standalone --check command
[x] Source enrichment + stale detection（追蹤 wiki page 對應的 raw 檔有沒有變）
[x] PII filter (regex_basic / null) wired into raw_sync — 3 OnHit modes (warn / skip / mask) + patterns_extra；防 hardcoded secrets / API keys / 個資進入 LLM context
[x] Lint feedback loop — 司機自動修 broken wikilink / oversize page / frontmatter 錯誤等；goal flow 自動接 + 獨立 `codebus --fix` mode；`--no-fix` / `lint.auto_fix.enabled` 可關（多回合用 git diff 假記憶撐 trait stateless）
[x] Token usage & log tracking — `StreamEvent::Usage` 從 Claude CLI stream-json 抽 token；`RunLog` 累加跨 fix-loop iterations；jsonl sink 預設寫 `<repo>/.codebus/logs/runs-YYYY-MM-DD.jsonl`（UTC 輪替 + nested-git ignore）；Multi-LLM cost 對比的資料層已到位
[x] Obsidian-clickable wikilinks — CLI thought 流 `[[wikilink]]` 染色 + Ctrl+Click 直接跳 Obsidian 開對應頁；init 自動把 `.codebus/wiki/` 註冊為 Obsidian vault（跨 OS path 解析、SHA-256 stable id、idempotent reuse same-path entry、Obsidian 跑著時 skip + hint、`--no-obsidian-register` opt-out、終端不支援 OSC 8 時優雅退化只染色不點）
```

### 🛣️ Next stops（規劃中）

優先序背後的策略：lint feedback loop 已 ship 為第一個多回合 trait use case；token tracking 也已上車（cost 對比資料層備妥）。Multi-LLM provider 是接下來的軸心，因為 API 家族不內建 Read/Write 等工具，這一階段勢必引入 codebus 自己的 tool 抽象 —— 一旦 tool 抽象就位，custom tool 場景（query gap detection 等）才有合適的家。

1. 🔌 **Multi-LLM provider + tool abstraction** — Anthropic API direct / OpenAI / 本地 model；同時要把 Read/Glob/Grep/Write/Edit 從 Claude CLI 內建提到 codebus 自己的 tool runtime（API 家族不內建這套）；tool 抽象一旦就位也順帶打開 custom tool 場景的大門；解綁對 Claude CLI 的硬依賴
2. 🆘 **Query gap detection** — 「這站沒明信片」→ 提議升級成 goal 補完缺口；屬於 #1 tool 抽象後的第一個 custom tool 範例（`propose_goal` 跨 provider 一致實作）
3. 🧭 **Onboarding wizard (`codebus setup`)** — 全域偏好 wizard：偵測 `claude` CLI、選 LLM provider、設 PII 模式 + patterns_extra，寫 `~/.codebus/config.yaml`（含註解）；多 provider 真的存在後 wizard 才有意義
4. 🗂️ **Vault registry** — `~/.codebus/registry.json` 紀錄機台上每個 codebus vault 的路徑 + last_used + source_version；獨立於 `config.yaml`（preferences vs machine state 分開），為下一段 Tauri hub view 鋪資料層
5. 🛡️ **Heavy-dep PII scanners** — `presidio` / `aws` Comprehend Detect-PII / 自訂 ML；regex_basic 已上車，需要更精準匹配時才補
6. 💾 **Disk preflight** — raw-sync 前估算 + 警告剩餘容量，避免大型 monorepo 把 disk 撐爆
7. 📦 **Multi-platform binary release + CI** — cargo install / homebrew tap / GitHub Releases / GitHub Actions cross-platform test matrix

### 🌅 Final destination — Tauri tutorial app

CLI 是基礎建設。**真正的產品形態**是 desktop tutorial app：

- 📑 markdown → 互動式「站點」（投影片模式）
- ✅ 嵌入式 `<Checkpoint>` / `<Quiz>` 練習組件
- 💬 Cmd+K Q&A drawer（query 命令的 GUI 化身）
- 🔄 投影片 vs 文件 兩種閱讀模式

從**「給 Obsidian 看的 wiki」**→**「能帶你重走一次旅程的 onboarding app」**是這台車的最終目的地。Workspace 裡 `codebus-app/` crate 已預留位置。

---

## Why "CodeBus"?

致敬 **「上車舞」** meme — 不用怕、跟著走、總會到站的 vibe。

讀陌生 codebase 不該是讓人焦慮的事。**上車就對了**。 🚌

---

## ⚠️ Security: goal / query 是直接餵給 LLM 的

`--goal` / `--query` 後面那段文字會變成 system prompt 的一部份送進 Claude。**不要把不可信的內容（隨機 GitHub issue、外部網頁、外部 Slack 訊息）貼進來** — 這是 prompt injection vector。

Sandbox 機制（best-effort）：

- spawn `cwd = .codebus/` 系統層隔離 agent 跟你的 source repo（cwd 外的 Write 直接被擋）
- `--permission-mode acceptEdits` + `--tools Read,Glob,Grep[+Write,Edit]` 白名單（`Bash` / `WebFetch` / `WebSearch` 都不在工具表裡）
- nested git auto-commit、隨時可 `git -C .codebus reset --hard` 還原
- `--check` 是 100% read-only，沒有任何 LLM 呼叫

但 `.codebus/` **內部** agent 仍能寫 `CLAUDE.md` / `.git/` / `goals.jsonl`。Phase 2 會用 `--settings permissions.deny` 鎖死這些。

詳細 sandbox 設計與 iter-9 教訓見 [`CLAUDE.md`](CLAUDE.md)。

---

## Architecture（給想 hack 的人）

Cargo workspace + 3 crates：

```
codebus-core/    純 Rust lib：lint, frontmatter, schema, stream parser, LLM trait
codebus-cli/     clap binary：init / goal / query / check
codebus-app/     Tauri shell — Phase E tutorial app 的預留位置
```

agent system prompt 是 `codebus-core/src/schema/CLAUDE.md`，**那才是真正定義產品行為的東西** — 比 Rust code 還重要。Lock-in tests 在 `codebus-core/src/schema/mod.rs` 守住關鍵 phrase 不會被誤刪。

跨語言 conformance 由 `tests/fixtures/uv-vault-snapshot/` 守 — Rust 端 lint 對 uv vault 的輸出跟 TS 0.1.0 baseline byte-equal。

完整設計與 Phase A-D rewrite 紀錄見 [`openspec/changes/archive/2026-05-05-rust-rewrite/`](openspec/changes/archive/2026-05-05-rust-rewrite/)。

---

## Development

```bash
cargo check --workspace        # 快速 type-check
cargo test --workspace         # 136 tests
cargo llvm-cov --workspace     # coverage report (~94%)
cargo clippy --workspace       # lint
cargo fmt --all                # format
cargo watch -x test            # 改檔自動跑 test
```

需要：
- Rust 1.85+ (edition 2024)
- `claude` CLI 已安裝且 OAuth ([Claude Code](https://claude.ai/code))
- Windows / macOS / Linux 都能 build；live-tested 主要在 Windows MSVC

---

## Status — early experimental 🟡

CodeBus 起源於**參與公司內部競賽**而開的應用發想。0.2.0 是 Rust 重寫完成的版本，Phase 1 CLI 功能完整、136 個 test pass、live e2e 對 [`uv`](https://github.com/astral-sh/uv) 跟內部專案跑通；但還沒正式上 cargo / homebrew / GitHub Releases，binary distribution 屬下一步規劃。

**找早期乘客** — 試用、報 bug、提建議都歡迎。issue 直接開、PR 也歡迎。

Tauri tutorial app（最終目的地）正在設計中。

---

## License

MIT — see [LICENSE](LICENSE).
