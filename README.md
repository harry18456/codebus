# 🚌 codebus

> **來囉來囉~** 跟 AI 上車探索陌生 codebase，每站發明信片，集結成你自己的程式碼旅遊書。
>
> *Build an LLM-maintained, Obsidian-compatible wiki for any codebase.*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
![Status](https://img.shields.io/badge/status-v3_shipped-blue)
![Rust](https://img.shields.io/badge/rust-1.85+-blue)

---

## 為什麼有這台車

身為 RD，這幾個情境是不是很熟：

- 接手陌生 repo，從零讀到有 mental model 要花一兩天
- 讀完一輪沒寫筆記，下次回來又得重讀
- 同事問「這 repo 的 X 怎麼運作」，又得 re-discover 一次
- 想開新 feature 前要先弄懂相關模組，但 grep 跟 IDE 跳定義有極限

codebus 把「**讀懂的中間態**」強制持久化成有結構的 wiki — 下次（或下個同事）來，看 wiki 就好。

不是自動 doc 產生器，是 **「用 AI 共寫的程式碼旅遊書」**。

---

## 30 秒體驗

```bash
# 先確保 Claude Code CLI 已安裝且 OAuth：https://claude.ai/code
cargo install --path codebus-cli

cd ~/some/unfamiliar/repo

codebus init                                     # 🚏 上車：init vault
codebus goal "搞懂 auth 模組怎麼運作"             # 🚌 第一站
codebus goal "搞懂 checkout flow"                # 🚌 第二站
codebus query "auth 跟 checkout 怎麼互動？"      # 💬 翻明信片問司機
codebus lint                                     # 🔍 車況檢查（純 lint）
codebus fix                                      # 🛠️ 司機自動修

# 開 Obsidian 看 .codebus/wiki/
```

---

## 安裝

```bash
cargo install --path codebus-cli
```

`cargo install` 會把 `codebus` 放到你的 `PATH`（`~/.cargo/bin/`）。**`fix` verb 需要 `codebus` 在 PATH 上才能完整運作** — 司機 spawn 出的 child process 會用 Bash tool 跑 `codebus lint --json` 取得即時 feedback；找不到 binary 時，agent 仍會結束，CLI 端的 final lint 還是會跑（end-state 正確），但 in-session iteration 品質會明顯下降。

如果只想試跑 `init` / `goal` / `query` / `lint`，`cargo build --release` 出 `target/release/codebus.exe`、用絕對路徑執行也行；只有 `fix` 強烈建議走 `cargo install`。

---

## 公車怎麼開（5 條路線）

| 指令 | 在做什麼 |
|---|---|
| `codebus init` | 🚏 **進站** — 在 repo 建 `.codebus/` vault、寫 schema、init nested git、註冊 Obsidian vault |
| `codebus goal "..."` | 🚌 **載你去某站** — AI 探索 source、寫 / 改明信片、auto-commit |
| `codebus query "..."` | 💬 **問司機** — AI 看現有 wiki 回答你的問題（read-only，沒 auto-commit） |
| `codebus lint` | 🔍 **車況檢查** — 純 lint，不叫 LLM |
| `codebus fix` | 🛠️ **司機修車** — spawn agent 跑 lint→edit→re-lint loop，收尾 auto-commit |

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

設計遵循 [Karpathy 的 "LLM Wiki" pattern](https://gist.github.com/karpathy/3ef7345f9192fe96d11a25fb1c40b35c) — 5 typed folders + cross-page wikilinks + goal-driven incremental growth，不是一次性 RAG。

---

## 為什麼 markdown + Obsidian

- **工具死了 wiki 還活著** — 純 markdown，沒有 proprietary state
- **Obsidian 開即用** — backlinks / graph view / Dataview 全部免費送
- **手動編輯也行** — AI 寫錯直接改，下次 lint 不會抗議
- **Git 友善** — 每個 goal / fix 收尾 nested-git auto-commit，wiki 演化全留歷史

---

## ⚠️ Security: goal / query / fix 的字串會直接餵給 LLM

`goal "..."` / `query "..."` 那段文字會變成 system prompt 的一部份送進 Claude。**不要把不可信的內容（隨機 GitHub issue、外部網頁、外部 Slack 訊息）直接貼進來** — 這是 prompt injection vector。

Sandbox 機制（best-effort）：

- spawn `cwd = .codebus/` 系統層隔離 agent 跟你的 source repo（cwd 外的 Write 直接被擋）
- **Triple-flag toolset gate**：`--tools <whitelist>` (hard gate) + `--allowedTools <same>` (auto-approval) + `--permission-mode acceptEdits`（v2 iter-9 spike-verified；2026-05-09 在 v3 重 spike 確認仍有效）
- `goal` 給 Read/Glob/Grep/Write/Edit；`query` 純 Read/Glob/Grep（read-only）；`fix` 多 `Bash(codebus lint *)` 一條 fine-grained whitelist
- nested git auto-commit、隨時可 `git -C .codebus reset --hard` 還原
- `lint` 是 100% read-only，沒有任何 LLM 呼叫

但 `.codebus/` **內部** agent 仍能寫 `CLAUDE.md` / `index.md` / `log.md` 等。後續會用 `--settings permissions.deny` 補強。

詳細 sandbox 設計見 [`CLAUDE.md`](CLAUDE.md) 與 `openspec/specs/cli/spec.md`。

---

## 路線預告（Roadmap）

### 🚌 Currently boarding（已在跑）

```
[x] v3.0.0 — Rust rewrite 完成，10 主線 change ship 完整
[x] CLI 5 verbs：init / goal / query / lint / fix
[x] Sandbox triple-flag：--tools / --allowedTools / --permission-mode acceptEdits
[x] Karpathy 5-folder taxonomy + Obsidian-compatible wikilinks (OSC 8)
[x] Source enrichment + drift detection（manifest signal 比對）
[x] PII filter (regex_basic / null) + severity-dispatched on_hit + Critical floor
[x] Lint 7 rules + JSON output + auto-fix loop (single-shot trust-agent)
[x] Auto-init nested git + auto_commit per goal/fix
[x] Per-verb claude_code config (--model / --effort) + global starter
[x] Banner system + emoji/color/OSC 8 hyperlinks（5-level priority）
[x] Stream rendering live + RunLog jsonl 持久化（token usage / 時戳 / lint counts）
```

### 🛣️ Next stops（CLI 路線繼續走）

優先序背後的策略：CLI 主線已完整、stream + RunLog 也讓 cost / behavior 可觀測，下一段重心是**鬆綁 provider 綁定**——讓 user 不只能跑 Claude CLI，也能對接 Azure / Bedrock 部署、其他 agentic AI provider、其他 PII 引擎。Provider 矩陣展開後，搭配 first-run wizard 才能讓「裝完即用」變成現實。

1. 🔌 **Azure / non-Anthropic endpoint 接入** — 透過 Claude CLI 的 `ANTHROPIC_BASE_URL` / `CLAUDE_CODE_USE_BEDROCK` / `CLAUDE_CODE_USE_VERTEX` 走 proxy；codebus 目前 spawn 已繼承父 env，剩驗證 + cookbook 文件
2. 🤖 **Multi-agentic AI provider** — 第二個 provider impl（codex / gemini / 其他）真的要進來時，先 spike 對方 CLI 的 slash command + toolset gate 機制，驗完才設計 trait surface（在那之前 provider 模組保持 single impl）
3. 🛡️ **Multi-PII provider** — 補強現有 regex_basic：`presidio`（Microsoft Presidio HTTP）、`aws` Comprehend Detect-PII、自訂 ML scanner
4. 🔎 **Embedded search** — 對 wiki/ pages 跑 embedding/vector index，提供 semantic search（補強 `query` 或開新 `codebus search` verb，不每次都 spawn Claude）
5. 🧭 **First-run setup wizard** — 第一次跑 cli 偵測 `~/.codebus/config.yaml` 不存在 → 互動引導選 AI provider / PII provider / 其他細節，寫進 config（**依賴 1+2+3+4，最晚做** — wizard 要有實際選擇可選才有意義）

### 🌅 Final destination — codebus-app（GUI / Tauri tutorial app）

CLI 是基礎建設。**真正的產品形態**是 desktop tutorial app（workspace 裡 `codebus-app/` crate 已預留位置）：

- 📑 markdown → 互動式「站點」（投影片模式）
- ✅ 嵌入式 `<Checkpoint>` / `<Quiz>` 練習組件
- 💬 Cmd+K Q&A drawer（query 命令的 GUI 化身）
- 🔄 投影片 vs 文件 兩種閱讀模式

從**「給 Obsidian 看的 wiki」**→**「能帶你重走一次旅程的 onboarding app」**是這台車的最終目的地。

---

## Architecture（給想 hack 的人）

Cargo workspace + 3 crates：

```
codebus-core/    純 Rust lib：lint / frontmatter / schema / stream parser /
                 PII / log sink / agent spawn
codebus-cli/     clap binary：5 verbs (init / goal / query / lint / fix)
codebus-app/     Tauri shell — Final destination tutorial app 的預留位置
```

Agent system prompt 是 `codebus-core/src/schema/CLAUDE.md`，**那才是真正定義產品行為的東西** — 比 Rust code 還重要。Lock-in tests 守住關鍵 phrase 不會被誤刪。

完整 spec 與 change history 見 [`openspec/`](openspec/)；roadmap 詳細版見 [`docs/v3-roadmap.md`](docs/v3-roadmap.md)。

---

## 演進

codebus 走到 v3 是踩過兩次坑後的第三次重寫。三個版本都還在 repo 裡可以 verify，不是抹掉重來：

### v1 — TypeScript prototype（[`legacy/ts-src/`](legacy/ts-src/)）

最早從前端切入做 Tauri app 殼，後端跟著 sidecar 拼上去。結果**前端走太快、後端跟不上** — 餅畫太大沒先驗證 backend 可行性，反覆修改、sidecar 讓整體行為複雜化，最後放棄。但 iter-8 / iter-9 累積的 sandbox argv 教訓、stream-parser schema、enrichSourceMetadata invariant 全留下來，是 v2 的起點。

### v2 — Rust rewrite，CLI-first（[`legacy/v2-rust/`](legacy/v2-rust/) + `v2-archive` branch）

撇除前端、純 CLI 做完 phase 1：5-folder taxonomy、`--tools` whitelist sandbox、auto-lint、source enrichment、PII filter、lint feedback loop、token tracking、Obsidian-clickable wikilinks。phase 1 收尾後 2026-05-08 有一場 strategy 討論（[skill-vs-binary-pivot](legacy/v2-rust/docs/strategy/2026-05-08-skill-vs-binary-pivot.md)），浮出 4 條路；第一次嘗試 path D（pivot 成 Claude Code skill）失敗、revert，歸納出 4 條 anti-patterns 後重啟成 v3。

### v3 — 現在這版（`codebus-core` + `codebus-cli` + `codebus-app`）

為了 **iteration 速度** 跟 **Final destination Tauri app 整合**，重新切成 3-crate workspace：核心邏輯放 `codebus-core`，CLI 跟 GUI 是兩個 thin shell。這切法讓 v3 從一開始就為「CLI 跟 app 並存」設計、沒有 v1 那種前端 → sidecar → backend 的串行依賴。

紀律從 v2 的失敗 first-attempt 萃出：no speculative single-impl trait、no schema double-ship、carry-over 前先 grep v2、`/spectra-apply` 不亂 checkpoint。10 主線一條一條序列做完、ship v3.0.0。

---

## Development

```bash
cargo check --workspace        # 快速 type-check
cargo test --workspace         # 377 tests
cargo build --release          # 出 target/release/codebus
cargo clippy --workspace       # lint
cargo fmt --all                # format
```

需要：

- Rust 1.85+ (edition 2024)
- `claude` CLI 已安裝且 OAuth ([Claude Code](https://claude.ai/code))
- Windows / macOS / Linux 都能 build；live-tested 主要在 Windows MSVC

---

## Why "codebus"?

致敬 **「上車舞」** meme — 不用怕、跟著走、總會到站的 vibe。

讀陌生 codebase 不該是讓人焦慮的事。**上車就對了**。 🚌

---

## Status

v3.0.0 主線 10 條 change 全 ship、377 tests 全綠、live e2e 對 [`uv`](https://github.com/astral-sh/uv) 跑通並寫進 `docs/v3-uv-verification-2026-05-10.md`。

**找早期乘客** — 試用、報 bug、提建議都歡迎。issue 直接開、PR 也歡迎。

`codebus-app`（Final destination Tauri tutorial app）尚在規劃。

---

## License

[MIT](LICENSE)
