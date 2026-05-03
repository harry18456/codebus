# CodeBus v2 — Phase 1 Design

> **狀態**：2026-05-04 brainstorm 完成，待 user review → writing-plans
> **Branch**：`main`（v2 從 e816a29 `feat: start v2` 起）
> **Reference**：`v1-archive` branch、`D:/side_project/llm_wiki/KNOWLEDGE.md`、Karpathy `llm-wiki.md` gist

---

## 1. 背景與動機

### 1.1 v1 為何停下來
v1 (`v1-archive` branch) 經 8-9 週 spec-driven 開發：Tauri 殼 + Python FastAPI sidecar + Nuxt 3 前端 + Qdrant + 自寫 ReAct agent + 7 層 audit。開發順序是 **spec → UI → 最後串接 LLM 行為**，導致：

- LLM 行為調整時要穿過 spec / UI / IPC 三層僵化的東西
- 後續加新功能（hot-swap settings / SSE wire / onboarding）每次都拖很大 change（D-033 B 5 工作天）
- 真正的 unknown（agent 決策品質）被推到最後，沒時間 iterate

### 1.2 v2 reframe

**核心方向**：先 CLI 把 LLM 行為跑通，逐步加 cross-cutting 功能，最後做 GUI。

| Phase | 範圍 |
|---|---|
| **1（本 doc）** | CLI 基本功能 + LLM wiki 建立流程 |
| **2** | PII filter / 多 model / token tracking / auto re-explore stale pages |
| **3** | Tauri (Rust) + Nuxt 4 (TS) GUI 殼，spawn CLI 當 backend |

### 1.3 為何選 LLM Wiki pattern

Karpathy 的 `llm-wiki.md`（`D:/side_project/llm_wiki/llm-wiki.md`，作者明示 "designed to be copy pasted"）描述：LLM 不是每次 query 重 retrieve raw（傳統 RAG），而是 **incrementally builds and maintains a persistent wiki**。Wiki 是 compounding artifact，cross-references / contradictions / synthesis 都已 file 好。

對 codebus 場景的 mapping：

| Karpathy 概念 | LLM Wiki 實際 | Codebus v2 對應 |
|---|---|---|
| Raw sources | `raw/sources/`（匯入文件）| `.codebus/raw/`（**codebase 本身的複製**） |
| Wiki | `wiki/` | `.codebus/wiki/` |
| Schema | `CLAUDE.md`（root .md 檔）| `.codebus/CLAUDE.md` |
| Operations: Ingest / Query / Lint | 三大 LLM 操作（§1.3.1 詳述） | Phase 1: **Ingest 完整 + Query 純問答** / Lint 部分（只 stale detect-and-flag）|

### 1.3.1 三大 Operations 在 codebus 怎麼對應

Karpathy 把 LLM 對 wiki 的操作分三類，phase 1 對應如下：

- **Ingest**（建立 / 增量更新 wiki）：對應 `codebus --goal "X"` 整條 flow — 讀 raw → 探索 codebase → 寫 wiki pages → update index/log/goals。**是 phase 1 主軸**，§7 詳述完整 sequence。
- **Query**（用 wiki 答問題）：Karpathy 原意是 user 對 wiki 提問，LLM 搜 wiki + 答覆 + citations，好答案 file 回 wiki 成新 page。**Phase 1 純問答** — `codebus --query "X"` 讀 `wiki/index.md` → 找相關 page → Read 那些 page → 答 + cite，answer 印 terminal。**Filing-back（好答案 file 回 wiki 成新 page）留 phase 1.5** — phase 1 query 讀 wiki 不寫 wiki。Obsidian 仍用於被動瀏覽（兩種 query 方式互補：CLI 主動問 / Obsidian 主動瀏覽）。
- **Lint**（定期 health check）：Karpathy 原意是找 contradictions / stale claims / orphan pages / missing cross-references / 知識 gap。**Phase 1 只做一小塊** — stale detect-and-flag（codebase 變動時標 page `stale: true`，§10 詳述）。完整 Lint（contradictions / orphans / coverage check）留 phase 2+。

---

## 2. 產品定位 (Phase 1)

### 2.1 一句話

給工程師一個 CLI：用 `--goal` 探索 codebase 建 wiki、用 `--query` 對 wiki 直接問問題、用 Obsidian 被動瀏覽。

### 2.2 目標 user

工程師 ramp up 陌生 codebase（v1 原 use case 延續）。

### 2.3 在 phase 1 範圍

- `codebus --repo <path> --goal "..."` 完整 ingest flow
- `codebus --repo <path> --query "..."` 純問答（讀 wiki + cite，不 file 回去）
- 建 / 維護 `.codebus/` vault（含 wiki）
- Append-merge incremental wiki 累積
- Source dedup（避免重 read 同 file）
- Codebase 變動 detect-and-flag

