## Why

v1（Tauri 殼 + Python sidecar + Nuxt 3 + 自寫 ReAct agent + 7 層 audit）開發 8-9 週後停下，根因是 spec → UI → 最後串接 LLM 行為的順序，讓最不確定的 LLM 決策品質被推到最後沒時間 iterate。v2 reframe 為「先 CLI 把 LLM 行為跑通，最後才做 GUI」，phase 1 落地最小可用的 wiki ingest/query 能力，未來 phase 2/3 在這個 CLI 上加 PII filter / 多 provider / GUI。

## What Changes

- 新建 codebus npm package（TypeScript，npm install -g 對齊「上 npm」目標），spawn claude -p 當 agent runtime（不自寫 ReAct loop）
- `codebus --repo <path>` 在 user repo 內初始化 `.codebus/` vault（nested git + 內建 CLAUDE.md schema + raw/code 雛形 + 在 source repo `.gitignore` 加 `.codebus`）
- `codebus --repo <path> --goal "<text>"` 執行 ingest flow：sync raw/code → 給 agent 探索 → agent 寫 wiki page → codebus enrich sha256 + at_commit → stale-detect → nested git auto-commit
- `codebus --repo <path> --query "<text>"` 執行 read-only flow：agent 只讀 `wiki/*` 答問題 + cite [[wikilink]]，不寫檔（filing-back 留 phase 1.5）
- 終端輸出 hybrid emoji/symbol（4 per-event + 4 banner emoji；非 TTY / CI / `NO_EMOJI` 自動 fallback 到 symbol）
- 全域設定檔 `~/.codebus/config.yaml`（phase 1 只 `emoji` 欄位，folder + load logic 為 phase 2 加 provider/token 欄位預留）
- Sandbox 模型（spec §3.2 + spike E 驗證）：spawn cwd = `.codebus/`（system-level user source repo 隔離）+ `--permission-mode acceptEdits` + `--disallowedTools Bash,WebFetch,WebSearch`（query mode 加 Write,Edit）
- License: MIT；built-in schema 帶 SPDX header；clean-room 對 LLM Wiki GPL v3（只借 idea，自寫 implementation）

## Non-Goals

明確 defer 到後續 phase 的功能：

- Query filing-back（好答案寫回 wiki 成新 page）→ phase 1.5
- Tutorial 生成 / PII filter / 多 LLM provider / token-cost tracking → phase 2
- Auto re-explore stale pages（phase 1 只 detect-and-flag）→ phase 2
- `--settings permissions.allow` 白名單（cwd 內細粒度寫保護，settings glob syntax 待 spike）→ phase 2
- File lock stale detection（PID alive check）/ init recovery from partial state → phase 2
- 任何 GUI（Tauri 殼 + Nuxt 4）→ phase 3
- LanceDB / vector RAG → phase 3+（小規模 wiki 不需要）

不採用的方向：

- 不自定 tool — 用 Claude Code 內建 Read/Grep/Glob/Write/Edit，phase 1 無 tool 抽象需求
- 不寫程式 sandbox hook — phase 1 靠 cwd 隔離 + acceptEdits + disallowedTools + agent self-judgment + nested git rollback 兜底
- `--allowedTools` 白名單 vs `--disallowedTools` 黑名單 — phase 1 用黑名單；forward-compat 改白名單留 phase 2 評估

## Capabilities

### New Capabilities

- `vault-init`: 初始化與維護 `.codebus/` vault 結構（含 raw/code/、wiki/、output/、nested `.git/`、內建 CLAUDE.md schema、source repo `.gitignore` 整合、file-based lock）
- `wiki-ingest`: `--goal` flow — 探索 codebase 並 incremental 建立/更新 wiki pages，含 source dedup、append-merge page conflict、UTC date 標記、sha256 + at_commit enrichment（codebus 後處理）、stale-detect flagging（不 auto re-explore）、nested git auto-commit
- `wiki-query`: `--query` flow — 純讀 wiki 答問題 + cite，read-only 模式（Write/Edit hard-disable），不寫檔不 commit
- `terminal-output`: 串流事件（agent thoughts / tool_use / tool_result）→ 終端 emoji/symbol 顯示，含 5-level emoji mode 優先序解析（CLI flag → env → global config → auto-detect → default）

### Modified Capabilities

(none — greenfield repo)

## Impact

- Affected specs: 4 個新 capability spec（vault-init / wiki-ingest / wiki-query / terminal-output）
- Affected code:
  - New (codebus 自身, npm package):
    - package.json
    - tsconfig.json
    - vitest.config.ts
    - .gitignore
    - LICENSE
    - README.md
    - src/cli.ts
    - src/commands/init.ts
    - src/commands/goal.ts
    - src/commands/query.ts
    - src/core/vault/layout.ts
    - src/core/vault/lock.ts
    - src/core/wiki/types.ts
    - src/core/wiki/frontmatter.ts
    - src/core/wiki/frontmatter-repair.ts
    - src/core/wiki/page-merge.ts
    - src/core/wiki/stale-detect.ts
    - src/infra/fs/file-ops.ts
    - src/infra/fs/raw-sync.ts
    - src/infra/git/source-version.ts
    - src/infra/git/nested-repo.ts
    - src/infra/llm/types.ts
    - src/infra/llm/claude-cli.ts
    - src/infra/global-config.ts
    - src/ui/emoji-mode.ts
    - src/ui/render.ts
    - src/ui/stream-parser.ts
    - src/schema/claude-md.ts
    - tests/ 完整 unit suite + tests/e2e/init-smoke.test.ts
  - Modified: (none — greenfield, codebus 是新 repo branch)
  - Removed: (none)
- External deps（全 MIT，與 codebus license 相容）：commander, chalk, ora, gray-matter, simple-git, js-yaml；devDeps: typescript, tsx, vitest, @types/node, @types/js-yaml
- 上線通路：npm publish 到公開 registry；user 需先安裝 @anthropic-ai/claude-code（spawn subprocess 不創 derivative work，無 license 衝突）
- 歷史紀錄保留：docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md 與 docs/superpowers/plans/2026-05-04-codebus-v2-phase1.md 保留為 brainstorm + 8 輪 review iteration 紀錄，不納入 spectra 工作流；spectra change 為 ship-time 工作的 source of truth
- 過程紀錄：docs/superpowers/REVIEW_LESSONS.md 跨 phase 保留 review process lessons（含 #8「Spec convergence ≠ plan convergence」）
