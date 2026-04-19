<!-- SPECTRA:START v1.0.1 -->

# Spectra Instructions

This project uses Spectra for Spec-Driven Development(SDD). Specs live in `openspec/specs/`, change proposals in `openspec/changes/`.

## Use `/spectra:*` skills when:

- A discussion needs structure before coding → `/spectra:discuss`
- User wants to plan, propose, or design a change → `/spectra:propose`
- Tasks are ready to implement → `/spectra:apply`
- There's an in-progress change to continue → `/spectra:ingest`
- User asks about specs or how something works → `/spectra:ask`
- Implementation is done → `/spectra:archive`

## Workflow

discuss? → propose → apply ⇄ ingest → archive

- `discuss` is optional — skip if requirements are clear
- Requirements change mid-work? Plan mode → `ingest` → resume `apply`

## Parked Changes

Changes can be parked（暫存）— temporarily moved out of `openspec/changes/`. Parked changes won't appear in `spectra list` but can be found with `spectra list --parked`. To restore: `spectra unpark <name>`. The `/spectra:apply` and `/spectra:ingest` skills handle parked changes automatically.

<!-- SPECTRA:END -->

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repo 現況

**Spec-first，尚未進實作**。目前只有：
- `docs/` — 19 份文件（14 份 Module / Agent / 橫切層 spec + `decisions.md` ADR + `README.md` / `dev-setup.md` / `implementation-plan.md` / `prompts.md`）
- `design/` — Phase A Trust Layer 的 3 份 HTML mockup（`r-01` / `o-01` / `o-05`；O-04 為 R-01 內 slide-in panel）+ 14 張截圖（見 `design/README.md`）
- `tests/golden/` — `demo-synthetic/`（比賽 demo / regression 合成 fixture）+ `timeline-gdrive-adapter/`（參考實作）
- `openspec/` — Spectra SDD：`specs/` 目前空、`changes/` 僅 `archive/`，尚未開 proposal

**實作目錄（`tauri/` `sidecar/` `web/`）尚未建立**，會在 Phase B 起手建。

## 溝通語言

使用者偏好 **繁體中文（zh-TW）** 回覆（見 `~/.claude/projects/-home-asus-codebus/memory/`）。Spec 內文也是 zh-TW；schema / code / filename 維持英文。

## 架構快照

**混合架構**（D-001）：Tauri 2.0 殼（Rust）↔ Python Sidecar（FastAPI）↔ Qdrant（本地向量 DB）。前端 Nuxt3 + TypeScript + Tailwind。IPC 走 `localhost:<random-port>` + Bearer token（見 `docs/sidecar-api.md §一`）。

**八大 Module**（完整清單在 `README.md §五`）：
- Module 1 Scanner → Module 2 KB Builder → Module 4 Explorer Agent → Module 5 Generator → Module 7 前端 → Module 8 Q&A Agent
- Module 3（Topic Explorer）Phase 2；Module 6（Intervention）前端實作期決定

**Agent 核心**（D-012）：自寫 ReAct loop + Instructor/Pydantic structured output。Explorer 與 Q&A Agent **共用** ReAct core，靠 `ExplorerTools` / `Judge` / `CoverageChecker` Protocol 抽象（`docs/agent-explorer-spec.md §十二`）。

**Trust Layer 四站**（Phase A，敘事核心 — 評審會停在這邊）：
- **R-01** Workspace（主畫面 + 六層 audit 面板）
- **O-04** LLM Call Inspector（R-01 內 slide-in panel，秀 wire payload）
- **O-05** Sanitizer Diff（LOCKED/UNLOCKED 稽核畫面）
- **O-01** Grant Modal（workspace 授權）

**三段 Sanitizer**（D-015）：Pass 1 Scanner 入 KB 前 → Pass 2 Provider pre-flight 每次 LLM call 前 → Pass 3 Q&A `add_to_kb` 寫入前。詳見 `docs/sanitizer.md §三`。

**七層 Audit JSONL**（workspace-level 六層 + App-level 一層）：
- `sanitize_audit.jsonl`（Sanitizer 命中）
- `tool_audit.jsonl`（Sandbox 工具呼叫）
- `kb_growth.jsonl`（Q&A add_to_kb）
- `reasoning_log.jsonl`（ReAct 每 step）
- `token_usage.jsonl`（D-021）
- `llm_calls.jsonl`（D-022 完整 wire payload）
- `~/.codebus/authorization_audit.jsonl`（跨 workspace，App-level，見 `docs/authorization.md §五`）

