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
```

### 🛣️ Next stops（規劃中）

按目前評估的優先序（會隨真實使用回饋調整）：

1. 🔐 **PII filter**（multi-provider：regex / presidio / 雲端 API / 自訂 ML）
   — 防 hardcoded secrets、API keys、個資進入 LLM context；v1-archive 曾有過 sanitizer，Phase 1 重寫期間沒 port 回來，是 security blocker
2. 🔌 **Multi-LLM provider** — Anthropic API direct / OpenAI / 本地 model；`LlmProvider` trait 已就緒，只缺各家 impl，可解綁對 Claude CLI 的硬依賴
3. ⚙️ **Restore `~/.codebus/config.yaml`** — emoji 等 user-level 預設值；**regression from TS 0.1.0**（5 級優先序砍到 3 級），補回來工程量小
4. 🪙 **Token usage & log tracking** — 紀錄每趟車花多少油、累積成本；在做後面幾項 feature 前先建立 telemetry
5. 🔧 **Lint feedback loop** — 司機自己檢查 wiki 寫得乾不乾淨、自動修 broken wikilink / oversize page 等
6. 🆘 **Query gap detection** — 「這站沒明信片」→ 提議升級成 goal 補完缺口
7. 💾 **Disk preflight** — raw-sync 前估算 + 警告剩餘容量，避免大型 monorepo 把 disk 撐爆
8. 📦 **Multi-platform binary release + CI** — cargo install / homebrew tap / GitHub Releases / GitHub Actions cross-platform test matrix

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