### 2.4 不在 phase 1（明確 defer）

- Query filing-back（好答案 file 回 wiki 成新 page；**phase 1.5** 才加）
- Tutorial 生成（透過 skill，phase 後段）
- PII filter
- 多 LLM provider
- Token / cost tracking
- 任何 GUI（Tauri + Nuxt 4 是 phase 3）
- Auto re-explore stale pages
- LanceDB / vector RAG

---

## 3. 架構

### 3.1 兩層分工

```
┌────────────────────────────────────────────────────┐
│  codebus (TypeScript binary, npm install -g)       │
│  - parse args                                       │
│  - init .codebus/ vault                             │
│  - copy codebase → raw/ (gitignore-aware filter)    │
│  - record commit hash / sha256                      │
│  - spawn claude -p subprocess                       │
│  - parse stream-json events                         │
│  - render emoji terminal output                     │
│  - auto-commit nested .codebus/.git                 │
└─────────────────────┬──────────────────────────────┘
                      │
                      │ spawn (cwd = repo_root)
                      ▼
┌────────────────────────────────────────────────────┐
│  claude -p (Anthropic Claude Code CLI)             │
│  Flags（ingest mode, --goal）:                     │
│    --output-format stream-json                      │
│    --input-format stream-json                       │
│    --verbose                                        │
│    --add-dir .codebus/                              │
│    --disallowedTools Bash,WebFetch,WebSearch        │
│  Flags（query mode, --query）:                     │
│    同上 + Write,Edit 也 disallow                   │
│  Tools available（ingest）:                         │
│    Read, Grep, Glob, Write（限 .codebus/ 內寫）    │
│  Tools available（query）:                          │
│    Read, Grep, Glob（讀 wiki 不寫）                │
│  讀 .codebus/CLAUDE.md schema 學 wiki 規則         │
│  Ingest: 探索 raw/ → 寫 wiki/                      │
│  Query: 讀 wiki/ → 答 + cite                       │
└────────────────────────────────────────────────────┘
```

### 3.2 codebus 不自定 tool

完全用 Claude Code 內建 tools。Phase 1 safety 靠：

- Prompt + schema 約束
- `--add-dir .codebus/` 限制 Write 範圍
- `--disallowedTools` 禁危險工具
- 不加程式 hook（phase 2 才補 sandbox）

### 3.3 Stack

| Phase | Stack | 為何 |
|---|---|---|
| 1 (CLI) | Node.js + TypeScript | npm install -g 對齊「上 npm」心願 / Anthropic SDK + claude-code 都 TS first / phase 3 對齊 Nuxt 4 |
| 2 | TypeScript（延續）| 加 Anthropic SDK provider 抽象 / PII filter |
| 3 (GUI) | Tauri (Rust) + Nuxt 4 (TS) | Tauri 殼 spawn codebus CLI 當 backend，沿用 v1 sidecar pattern |

### 3.4 Distribution

- 開發：repo 內 `npm link` 本地連
- 上線：`npm publish` 到 npm registry，user 跑 `npm install -g codebus`

### 3.5 Module Architecture（為 phase 2/3 替換性留邊界）

採 hexagonal / ports-and-adapters pattern — core domain 跟 side-effect 分離，phase 2/3 升級（加 PII / 多 provider / Tauri）時加新 adapter file 不動 core。

**三層分離:**

