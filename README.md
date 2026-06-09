# 🚌 codebus

> **來囉來囉~** 跟 AI 上車探索陌生 codebase，每站發明信片，集結成你自己的程式碼旅遊書。
>
> *Build an LLM-maintained, Obsidian-compatible wiki for any codebase.*

**白話講**：codebus 驅動你已裝好的 AI coding agent（[Claude Code](https://claude.ai/code) 或 [OpenAI Codex](https://github.com/openai/codex)）邊讀你的 repo、邊把理解寫成一份結構化、可用 [Obsidian](https://obsidian.md)（免費的 markdown 筆記軟體）開啟的 wiki，存在 repo 裡的 `.codebus/` 資料夾（稱為 vault）。codebus 本身不含 LLM，是「驅動 agent CLI」的工具。

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

> **前置需求**：codebus 不含 LLM — 先裝好一個 agent CLI 並登入。預設是 [Claude Code](https://claude.ai/code)（OAuth）；也可改用 [OpenAI Codex](https://github.com/openai/codex)（見下方 [Provider 選擇](#provider-選擇)）。

```bash
# 先確保 Claude Code CLI 已安裝且 OAuth（預設 provider）：https://claude.ai/code
cargo install --path codebus-cli

cd ~/some/unfamiliar/repo

codebus init                                # 🚏 上車：建 vault
codebus goal "搞懂 auth 模組怎麼運作"        # 🚌 第一站，AI 開寫
codebus query "token 在哪驗證？"             # 💬 問一句、看現有 wiki（不改檔）
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

- **Claude**（預設）：裝 [`claude` CLI](https://claude.ai/code) 並 OAuth。若用 Azure 部署的 claude，用 `codebus config set-key azure` 把 API key 收進 OS keyring。
- **Codex**：裝 [`codex` CLI](https://github.com/openai/codex)。Azure OpenAI 部署的 provider 切換、model / effort、base_url / api_version 都在[桌面 app](#桌面-app) 的 Settings → Endpoint 設定（CLI 目前不提供 provider 切換指令）。Azure API key 放進 OS keyring，或設環境變數 `CODEBUS_AZURE_KEY=<your-key>`（keyring 取不到時的通用 fallback，claude / codex 皆適用）。

provider 切換、model / effort、Azure base_url / api_version 全在 app 的 Settings 介面設；CLI 共用同份 `~/.codebus/config.yaml`。

---

## 桌面 app

CLI 之外有一個 Tauri 桌面 app — goal / chat / quiz / wiki 預覽的圖形介面，也是設定 codex / Azure provider 的地方。

從原始碼跑：

```bash
cd codebus-app
npm install
npm run tauri dev          # 開發模式（需 Node 20+、Rust 1.85+）
```

桌面 app 還需要 [Tauri 的系統相依](https://tauri.app/start/prerequisites/)。Windows 想出安裝檔：在 repo 根目錄跑 `./build-installer.ps1` 產 NSIS `-setup.exe`（含 GUI + 內嵌 CLI）；標記版本（`v*` tag）也會由 CI 自動建置並發到 [GitHub Releases](https://github.com/harry18456/codebus/releases)（**未簽章**，Windows SmartScreen 會警告 → 仍要執行）。

---

## 平台支援

- **Windows** — 主要開發 / 測試平台，也是唯一有自動建置安裝檔的平台；codex provider 的沙箱隔離只在 Windows 實測過（細節見 [`docs/security.md`](docs/security.md) §5）。
- **macOS / Linux** — 能 build 與執行，但 codex 沙箱隔離尚未實測；涉及敏感家目錄讀取的任務建議走 claude provider。

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

- **claude** 走 `--tools` 白名單 + PreToolUse hook，讀寫都有 deterministic gate：**寫**鎖在 vault cwd；**讀**自 `check-read-vault-containment` 起為 **vault-root containment**——Read/Glob/Grep 的 path canonicalize 後不在 vault 內一律 block（母 repo 原始碼、`~/.ssh`/`~/.kube`/`~/.env` 等皆擋），`hooks.read_path_containment` 預設 on
- **codex** 走 `-s sandbox`（`read-only`／`workspace-write`）+ OS restricted token，但 Windows unelevated 實測（codex-cli 0.135.0）只是**部分**隔離：**寫**對正常 ACL 路徑有擋（workspace 外／家目錄回 `Access is denied`），但 Everyone-writable 目錄（如 `C:\Windows\Temp`）仍漏；**讀**照樣讀得到 workspace 外的檔與 `%USERPROFILE%` 內容（`~/.ssh`、`~/.aws` 等家目錄機密屬此類）；**網路**只擋外部 HTTPS/443，loopback 與外部 HTTP/80 仍外洩。→ codex 是**讀／網路 soft-partial、寫較硬**，敏感家目錄「讀」相關任務請用 claude 或自行承擔風險（細節見 [`docs/security.md`](docs/security.md) §5）

完整 threat model 跟每層防護怎麼運作：[`docs/security.md`](docs/security.md)。

---

## 還想看更多？

README 故意只放「目的 + 怎麼用」。其他往這走：

- 🏗️ **AI 怎麼被馴服成執行引擎（架構＋流程圖）** → [`docs/codebus-ai-architecture.md`](docs/codebus-ai-architecture.md)
- 📐 **capability 規格 / spec 在哪** → [`openspec/specs/`](openspec/specs/)

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
