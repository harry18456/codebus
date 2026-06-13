# 🚌 codebus

**English** | [繁體中文](README.zh-TW.md)

> Hop on. Explore an unfamiliar codebase with an AI, leave a postcard at every stop, and end up with your own travel guide to the code.
>
> *Build an LLM-maintained, Obsidian-compatible wiki for any codebase.*

In plain terms: codebus drives the AI coding agent you already have ([Claude Code](https://claude.ai/code) or [OpenAI Codex](https://github.com/openai/codex)) to read your repo and write down its understanding as a structured wiki you can open in [Obsidian](https://obsidian.md) (a free markdown note app), stored inside the repo under `.codebus/` (the *vault*). codebus ships no LLM of its own — it is a tool that drives an agent CLI.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
![Rust](https://img.shields.io/badge/rust-1.85+-blue)

---

## Why codebus

If you write software for a living, these probably sound familiar:

- You inherit an unfamiliar repo, and going from zero to a working mental model takes a day or two
- You read through it once, take no notes, and have to re-read everything next time
- A colleague asks "how does X work in this repo?" and you re-discover it from scratch
- You need to understand a module before starting a feature, but grep and IDE go-to-definition only take you so far

codebus forces that **intermediate state of understanding** to persist as a structured wiki — next time (or the next person), just read the wiki.

It is not an automatic doc generator. It is a **travel guide to the code, co-written with an AI**.

---

## Requirements

codebus ships no LLM — install and log in to an agent CLI first:

- [Claude Code](https://claude.ai/code) (default, OAuth login), or
- [OpenAI Codex](https://github.com/openai/codex) (incl. Azure OpenAI deployments — see [Providers](#providers))

Building the CLI from source also needs Rust 1.85+ (edition 2024).

> **💸 Cost warning — a subscription does not make this free.** codebus drives claude in headless mode (`claude -p`). Starting June 15, 2026, on Claude subscription plans (Pro / Max / Team / Enterprise) this usage draws from a separate monthly **Agent SDK credit** (e.g. US$20/month on Pro), not your interactive usage limits — once the credit runs out, Agent SDK requests stop until it refreshes (or spill over to pay-as-you-go usage credits if you've enabled them). API-key / Azure setups bill per token as usual, and the codex provider likewise consumes your OpenAI / Azure quota. Details: [Use the Claude Agent SDK with your Claude plan](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan).

---

## Install

```bash
# from a checkout of this repo
cargo install --path codebus-cli
```

`cargo install` drops `codebus` into `~/.cargo/bin/`. **Strongly recommended for `fix`** — while fixing, the driver calls `codebus lint` to check its own work; off PATH, it fixes blind.

Just want to try `init` / `goal` / `query` / `lint`? Running the `cargo build --release` binary by absolute path works too.

On Windows there is also a prebuilt installer (desktop app + bundled CLI) on [GitHub Releases](https://github.com/harry18456/codebus/releases) — see [Desktop app](#desktop-app).

---

## 30-second tour

```bash
cd ~/some/unfamiliar/repo

codebus init                                  # 🚏 Hop on: create the vault
codebus goal "understand the auth module"     # 🚌 First stop — the AI starts writing
codebus query "where are tokens validated?"   # 💬 One question, answered from the wiki (read-only)
codebus chat                                  # 💬 Multi-turn chat with the driver
codebus quiz "auth flow"                      # 🎓 Quiz yourself on what you read
codebus lint                                  # 🔍 Vehicle inspection
codebus fix                                   # 🛠️ The driver fixes it himself

# Open .codebus/wiki/ in Obsidian — the postcards are all there
```

You've arrived.

---

## Commands

| Command | What it does |
|---|---|
| `codebus init` | 🚏 **Board** — create `.codebus/` in the repo, register it with Obsidian |
| `codebus goal "..."` | 🚌 **Ride to a stop** — the AI explores, writes pages, auto-commits |
| `codebus query "..."` | 💬 **Quick question** — answered from the existing wiki (no writes) |
| `codebus chat` | 💬 **Chat with the driver** — multi-turn REPL; a good thread can be promoted into a goal |
| `codebus quiz "..."` | 🎓 **Pop quiz** — give a topic, get multiple-choice questions to check yourself |
| `codebus lint` | 🔍 **Vehicle inspection** — pure rules, no LLM call |
| `codebus fix` | 🛠️ **Repairs** — run lint → edit → re-lint until green |
| `codebus config` | 🔑 **Fuel card** — set / show / delete Azure endpoint API keys (OS keyring) |

Each wiki page is a markdown file in one of five buckets (a nod to Karpathy's 5-bucket scheme):

```
.codebus/wiki/
├─ concepts/      abstract ideas, design principles, mental models
├─ entities/      data structures, schemas, records
├─ modules/       code organization units, libraries, services
├─ processes/     workflows, state machines, ordered algorithms
├─ synthesis/     cross-page summaries
├─ index.md       the route map (table of contents for all postcards)
└─ log.md         the travel log (timeline of every ride)
```

The design follows [Karpathy's "LLM Wiki" pattern](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) — typed folders + wikilinks, grown goal by goal, not one-shot RAG.

---

## Desktop app

Alongside the CLI there is a Tauri desktop app — a GUI for goal / chat / quiz / wiki preview, and the place to configure the codex / Azure provider.

Run from source:

```bash
cd codebus-app
npm install
npm run tauri dev          # dev mode (needs Node 20+, Rust 1.85+)
```

The desktop app additionally needs [Tauri's system prerequisites](https://tauri.app/start/prerequisites/). To produce a Windows installer, run `./build-installer.ps1` at the repo root for an NSIS `-setup.exe` (GUI + bundled CLI); tagged versions (`v*` tags) are also built by CI and published to [GitHub Releases](https://github.com/harry18456/codebus/releases) (**unsigned** — Windows SmartScreen will warn → choose to run anyway).

---

## Providers

codebus drives the **Claude Code CLI** by default (the 30-second tour assumes it). The **OpenAI Codex CLI** (incl. Azure OpenAI deployments) is supported as a second provider:

- **Claude** (default): install the [`claude` CLI](https://claude.ai/code) and log in via OAuth. For an Azure-deployed claude, store the API key in the OS keyring with `codebus config set-key azure`.
- **Codex**: install the [`codex` CLI](https://github.com/openai/codex). Provider switching, model / effort, and Azure base_url / api_version are configured in the [desktop app](#desktop-app) under Settings → Endpoint (the CLI has no provider-switch command yet). Put the Azure API key in the OS keyring, or set `CODEBUS_AZURE_KEY=<your-key>` (a generic fallback when the keyring is unavailable; applies to both claude and codex).

All provider settings live in the app's Settings UI; the CLI shares the same `~/.codebus/config.yaml`.

---

## Platform support

- **Windows** — primary dev / test platform and the only one with automated installer builds; codex sandbox isolation has only been measured on Windows (details in [`docs/security.md`](docs/security.md) §5).
- **macOS / Linux** — builds and runs, but codex sandbox isolation is untested there; for tasks involving sensitive home-directory reads, prefer the claude provider.

---

## ⚠️ Security note: your words are fed to an LLM

Whatever you type into `goal` / `query` / `chat` / `quiz` becomes part of the system prompt sent to the agent.

**Don't paste untrusted content (random GitHub issues, external web pages, Slack messages) wholesale** — that is a prompt-injection vector; passengers can steer the driver off route.

codebus ships several layers of protection (cwd isolation, a PII filter, nested git for easy rollback, plus provider-specific command/tool restrictions), but they are not a silver bullet — and **isolation strength differs by provider**:

- **claude** uses a `--tools` allowlist + PreToolUse hooks, with deterministic gates on both reads and writes: **writes** are locked to the vault cwd; **reads**, since `check-read-vault-containment`, enforce **vault-root containment** — any Read/Glob/Grep path that does not canonicalize into the vault is blocked (parent-repo source, `~/.ssh` / `~/.kube` / `~/.env`, all of it), with `hooks.read_path_containment` on by default
- **codex** uses the `-s` sandbox (`read-only` / `workspace-write`) + an OS restricted token, but as measured on Windows unelevated (codex-cli 0.135.0) the isolation is only **partial**: **writes** are blocked on normal-ACL paths (outside the workspace / home dir → `Access is denied`) but Everyone-writable dirs (e.g. `C:\Windows\Temp`) still leak; **reads** still reach files outside the workspace and inside `%USERPROFILE%` (home-dir secrets like `~/.ssh`, `~/.aws` fall in this class); **network** only blocks external HTTPS/443 — loopback and external HTTP/80 still get out. → codex is **soft-partial on reads/network, harder on writes**; for read-sensitive home-directory tasks use claude or accept the risk (details in [`docs/security.md`](docs/security.md) §5)

Full threat model and how each layer works: [`docs/security.md`](docs/security.md).

---

## Why markdown + Obsidian

- **The wiki outlives the tool** — plain markdown, no lock-in
- **Obsidian works out of the box** — backlinks / graph view / Dataview, all for free
- **Hand-edits welcome** — if the AI got something wrong, just fix it; the next lint won't complain
- **Git friendly** — every goal / fix ends with an auto-commit, so the wiki's full evolution history is kept

---

## Want more?

The README deliberately covers only "what it's for + how to use it". For the rest:

- 🏗️ **How the AI is tamed into an execution engine (architecture + flow charts)** → [`docs/codebus-ai-architecture.md`](docs/codebus-ai-architecture.md)
- 📐 **Capability specs** → [`openspec/specs/`](openspec/specs/)

---

## Development

```bash
cargo check --workspace        # fast type-check
cargo test --workspace         # run all tests
cargo build --release          # produce target/release/codebus
cargo clippy --workspace       # lint
cargo fmt --all                # format
```

Needs Rust 1.85+ (edition 2024) and at least one provider CLI (see [Requirements](#requirements)). All three platforms build; primary dev happens on Windows MSVC.

---

## Why "codebus"?

A tribute to the **"get on the bus" dance meme (上車舞)** — don't overthink it, follow along, you'll get there.

Reading an unfamiliar codebase shouldn't be stressful. **Just hop on.** 🚌

---

## Hop on

Looking for early riders — issues / PRs / bug reports / suggestions all welcome (see [CONTRIBUTING.md](CONTRIBUTING.md)); there are still seats up front.

`codebus-app` (the Tauri desktop app that turns the wiki into an interactive "stops + quizzes" learning UI) is still in the oven — the lobby, running goals, quizzes, and Cmd+K chat already work.

---

## License

[MIT](LICENSE)
