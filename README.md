# CodeBus

> Build an LLM wiki for any codebase via `claude -p`. Browse with Obsidian.

[![npm version](https://img.shields.io/npm/v/codebus.svg)](https://www.npmjs.com/package/codebus)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## What it does

Point codebus at any codebase + give it a goal. It spawns `claude -p` to
explore the code and incrementally builds a structured markdown wiki under
`.codebus/wiki/`. Open `.codebus/` in Obsidian to browse with backlinks /
graph view / Dataview queries.

## Install

```bash
# Prerequisite: install Anthropic Claude Code CLI first
npm install -g @anthropic-ai/claude-code

# Then install codebus
npm install -g codebus
```

Requires Node.js ≥ 20.

## Usage

```bash
# 1. Initialize vault (creates .codebus/ in your repo, adds it to .gitignore)
codebus --repo /path/to/your/repo

# 2. Build wiki for a goal
codebus --repo /path/to/your/repo --goal "了解購物車結帳流程"

# 3. Ask the wiki a question (read-only)
codebus --repo /path/to/your/repo --query "PaymentGateway 怎麼處理失敗?"
```

Open `<repo>/.codebus/` in Obsidian to browse the generated wiki.

## Flags

| Flag | Meaning |
|---|---|
| `--repo <path>` | repo path (default: cwd) |
| `--goal <text>` | build wiki for this goal |
| `--query <text>` | ask the wiki (read-only) |
| `--debug` | verbose stream-json output |
| `--emoji <auto\|on\|off>` | emoji mode (default: auto-detect TTY/CI) |
| `--no-emoji` | sugar for `--emoji off` |

Settings priority (emoji): CLI flag > `NO_EMOJI` env > `~/.codebus/config.yaml` `emoji:` > auto-detect.

## ⚠️ Security: goal/query text is fed directly to the LLM

The text you pass to `--goal` and `--query` becomes part of the system
prompt sent to Claude Code. **Do not paste content from untrusted sources**
(random GitHub issues, web pages, Slack messages from outside your team)
into `--goal` / `--query` — that is a prompt injection vector.

Phase 1 sandbox is best-effort:

- spawn `cwd = .codebus/` isolates the agent from your source repo
  (cwd-external Writes are system-blocked)
- `--permission-mode acceptEdits` + `--disallowedTools Bash,WebFetch,WebSearch`
- nested git auto-commits the wiki so you can `git -C .codebus reset --hard`
  if anything goes sideways

But within `.codebus/`, the agent can still write to `CLAUDE.md`, `.git/`,
and `goals.jsonl`. Phase 2 will add `--settings permissions.deny` to lock
those down.

## License

MIT — see [LICENSE](LICENSE).