| 層 | 內容 | 對外界依賴 |
|---|---|---|
| **core/** | 純 domain：wiki 規則、frontmatter、page-merge、stale-detect、vault layout、lock | 無（無 LLM / 無 disk / 無 process）|
| **infra/** | side-effect adapters：fs / git / llm provider | 有外界依賴（spawn / fs / network）|
| **ui/** | rendering：stream-parser / terminal render | 有 process IO |
| **commands/** | thin orchestration：init / goal / query | 拼上面三層 |
| **schema/** | 內建 `.codebus/CLAUDE.md` 範本 | 無 |

**LLMProvider interface（核心 port）:**

```typescript
// src/infra/llm/types.ts
export type StreamEvent =
  | { type: 'thought'; text: string }
  | { type: 'tool_use'; name: string; args: unknown }
  | { type: 'tool_result'; output: string; error?: string }
  | { type: 'done' }

export interface LLMProvider {
  invoke(opts: {
    systemPrompt: string
    userMessage: string
    mode: 'ingest' | 'query'
  }): AsyncIterable<StreamEvent>
  cancel(): void
}
```

**Phase 1 唯一 adapter:**

```typescript
// src/infra/llm/claude-cli.ts
export class ClaudeCliProvider implements LLMProvider { ... }
```

**Phase 2 加 adapter（不動 core）:**

```typescript
// src/infra/llm/anthropic-sdk.ts (phase 2)
export class AnthropicSdkProvider implements LLMProvider { ... }

// src/infra/llm/openai-sdk.ts (phase 2)
export class OpenAiSdkProvider implements LLMProvider { ... }
```

**Phase 2/3 預期會被取代 / 擴展的 module:**

| Module | Phase 1 | Phase 2/3 取代成 |
|---|---|---|
| `infra/llm/` | claude-cli only | + anthropic-sdk / openai-sdk / gemini / ... |
| `ui/stream-parser.ts` | parse claude-cli stream-json only | dispatch by provider type（各 SSE format）|
| `ui/render.ts` | terminal emoji | + emit events to Tauri webview (phase 3) |
| `infra/fs/raw-sync.ts` | direct copy | + PII filter at copy boundary (phase 2) |

**Phase 1 不抽象的（避免過度設計）:**

- `core/wiki/page-merge.ts` 不抽象成 strategy pattern — phase 1 只一種策略 (append-merge)，phase 2 加 LLM body merge 時再 refactor
- `infra/git/` 不抽象 — git CLI 不會換
- Tool registry — phase 1 用 Claude Code 內建，無 tool 抽象需求

---

## 4. Disk Layout

```
your-repo/                       ← user 的 source repo（任意 layout）
├── (你的 codebase)              ← 可以是 src/ / lib/ / app/ / cmd/ / packages/，
│                                  或根目錄平鋪（Python/Go 慣例）。
│                                  **整個 repo 除 .codebus/ 跟 .git/ 之外都算 codebase**
├── .git/                        ← codebus 不複製
├── .gitignore                   ← codebus 自動加 .codebus
└── .codebus/                    ← codebus vault（Obsidian 開這）
    ├── .git/                    ← nested repo（codebus 自動 init + auto-commit）
    ├── .gitignore               ← codebus 自管（cache 等）
    ├── goals.jsonl              ← codebus 內部 metadata
    ├── CLAUDE.md                ← schema（教 agent wiki 規則；§6 詳述）
    ├── raw/                     ← codebase 完整 copy（每跑 goal 重 copy）
    │   └── ...
    ├── wiki/                    ← LLM 寫的 wiki
    │   ├── overview.md          ← repo 全貌
    │   ├── index.md             ← page catalog (Karpathy index.md)
    │   ├── log.md               ← chronological append-only (Karpathy log.md)
    │   ├── pages/               ← 平鋪 wiki pages
    │   │   ├── checkout-flow.md
    │   │   └── payment-gateway.md
    │   └── goals/               ← per-goal reading guides
    │       └── 了解購物車結帳流程.md
    └── output/                  ← phase 2+: tutorial / slide / chart
        └── (phase 1 default 空，先建 placeholder)
```

### 4.1 raw/ 為何複製整個 codebase

- 為 phase 2 PII filter 提供 boundary anchor（過濾發生在 src→raw 邊界，agent 永遠讀 raw 不知道有過濾）
- 跟 LLM Wiki "raw is immutable" 哲學對齊
- Agent 探索時直接讀 `.codebus/raw/` 不必跑出 .codebus/ 外（cwd 留在 .codebus/ 也行）

### 4.2 raw/ sync 策略

- 每跑 `--goal` 重 copy（覆蓋）src → raw
- `.gitignore`-aware filter：跳 `node_modules` / `dist` / `.git` / 大 binary / `.env`
- Init 時不 copy（init 只建空 folder + 加 .gitignore），第一個 goal 才填 raw

### 4.3 .codebus/ 為何 nested git repo

- Wiki 該有版本歷史 / rollback / diff
- 但不汙染 source repo PR
- User 想跨機器同步可自己 push 到 private remote
- Phase 2 想加 `codebus wiki publish` 把選定 page 複製到 source repo `docs/`

---

## 5. CLI Command Surface (Phase 1)

```
codebus --repo <path>
    Init only：
      在 <path> 建 .codebus/ + 加 .gitignore + nested git init + 寫 CLAUDE.md schema
    
codebus --repo <path> --goal "<goal-text>"
    若 init 未做 → 先 init
    執行 goal flow（§7 詳述）
    
codebus --repo <path> --goal "<goal-text>" --debug
    Verbose 模式，多印 stream-json raw events

codebus --repo <path> --query "<question>"
    純問答：讀 .codebus/wiki/ 既有 pages → LLM 答 + cite → 印 terminal
    若 .codebus/wiki/pages/ 為空 → 報錯「請先用 --goal 建 wiki」
    Phase 1 不 file 答案回 wiki（phase 1.5 才加 filing-back）

codebus --repo <path> --query "<question>" --debug

codebus --version
codebus --help
```

**Global flags（任何 command 都適用）:**
- `--debug` — verbose，多印 stream-json raw events
- `--no-emoji` — 強制純文字 + unicode symbol（CI / log file / 企業環境用）；亦可設 `NO_EMOJI=1` env var；預設 `auto`：偵測 `process.stdout.isTTY` + `process.env.CI` + `process.env.TERM`，非 tty / CI 環境自動 fallback

`--repo` 預設值：cwd（沒指定時用當前目錄）

---

## 6. Wiki Schema (`.codebus/CLAUDE.md`) Outline

12 個 section（內容細節留實作期 iterate；Karpathy 強調 schema 跟 LLM co-evolve）：

| # | Section | 內容大綱 |
|---|---|---|
| 1 | Your Role | 你是 codebus wiki maintainer / goals / non-goals |
| 2 | Workspace Layout | 能讀 raw/ wiki/ ; 能寫 wiki/ ; 不該碰 raw/ output/ goals.jsonl .git/ |
| 3 | Wiki Structure | 4 special files (overview/index/log/goals) + pages/ + frontmatter |
| 4 | Workflow per Goal (Ingest) | 7 步：Discover → Plan → Explore → Write → Index → Log → Guide |
| 5 | Page Conflict | 新建 vs append-merge / array union / locked fields (title/type/created) |
| 6 | Frontmatter Schema | title / type / sources (sha256+at_commit) / goals / wikilinks / stale |
| 7 | WikiLinks 約定 | 用 slug / YAML 列表 quote 必加 / body 內可不 quote |
| 8 | Source 引用 | frontmatter sources 寫法 + body fenced block 寫法 |
| 9 | Stopping Criteria | step budget / token budget / self-judgment（守住 goal scope）|
| 10 | Failure Modes | read/write 失敗 → log + skip 不無限 retry |
| 11 | Output Format | thoughts / tool_use / tool_result 會被 codebus render 成 emoji |
| 12 | Workflow per Query | 4 步：Read index → Identify pages → Read pages → Answer + cite (phase 1 不 file 回去) |

### 6.1 Frontmatter Schema (per page)

```yaml
---
title: Payment Gateway
type: concept                    # concept | module | process | entity
sources:
  - path: src/services/payment.py
    sha256: abc123...
    at_commit: deadbeef
goals:
  - "了解購物車結帳流程"
created: 2026-05-04
updated: 2026-05-04
related:
  - "[[checkout-flow]]"
stale: false                     # phase 1 stale-detect flag
---
```

### 6.2 `goals/<slug>.md` per-goal reading guide

每跑一個 goal 都產生一份，用途：給 user 在 Obsidian 打開 wiki 時的入口檔（不必從 `pages/` 一堆 .md 中猜從哪讀起）。

```markdown
---
goal: 了解購物車結帳流程
created: 2026-05-04
pages: ["[[checkout-flow]]", "[[payment-gateway]]"]
---

# 了解購物車結帳流程

## 建議閱讀順序
1. **[[checkout-flow]]** — 結帳高層流程
2. **[[payment-gateway]]** — Stripe 整合細節

## Source files
- src/controllers/checkout_ctrl.py
- src/services/payment.py
```

---

## 7. `codebus --goal` 完整 Sequence

```
1. Parse args，validate --repo path 存在

2. If .codebus/ 不存在 → init:
   - mkdir .codebus/
   - 加 .codebus/ 到 source repo 的 .gitignore
   - git init .codebus/
   - 寫 .codebus/CLAUDE.md（內建 schema）
   - 寫 .codebus/.gitignore（cache 等）
   - touch .codebus/goals.jsonl
   - mkdir raw/ wiki/ wiki/pages/ wiki/goals/ output/

3. Sync raw:
   - rm -rf .codebus/raw/*
   - copy repo 內容（除 .codebus/ 跟 .git/）→ raw/
   - 套用 .gitignore-aware filter（跳 node_modules / dist / 大 binary / .env）

4. Record source version:
   - git rev-parse HEAD
   - git status --short → detect uncommitted
   - 若 uncommitted: warn user「raw is working tree not HEAD」
   - append goals.jsonl: {goal, source_commit, uncommitted, timestamp}

5. Compose system prompt:
   - 讀 .codebus/CLAUDE.md schema
   - + goal text
   - + 已有 wiki page list（給 agent 看 source dedup 線索）

6. Spawn claude -p:
   - cwd = repo_root
   - args: --output-format stream-json --input-format stream-json --verbose
           --add-dir .codebus/ --disallowedTools Bash,WebFetch,WebSearch
   - stdin: stream-json messages（含 system prompt）

7. Parse stream events from stdout，render emoji output（§8 詳述）

8. On agent done:
   - Validate wiki pages 寫得對（frontmatter parse / wikilink syntax）
   - Repair YAML wikilink list 不合法（自寫 util）
   - 確認 wiki/index.md / wiki/log.md / wiki/goals/<slug>.md 都更新

9. Stale check (phase 1 detect-and-flag):
   - grep wiki/pages/*.md frontmatter sources
   - 對比 raw/ 對應 file 的 sha256
   - mismatch → 標 page frontmatter `stale: true`
   - 印警告：「N 個 page based on 已變動的 source」

10. Auto-commit nested git:
    - git -C .codebus add -A
    - git -C .codebus commit -m "wiki: <goal>"

11. 印產出 banner：「🎉 抵達終點！wiki 已生成於 .codebus/wiki/」
12. 印 Obsidian 提示：「請用 Obsidian 開 .codebus/」
```

30 分鐘 timeout backstop（reasoning model 真的可能跑很久）。

---

## 7.5 `codebus --query` 完整 Sequence

```
1. Parse args，validate --repo path 存在 + .codebus/wiki/pages/ 非空
   - 若空 → 報錯「請先用 --goal 建 wiki」並 exit

2. Compose system prompt:
   - 讀 .codebus/CLAUDE.md schema
   - + query text
   - + wiki/index.md content（讓 agent 看有哪些 page 可讀）

3. Spawn claude -p:
   - cwd = repo_root
   - args 同 §7 step 6，但 disallowedTools 加 Write,Edit
     （query mode 不讓 agent 寫檔；filing-back 留 phase 1.5）
   - stdin: stream-json messages（含 system prompt + query）

4. Parse stream events，render emoji output（§8 一樣）
   - tool_use 主要看到 Read（讀 wiki page）
   - 不會看到 Write（已 disallow）

5. 累積 final answer text from content_block_delta

6. 印 final answer to terminal:
   - Highlight (chalk)
   - 列 cited pages（[[wikilink]] format）
   - 提示「想 file 回 wiki？phase 1.5 才支援」

7. End — 不 commit nested git（沒寫檔）
```

**Query mode 跟 Ingest mode 的差異:**
- 不重 copy raw（query 對 wiki 不對 codebase；agent 只 Read wiki/* 不 Read raw/）
- 不寫檔（disallowedTools 加 `Write,Edit`）
- 不 update `goals.jsonl`（沒 ingest event）
- 不做 stale check（沒新 sources）
- 不 auto-commit（沒寫檔）

30 分鐘 timeout backstop 一樣 apply。

---

## 8. Stream-json → Terminal 顯示對應

採 **Hybrid emoji mode**：預設 emoji（friendly），`--no-emoji` flag / `NO_EMOJI=1` env / 非 tty / CI 環境自動 fallback 到 symbol mode。Emoji 量收斂到 **核心 4 + banner 4 = 8 種**。

### 8.1 Per-event 4 種 emoji（stream events render）

| Stream Event | Emoji mode (default) | Symbol mode (--no-emoji / CI auto) | chalk 染色 |
|---|---|---|---|
| `stream_event.content_block_delta` (text) | `🤔 [Agent 思考] {text}` | `◆ thought {text}` | dim |
| `tool_use` (name=Read/Grep/Glob) | `🛠️ [呼叫工具] {name}({args})` | `→ tool {name}({args})` | cyan |
| `tool_use` (name=Write) | `✍️ [正在生成] {file_path}` | `+ write {file_path}` | green |
| `tool_result` (success) | `👀 [觀察結果] {summary}` | `← result {summary}` | dim |
| `tool_result` (error) | `👀 [觀察結果] {error}` | `← result {error}` | **red**（用 chalk 染色區分 error，不另加 emoji）|
| `assistant.content` (fallback for old CLI) | 同 thought | 同 thought | dim |
| `session_init` / `result_summary` | （忽略）| （忽略）| — |

### 8.2 Banner 4 種 emoji（lifecycle / hint，非 per-event）

| 場景 | Emoji mode | Symbol mode |
|---|---|---|
| 啟動 | `🚌 CodeBus 啟動！正在駛入 {path} ...` | `▶ CodeBus 啟動 ...` |
| 任務目標 | `🎯 任務目標：{goal}` | `◎ 任務目標：{goal}` |
| 完成 | `🎉 抵達終點！wiki 已生成於 .codebus/wiki/` | `✓ 完成。wiki 已生成於 .codebus/wiki/` |
| 提示 | `💡 請用 Obsidian 開 .codebus/` | `i 請用 Obsidian 開 .codebus/` |

### 8.3 EmojiMode 偵測邏輯

```typescript
type EmojiMode = 'auto' | 'on' | 'off'

function resolveEmojiMode(flag: EmojiMode): boolean {
  if (flag === 'on') return true
  if (flag === 'off') return false
  // auto: enable emoji 條件全要滿足
  return process.stdout.isTTY
      && !process.env.CI
      && !process.env.NO_EMOJI
      && process.env.TERM !== 'dumb'
}
```

詳細 chalk 配色微調（具體 hex / 是否粗體等）phase 1 實作期 iterate。

---

## 9. Page Conflict 處理（Append-merge）

| 情境 | 處理 |
|---|---|
| Page 不存在 | 新建（frontmatter + body） |
| Page 已存在 | frontmatter `sources` / `goals` / `related` array union；body 加 `## from goal: <X> (YYYY-MM-DD)` section；`updated` 改今天 |
| Locked fields | `title` / `type` / `created` 永不改 |

特殊檔處理：

| 檔 | 寫入策略 |
|---|---|
| `log.md` | append 一行 `## [date] goal: "X" → 涵蓋 page: [[A]], [[B]]` |
| `index.md` | 完整覆寫 catalog |
| `overview.md` | 完整覆寫 repo 全貌 |
| `goals/<slug>.md` | 每 goal 一份不衝突（slug 唯一）|

---

## 10. Sync 策略（Codebase 變動）

| 機制 | 行為 |
|---|---|
| 重 copy | 每跑 `--goal` 重 copy repo 內容（除 .codebus/ + .git/）→ raw/（覆蓋）|
| `.gitignore`-aware filter | 跳 `node_modules` / `dist` / `.git` / 大 binary / `.env` |
| Source version record | `goals.jsonl` 記 commit hash + uncommitted flag + timestamp |
| Page-level sha256 + at_commit | frontmatter 記每個 source file 的版本 |
| Stale detect-and-flag | 跑前對比 frontmatter sha256 vs 新 raw/ 對應 file，mismatch 標 `stale: true` + 印警告 |

> 注意：sync 只發生在 `--goal`，**`--query` 不重 copy raw**（query 對 wiki 不對 codebase；agent 只 Read wiki/* 不 Read raw/）。

Phase 2 才加：auto re-explore + incremental sync + PII filter at copy boundary。

---

## 11. License & Clean Room

### 11.1 License: MIT

Codebus 自身採用 **MIT License**。理由：
- Permissive — 保留 phase 3+ 商業化可能
- npm 生態 99% deps 是 MIT，相容性零問題
- 跟 GPL v3 LLM Wiki clean room 邊界清楚（不 copy code 即可）
- 簡單一頁 LICENSE 文字

#### 11.1.1 Phase 1 ship 前 LICENSE checklist

| # | Action | Why |
|---|---|---|
| 1 | 建 codebus repo root `LICENSE` 檔（MIT 範本）| 使用者 fork / npm consumer 知道 terms |
| 2 | `package.json` 設 `"license": "MIT"` | npm registry / `npm install` warning |
| 3 | README 加 License section + badge | discoverability |
| 4 | NOTICE 檔列 third-party deps（如有 Apache/BSD attribution 需求）| Apache/BSD 法律要求 |
| 5 | 內建 `.codebus/CLAUDE.md` schema header 加 SPDX 標 | 寫到 user 機器，標清楚這段是 codebus owner 的、不是 user 的（避免後續 derivative 爭議）|

#### 11.1.2 Dependencies license check

§13 toolkit 列的 deps：`commander` / `clipanion` / `chalk` / `ora` / `gray-matter` / `simple-git` / `vitest` / `tsx` — **全 MIT**，相容無問題。實作期裝任何新 dep 前 check `npm view <pkg> license` 確認 permissive。

#### 11.1.3 Anthropic CLI 的 nuance

`@anthropic-ai/claude-code` 是 Anthropic 商業 license，但 codebus **不是 import 這 package** — 是 spawn `claude` subprocess（user 自己 `npm install -g @anthropic-ai/claude-code`）。codebus runtime 無 npm 依賴關係，subprocess fork-exec 不創 derivative work，**不算 license issue**。

### 11.2 Clean Room 守則

LLM Wiki 是 GPL v3，不能直接 copy code（會 license contaminate）。

- ❌ 不打開 LLM Wiki 任何 .ts / .rs source code
- ✅ 看 KNOWLEDGE.md（doc 描述 idea）+ Karpathy gist（明示自由）+ Anthropic Claude CLI 公開 doc
- ✅ 借 idea / pattern / architecture，自己用 TypeScript 重寫實作

### 11.3 Honest Disclosure

Brainstorm 期間 Claude（協作 AI）讀過 LLM Wiki `frontmatter.ts` 約 200 行（含 `repairWikilinkLists` 函式 ~10 行）。函式 trivial（regex match + reformat），實務 risk 低。若想 100% 嚴格 clean room，可 commission 沒看過的人寫該函式。

---

## 12. LLM Wiki 借鑑清單 (Phase 1 借 idea，自己重寫)

| LLM Wiki | 借鑑點 | Phase 1 怎麼用 |
|---|---|---|
| §4.6 `claude-cli-transport.ts` (idea) | Spawn 命令 + stream-json event 結構 | 自己寫 spawn + parser；**反向**處理 tool_use（render 不丟） |
| §9.3 wikilink repair (idea) | LLM 寫 `related: [[a]], [[b]]` 不合法 YAML 需 repair | 自己用 TS regex 重寫 util |
| §9.4 寫入策略分流 (idea) | 寫入 dispatcher | log → append / index → 覆寫 / pages → merge |
| §9.4 frontmatter union + locked fields (idea) | Page 衝突合併規則 | sources/goals/related union；title/created lock |
| §9.5 `withProjectLock` (idea) | Per-project mutex | 改 file-based lock `.codebus/.lock` 跨 process |
| §13 (idea) | 30 分鐘 timeout backstop | claude -p 防無限等待 |
| §11.5 補強做法 (idea) | claude CLI 該下的 flag | `--add-dir .codebus/` + `--disallowedTools Bash,WebFetch,WebSearch` + cwd repo_root |

---

## 13. Phase 1 Toolkit 建議（spec 暫定，實作可調）

| 用途 | Lib |
|---|---|
| CLI framework | `commander` 或 `clipanion` |
| Terminal UI | `chalk` (color) + `ora` (spinner) + emoji 直出 |
| Frontmatter parse | `gray-matter` |
| WikiLink YAML repair | 自寫 regex util（clean room）|
| subprocess + stream | stdlib `child_process.spawn` + `readline` + `JSON.parse` |
| Git interaction | `simple-git` 或 `child_process.execSync('git ...')` |
| File hashing | stdlib `crypto` + `fs` |
| Test | `vitest` |
| Dev runner | `tsx` 直接跑 .ts 無 build step |
| Distribute | `tsc` 編成 JS → `npm publish` |

---

## 14. Phase 1 Repo 結構（codebus 自身）

```
codebus/                          ← v2 main branch
├── package.json                  ← name=codebus, bin=codebus, dependencies
├── tsconfig.json
├── .gitignore                    ← node_modules, dist
├── README.md
├── LICENSE                       ← 待定
├── src/
│   ├── cli.ts                    ← entry，commander 設 args
│   ├── commands/                 ← thin orchestration
│   │   ├── init.ts
│   │   ├── goal.ts               ← --goal (ingest)
│   │   └── query.ts              ← --query (純問答)
│   ├── core/                     ← 純 domain（無 LLM / disk / process dep）
│   │   ├── wiki/
│   │   │   ├── frontmatter.ts          ← parse / serialize
│   │   │   ├── frontmatter-repair.ts   ← wikilink YAML repair util（clean room）
│   │   │   ├── page-merge.ts           ← append-merge dispatcher
│   │   │   ├── stale-detect.ts         ← sha256 比對
│   │   │   └── types.ts
│   │   └── vault/
│   │       ├── layout.ts               ← .codebus/ structure constants
│   │       └── lock.ts                 ← file-based mutex（純 path 邏輯）
│   ├── infra/                    ← side-effect adapters
│   │   ├── fs/
│   │   │   ├── raw-sync.ts             ← copy repo → raw + gitignore filter
│   │   │   └── file-ops.ts             ← read/write 包裝
│   │   ├── git/
│   │   │   ├── source-version.ts       ← commit hash / uncommitted check
│   │   │   └── nested-repo.ts          ← .codebus/.git init / auto-commit
│   │   └── llm/
│   │       ├── types.ts                ← LLMProvider interface + StreamEvent schema
│   │       └── claude-cli.ts           ← phase 1 唯一 impl（spawn + 解析）
│   ├── ui/                       ← rendering
│   │   ├── stream-parser.ts            ← parse claude-cli stream-json → StreamEvent
│   │   └── render.ts                   ← StreamEvent → terminal emoji
│   └── schema/
│       └── claude-md.ts                ← 內建 .codebus/CLAUDE.md 範本
├── tests/
│   └── (vitest)
└── docs/
    └── superpowers/specs/
        └── 2026-05-04-codebus-v2-phase1-design.md   ← 本檔
```

---

## 15. Phase 1 Open Questions（spec 不阻擋，實作期定）

- Failure modes 全套（OAuth 過期跳什麼錯訊息 / Ctrl+C 半寫 page rollback / claude crash）
- `--repo` 路徑驗證 / 不是 git repo 時跳過 .gitignore 步驟
- Test strategy 細節（unit / integration / e2e 範圍）
- Demo repo for dev iteration（v1 用 Timeline，v2 還是嗎？）
- Stream-json render chalk 配色微調（具體 hex / 是否粗體 / dim 強度）
- `goals.jsonl` 完整 schema (extra metadata 欄位)
- Query 連續多輪是否要 session memory（phase 1 暫定 stateless / 每次 query 獨立）
- Query final answer terminal markdown 渲染程度（純文字 vs 簡單 markdown bold/list parse）

---

## 16. Phase 1.5 / 2 / 3 預告（不在本 spec 範圍）

### Phase 1.5 (Query Filing-back)

- Query 結束後 codebus 印 prompt：「file 答案回 wiki？(y/N/edit)」
- y → 寫到 `.codebus/wiki/answers/<date>-<slug>.md`（新 page）或 append 到 most-relevant page（user 選或 codebus 推）
- N → 答案只在 terminal 出現，不寫
- edit → 開 user `$EDITOR` 改完才 file
- Filing-back 寫入沿用 §9 page conflict 規則（append-merge / array union）
- Filed 後 nested git auto-commit `query: <question>`

### Phase 2 (PII / 多 model / token)

- PII filter at copy boundary (src → raw)
- Provider 抽象（Anthropic SDK / OpenAI SDK / 多 provider preset 表 from §4.7）
- Token / cost tracking
- Auto re-explore stale pages
- 完整 sanitize rules（YAML fence、`frontmatter:` 鍵）
- `isSafeIngestPath` 程式層 sandbox
- Reasoning chars 監測（DeepSeek-R1 等推理 model）
- LLM body merge with 70% length guard

### Phase 3 (GUI)

- Tauri (Rust) + Nuxt 4 (TS / Vue)
- Tauri 殼 spawn codebus CLI 當 backend
- tauri-plugin-http (CORS workaround)
- Multi-modal ContentBlock + image extraction + vision-caption pipeline
- `Origin: http://localhost` 寫死（Ollama 整合）
- Tutorial 透過 skill 生成（user 原意：在 Claude Code session 內呼 skill）

### 可選（phase 3+）

- LanceDB / vector RAG（規模大才考慮，Karpathy 也說 small scale 不用）
- Web Clipper（從 web 補充 raw/）

---

## Appendix A：v1 → v2 Mapping

| v1 概念 | v2 對應 |
|---|---|
| Tauri 殼 + Python sidecar | Phase 1 不要；Phase 3 才 Tauri 殼 spawn CLI |
| Self-written ReAct agent | claude -p 內建 agent loop |
| 7 層 audit logs | Phase 1 不做；Phase 2 加 token / sanitize / kb_growth |
| Sanitizer 三段 | Phase 2 PII filter |
| Sandbox `ensure_in_workspace` | Phase 1 靠 `--add-dir` + prompt；Phase 2 加程式 hook |
| Scanner / KB Builder modules | 不需要（Claude 自己用 Read/Grep 探索） |
| Generator (tutorial.md) | Phase 後段透過 skill 做 |
| Station board / Q&A drawer / Agent console | Q&A 功能 phase 1 已有 CLI 對等（`--query`）；GUI 形式（drawer/console）留 phase 3 |
| Spectra workflow | 沿用（managing v2 changes）|
| Qdrant | LanceDB（phase 3+ 才考慮，phase 1/2 不需要）|

---

## Appendix B：核心 Mindset 差異 vs LLM Wiki

| | LLM Wiki | Codebus v2 |
|---|---|---|
| 把 claude 當什麼用 | Completion engine（tool_use 丟掉，沒關 tools） | Agent + tool runtime（tool_use render；嚴限 tools 範圍）|
| Raw 來源 | 使用者匯入的外部文件（PDF / web clip）| Codebase 本身（複製整個 repo 除 .codebus/+.git/ → raw/）|
| 寫檔協定 | `---FILE: path---` 文字標記 + `parseFileBlocks` | Claude 內建 Write tool（更原生） |
| 規模 | RAG-scale（hundreds of pages, optional LanceDB） | Repo-scale（dozens-hundreds pages, no vector phase 1）|
| Sandbox | Path traversal 防禦在 TS parse 邊界 | Phase 1 靠 prompt + Claude `--add-dir`；Phase 2 加程式 hook |

---

## End of Phase 1 Design
