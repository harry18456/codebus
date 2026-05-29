# 🚌 codebus

> **來囉來囉~** 跟 AI 上車探索陌生 codebase，每站發明信片，集結成你自己的程式碼旅遊書。
>
> *Build an LLM-maintained, Obsidian-compatible wiki for any codebase.*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
![Rust](https://img.shields.io/badge/rust-1.85+-blue)

---

## 為什麼有這台車

身為 RD，這幾個情境是不是很熟：

- 接手陌生 repo，從零讀到有 mental model 要花一兩天
- 讀完一輪沒寫筆記，下次回來又得重讀
- 同事問「這 repo 的 X 怎麼運作」，又得 re-discover 一次
- 想開新 feature 前要先弄懂相關模組，但 grep 跟 IDE 跳定義有極限

codebus 把「**讀懂的中間態**」強制持久化成有結構的 wiki — 下次（或下個同事）來，看 wiki 就好。

不是自動 doc 產生器，是 **「跟 AI 共寫的程式碼旅遊書」**。

---

## 30 秒體驗

```bash
# 先確保 Claude Code CLI 已安裝且 OAuth（預設 provider）：https://claude.ai/code
# 想用 Codex CLI / Azure OpenAI 改 provider 見下方 "Provider 選擇"
cargo install --path codebus-cli

cd ~/some/unfamiliar/repo

codebus init                                # 🚏 上車：建 vault
codebus goal "搞懂 auth 模組怎麼運作"        # 🚌 第一站，AI 開寫
codebus chat                                # 💬 跟司機多輪聊天
codebus quiz "auth flow"                    # 🎓 抽考你看懂沒
codebus lint                                # 🔍 車況檢查
codebus fix                                 # 🛠️ 司機自己修

# 開 Obsidian 看 .codebus/wiki/，明信片都在裡面
```

到站。

---

## 安裝

```bash
cargo install --path codebus-cli
```

`cargo install` 會把 `codebus` 丟到 `~/.cargo/bin/`。**`fix` 強烈建議走這條** — 司機修車時要呼叫 `codebus lint` 看看自己修得怎樣，沒在 PATH 上他會盲修。

只想試 `init` / `goal` / `query` / `lint`？`cargo build --release` 用絕對路徑跑也行。

---

## Provider 選擇

codebus 預設用 **Claude Code CLI** 開司機（30 秒體驗的設定就是這條）。也支援 **OpenAI Codex CLI**（含 Azure OpenAI 部署）當第二 provider：

- **Claude**（預設）：裝 [`claude` CLI](https://claude.ai/code) 並 OAuth。
- **Codex**：裝 [`codex` CLI](https://github.com/openai/codex)；Azure OpenAI 端點透過 `codebus config set-key` 把 API key 收進 keyring，再用 `codebus-app` 的 Settings → Endpoint 切到 codex / azure profile。

provider 切換、model / effort、Azure base_url / api-version 全在 app 的 Settings 介面設；CLI 共用同份 `~/.codebus/config.yaml`。

---

## 公車怎麼開

| 指令 | 在做什麼 |
|---|---|
| `codebus init` | 🚏 **進站** — 在 repo 建 `.codebus/`、註冊到 Obsidian |
| `codebus goal "..."` | 🚌 **載你去某站** — AI 探索、寫明信片、auto-commit |
| `codebus query "..."` | 💬 **問司機一句** — 看現有 wiki 回答（不改檔） |
| `codebus chat` | 💬 **跟司機聊** — 多輪 REPL，聊到滿意可以直接升級成 goal |
| `codebus quiz "..."` | 🎓 **司機抽考** — 給個主題，出選擇題自我驗證 |
| `codebus lint` | 🔍 **車況檢查** — 純規則，不叫 LLM |
| `codebus fix` | 🛠️ **司機修車** — 跑 lint → 改 → re-lint 直到綠燈 |
| `codebus config` | 🔑 **油卡管理** — 設 / 查 / 刪 Azure 端點 API key（keyring） |

每站的明信片是一個 markdown，分成 5 種類型（致敬 Karpathy 5-bucket）：

```
.codebus/wiki/
├─ concepts/      抽象概念、設計原則、mental models
├─ entities/      data structures、schemas、records
├─ modules/       code organization units、libraries、services
├─ processes/     workflows、state machines、有順序的演算法
├─ synthesis/     跨頁面整合的綜述
├─ index.md       路線圖（所有明信片的目錄）
└─ log.md         旅行日誌（每次上車的時間軸）
```

設計遵循 [Karpathy 的 "LLM Wiki" pattern](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) — typed folders + wikilinks + goal-driven 慢慢長大，不是一次性 RAG。

---

## 為什麼 markdown + Obsidian

- **工具死了 wiki 還活著** — 純 markdown，沒鎖在誰家
- **Obsidian 開即用** — backlinks / graph view / Dataview 全部免費送
- **手動編輯也行** — AI 寫錯直接改，下次 lint 不會抗議
- **Git 友善** — 每次 goal / fix 收尾 auto-commit，演化歷史全留

---

## ⚠️ 安全提醒：你輸入的字會餵給 LLM

`goal` / `query` / `chat` / `quiz` 裡你打的字會變成 system prompt 的一部份送進 Claude。

**別把不可信的內容（隨機 GitHub issue、外部網頁、Slack 訊息）整段貼進來** — 這是 prompt injection vector，會讓司機被乘客帶歪路。

codebus 已內建幾層防護（cwd 隔離、PII filter、nested git 可隨時還原，加上 provider-specific 命令/工具限制），但這些不是萬靈丹——而且**各 provider 隔離強度不同**：

- **claude** 走 `--tools` 白名單 + PreToolUse hook，讀寫都有 deterministic gate（含擋讀 `~/.ssh` 等敏感路徑）
- **codex** 走 `-s sandbox`（`read-only`／`workspace-write`）+ OS restricted token，但 Windows unelevated 實測（codex-cli 0.135.0）只是**部分**隔離：**寫**對正常 ACL 路徑有擋（workspace 外／家目錄回 `Access is denied`），但 Everyone-writable 目錄（如 `C:\Windows\Temp`）仍漏；**讀**照樣讀得到 workspace 外的檔與 `%USERPROFILE%` 內容（`~/.ssh`、`~/.aws` 等家目錄機密屬此類）；**網路**只擋外部 HTTPS/443，loopback 與外部 HTTP/80 仍外洩。→ codex 是**讀／網路 soft-partial、寫較硬**，敏感家目錄「讀」相關任務請用 claude 或自行承擔風險（細節見 [`docs/security.md`](docs/security.md) §5）

完整 threat model 跟每層防護怎麼運作：[`docs/security.md`](docs/security.md)。

---

## 還想看更多？

README 故意只放「目的 + 怎麼用」。其他往這走：

- 🛣️ **接下來要做啥** → [`docs/v3-roadmap.md`](docs/v3-roadmap.md)
- 🏗️ **架構長怎樣 / spec 在哪** → [`openspec/specs/`](openspec/)
- 🪦 **v1 / v2 怎麼死的** → [`legacy/README.md`](legacy/README.md)

---

## Development

```bash
cargo check --workspace        # 快速 type-check
cargo test --workspace         # 跑全部 test
cargo build --release          # 出 target/release/codebus
cargo clippy --workspace       # lint
cargo fmt --all                # format
```

需要：

- Rust 1.85+ (edition 2024)
- 至少裝一個 provider CLI：[Claude Code](https://claude.ai/code)（預設）或 [OpenAI Codex CLI](https://github.com/openai/codex)（含 Azure OpenAI 部署支援）
- Windows / macOS / Linux 都能 build；主力 dev 在 Windows MSVC

---

## Why "codebus"?

致敬 **「上車舞」** meme — 不用怕、跟著走、總會到站的 vibe。

讀陌生 codebase 不該是讓人焦慮的事。**上車就對了**。 🚌

---

## 上車吧

找早期乘客中 — issue / PR / 報 bug / 提建議都歡迎，駕駛座還有空位。

`codebus-app`（Tauri 桌面 app，把 wiki 變成可互動的「站點 + 抽考」教學介面）正在烤，目前已經能 lobby / 跑 goal / 抽 quiz / Cmd+K 聊天。

---

## License

[MIT](LICENSE)