## 關鍵不變式（寫 spec / code 時必守）

1. **雙模 discriminator day 1**（D-002）：`workspace_type: "folder" | "topic"` 欄位從一開始就寫進 schema；MVP 只實作 `folder`，但 `topic` 加進來不能造成 breaking change。`ToolContext`（`docs/tool-sandbox.md §三`）、`POST /scan`（`docs/sidecar-api.md §三`）、`authorization_audit`（`docs/authorization.md §五`）都遵守此約。
2. **Sanitizer 單向**：placeholder `<REDACTED:kind#N>` 無 reverse mapping，一旦替換即不可逆；原值「不額外儲存」，原檔在本機原處，不 copy 到 KB/log/網路。
3. **LLM 看到的一定是 Sanitize 過的**：`llm_calls.jsonl` 記的是 post-Sanitizer Pass 2 版本，不還原 pre-sanitize 原文（D-022）。
4. **檔名 kebab-case**：`docs/*.md`、`design/*.html`、`design/screenshots/*.png` 一律 `{代號}-{語意}`。舊版直接刪，不留 `-v1` 後綴（歷史去 git log 找）。
5. **Spec 為主、mockup 其次**：`design/*.html` 與 `docs/*.md` 衝突時以 spec 為準，回頭修 mockup。
6. **Sanitizer rules 改動必 bump version**：`docs/sanitizer.md` 的 rule pattern 有任何增減，必須同步 bump rules version；`docs/authorization.md §六` 規定使用者同意需依版本重取。不得「靜默改 rule」——會造成既有 workspace 套用新 rule 但未重授權，稽核鏈斷裂。

## 決策記憶 — `docs/decisions.md`

所有非 trivial 的技術取捨都寫成 **D-XXX ADR**（脈絡 / 選項 / 理由 / 後續）。Spec 首行必引相關 D-XXX。改決策時**先改 `decisions.md`，再改引用它的 spec**。目前 D-001 ~ D-022+；常查：
- D-001 混合架構 / D-002 Topic mode 不進 MVP / D-003 LLM Provider 抽象
- D-008 三階段進度 / D-011 資安 / D-012 自寫 ReAct / D-014 uv toolchain
- D-015 Sanitizer / D-016 Q&A add_to_kb / D-017 ToolSandbox
- D-021 token_usage / D-022 llm_calls

## 實作期常用指令（`docs/dev-setup.md` 摘）

目錄尚未建立；以下是 Phase B 起手後的預期指令形態：

**Python Sidecar**（`sidecar/`，uv toolchain · D-014）
```bash
uv sync                            # 裝依賴 + 建 venv
uv run pytest                      # 全測
uv run pytest tests/unit/          # 單元測試
uv run pytest tests/golden/        # golden sample regression
uv run pytest tests/sandbox/       # red team（path escape 等）
uv run ruff check .
uv run pyright
uv run python -m codebus_agent.api --dev   # 單獨啟 sidecar
```

**前端**（`web/`，Bun）
```bash
bun install
bun run dev          # http://localhost:3000
bun run typecheck
bun run lint
bun test
```

**Tauri 殼**（`tauri/`，Rust）
```bash
cargo tauri dev      # 自動 spawn web + sidecar
cargo test
cargo tauri build    # 產 AppImage / MSI / dmg
```

打包鏈：PyInstaller 先打 sidecar binary → `cargo tauri build` 透過 `externalBin` 內嵌。

## 常見引用關係

改 spec 時容易漏掉的連動（`docs/README.md §五` 完整對照）：
- 改 `sanitizer.md` → 檢查 `authorization.md §六`（rules version bump 政策）、`sidecar-api.md` audit endpoints、`security.md` §3.x
- 改 `authorization.md` → 檢查 `sidecar-api.md` POST /scan schema（`workspace_type`）、`tool-sandbox.md §三` ToolContext、`design/o-01-grant-modal.html`
- 改 `agent-core.md` → 檢查 `agent-explorer-spec.md` §十二 trait、`qa-agent.md` §二、`prompts.md`
- 改 Module spec → 檢查 `implementation-plan.md` 依賴圖 + `sidecar-api.md` 對應 endpoint
