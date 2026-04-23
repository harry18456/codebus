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

## 溝通語言

使用者偏好 **繁體中文（zh-TW）** 回覆。Spec 內文、commit message、code comment 的 prose 也是 zh-TW；schema / 識別字 / filename / test name 維持英文。

## Repo 現況

M1「power-on」通電（2026-04-19 archive）後，資料層 + provider wiring 已補齊；下一個里程碑是 **Module 4 Explorer Agent P0**（`docs/implementation-plan.md` 步驟 16-23）。

**子系統**

- `sidecar/` — uv-managed Python 3.12 FastAPI sidecar。核心：app factory、ephemeral port + bearer、stdout handshake、`--parent-pid` watchdog、ToolSandbox（`ensure_in_workspace` + red team fixture）、LLMProvider Protocol + `ProviderRole` dispatch + `MockProvider` / `OpenAIEmbeddingProvider` / `OpenAIChatProvider`、`TrackedProvider`（唯一 `token_usage.jsonl` / `llm_calls.jsonl` 寫入路徑）、三段 Sanitizer 前兩段、Qdrant lifecycle、Module 1 Scanner、Module 2 KB Builder + KB query + dim-mismatch guard、SSE task skeleton、PyInstaller onefile spec。
- `tauri/src-tauri/` — Rust host + `sidecar_ping` command。`src/sidecar.rs` spawn 協定、`src/lib.rs::resolve_sidecar_path()` 在 packaged / dev 模式都找得到 sibling binary。
- `web/` — Nuxt 3 + Tailwind + TypeScript 骨架（npm，D-026）。目前只有 landing page 與 Sidecar Ping 按鈕；Trust Layer 四站（R-01 / O-01 / O-04 / O-05）mockup 已畫好但尚未實作。
- `openspec/specs/` — 11 個 capability spec：`app-packaging` / `folder-scanner` / `knowledge-base` / `llm-provider` / `qdrant-client` / `repo-layout` / `sanitizer` / `sidecar-runtime` / `tauri-shell` / `tool-sandbox` / `usage-tracking`。
- `docs/` — 14 份 Module / Agent / 橫切層 spec + `decisions.md` ADR + `README.md` / `dev-setup.md` / `implementation-plan.md` / `prompts.md`。
- `design/` — Phase A Trust Layer 的 3 份 HTML mockup（`r-01` / `o-01` / `o-05`）+ 14 張截圖。
- `tests/golden/` — `demo-synthetic/`（比賽 demo / regression fixture）+ `timeline-gdrive-adapter/`（參考實作）。
- `tests/fixtures/` — `precommit-violations/`（commit-gate 負測 fixture）。

**archive 時間軸**（由舊至新，每條 archive 對應 `openspec/changes/archive/YYYY-MM-DD-<name>/`）

| 日期 | Change | 重點 |
|---|---|---|
| 2026-04-19 | `m1-power-on` | Tauri ↔ Sidecar 通電骨架、bearer、handshake、watchdog、Provider Protocol、UsageTracker、LLMCallLogger |
| 2026-04-20 | `llm-role-routing` | `ProviderRole`（reasoning / judge / chat / embed）+ `registry.get(role)` 分派；TrackedProvider 必帶 `role` |
| 2026-04-21 | `qdrant-lifecycle-bootstrap` | `AsyncQdrantClient` 綁 app state + probe-backed healthz |
| 2026-04-21 | `scanner-skeleton` | Module 1 遍歷 + gitignore + encoding + `ScanResult` |
| 2026-04-21 | `sanitizer-safety-chain` | Pass 1 detect-secrets + PII regex + placeholder；Pass 2 provider pre-flight hook（`sanitizer_pass2_applied` 會翻 true）；`sanitize_audit.jsonl` |
| 2026-04-21 | `scanner-sanitizer-orchestration` | Scanner 入 KB 前過 Pass 1 |
| 2026-04-21 | `module-2-kb-builder-p0` | `KnowledgeBase` / `KBPayload` / token-window chunker + 策略分派 / `KBQdrantBackend` Protocol + `QdrantHttpBackend` |
| 2026-04-22 | `sse-progress-skeleton` | `POST /kb/build` async + `POST /scan?stream=true` + `GET /tasks/{id}/events\|result` + `_run_background_task` 錯誤收斂 |
| 2026-04-23 | `kb-build-production-wiring`（D-032） | `OpenAIEmbeddingProvider`（`text-embedding-3-small` dim 1536）+ `wire_kb_dependencies` factory DI + dim-mismatch guard + `/healthz` `openai_embedding` 三態 |
| 2026-04-23 | `kb-query-endpoint` | `POST /kb/query` 同步查詢 + `kb_query_provider` factory 帶 `default_module="kb_query"` 拆帳 |
| 2026-04-23 | `usage-tracker-dedup` | TrackedProvider 加 `default_module`，變成 `module` 欄唯一寫入路徑；KB 不再手動 `tracker.record(...)` |
| 2026-04-23 | `chat-provider-wiring` | `OpenAIChatProvider`（instructor-wrapped `gpt-4o-mini`）+ `OpenAIContextLengthError`/`OPENAI_CONTEXT_EXCEEDED` + 三個 chat-ish role factory（`llm_reasoning_provider` 0.1 / `llm_judge_provider` 0.0 / `llm_chat_provider` 0.2）+ `/healthz` `openai_chat` 三態；M1 `No outbound LLM traffic during M1` 不變式退役，由 `Outbound LLM traffic gated by TrackedProvider whitelist` 取代 |
| 2026-04-24 | `explorer-react-loop-p0` | Module 4 Explorer ReAct skeleton：`codebus_agent.agent` 子套件（`types` / `protocols` / `explorer` / `judge` / `reasoning_logger` / `prompts`）；`run_explorer` 六步主迴圈（Think→Act→Observe→Judge→Log→Update）+ `_should_stop` 三分支收斂（budget / queue / cancel）+ `_MIN_STATIONS_FOR_CONVERGENCE=3`；`LLMJudge` one-shot + `ReasoningLogger` append-only JSONL（`explorer_prompt_version` / `judge_prompt_version` 寫每行）；`ExplorerTools` / `Judge` / `CoverageChecker` 三個 `@runtime_checkable` Protocol（day-1 抽象 for Q&A 共用）；Coverage 遞迴 hook 以 `_COVERAGE_RECURSION_ENABLED=False` 夾住，由 `coverage-gap-recurse` 後續打開 |

**目前沒有 in-progress change**。下一步依 `docs/implementation-plan.md`：**步驟 17 `explorer-tools-p0`**（`search` / `list_dir` / `read_file` / `mark_station` 四個真工具 + ToolContext + `ensure_in_workspace` 紅線），解鎖 Explorer 跑真 codebase。

## 架構快照

**混合架構**（D-001）：Tauri 2.0 殼（Rust）↔ Python Sidecar（FastAPI）↔ Qdrant（本地向量 DB）。前端 Nuxt 3 + TypeScript + Tailwind。IPC 走 `127.0.0.1:<random-port>` + Bearer token（`docs/sidecar-api.md §一`、`openspec/specs/sidecar-runtime/spec.md`）。

**sidecar 啟動協定**（M1 已實作）：Tauri spawn binary with `--parent-pid <pid>` → sidecar 首行 stdout 印 `{"port":<int>,"bearer":"<≥32 chars>"}` → Tauri 解析後用 bearer 打 `/healthz` → 200 即通電。parent process 消失 5 秒內 sidecar 自殺（watchdog）。

**八大 Module**（`README.md §五`，M2+ 才動工）：
- Module 1 Scanner → Module 2 KB Builder → Module 4 Explorer Agent → Module 5 Generator → Module 7 前端 → Module 8 Q&A Agent
- Module 3（Topic Explorer）Phase 2；Module 6（Intervention）前端實作期決定
- Module 5 輸出多檔（D-029）：`tutorials/{task_id}/tutorial.md`（MOC 索引）+ `stations/s{NN}-slug.md`（每站一檔，含 YAML frontmatter + stable station id；跨檔用標準 markdown link，禁 wikilinks）

**Agent 核心**（D-012）：自寫 ReAct loop + Instructor/Pydantic structured output。Explorer 與 Q&A Agent **共用** ReAct core，靠 `ExplorerTools` / `Judge` / `CoverageChecker` Protocol 抽象（`docs/agent-explorer-spec.md §十二`）。

**LLM 呼叫鏈**（M1 + llm-role-routing + kb-build-production-wiring + chat-provider-wiring 合成）：所有 provider 必須包 `TrackedProvider` 裝飾器——registry 在實例化階段 raise 拒絕 unwrapped provider。允許的 inner class 是一個顯式 allowlist：`TrackedProvider.ALLOWED_INNER_TYPES = {MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}`；要擴增 provider（Ollama / Anthropic）必須在新 change 裡同步改 spec `Outbound LLM traffic gated by TrackedProvider whitelist` 與 code allowlist。測試 suite 用 `respx` mock OpenAI wire。分派機制走 `ProviderRole`（`reasoning` / `judge` / `chat` / `embed`）：呼叫端用 `registry.get(role)` 或 app state 的 `llm_<role>_provider(workspace)` factory 取對應 TrackedProvider。`TrackedProvider` 建構必帶 `role`、`default_module` kwarg，每筆 `token_usage.jsonl` / `llm_calls.jsonl` 記錄都帶 `role` + `module`（後者由 `usage-tracker-dedup` 引入，是 module 欄的唯一寫入路徑）。`llm_calls.jsonl` 含 `sanitizer_pass2_applied` 欄位：sanitizer-safety-chain 後真的 Pass 2 hit 即 true。

**Trust Layer 四站**（Phase A，敘事核心 — 評審會停在這邊）：
- **R-01** Workspace（主畫面 + 六層 audit 面板）
- **O-04** LLM Call Inspector（R-01 內 slide-in panel，秀 wire payload）
- **O-05** Sanitizer Diff（LOCKED/UNLOCKED 稽核畫面）
- **O-01** Grant Modal（workspace 授權）

**三段 Sanitizer**（D-015）：Pass 1 Scanner 入 KB 前（`scanner-sanitizer-orchestration` 落地）→ Pass 2 Provider pre-flight 每次 LLM call 前（`sanitizer-safety-chain` 落地，由 `TrackedProvider` 注入 `SanitizerEngine` + `SanitizerAuditLogger`）→ Pass 3 Q&A `add_to_kb` 寫入前（待 Module 8 Q&A P0）。詳見 `docs/sanitizer.md §三`。

**七層 Audit JSONL**（workspace-level 六層 + App-level 一層）：
- `sanitize_audit.jsonl`（Sanitizer 命中；Pass 1 Scanner + Pass 2 Provider 都會寫，帶 `pass_num` 欄）
- `tool_audit.jsonl`（Sandbox 工具呼叫；Module 4 Explorer 實作時開始填）
- `kb_growth.jsonl`（Q&A `add_to_kb`；Module 8 實作時開始填）
- `reasoning_log.jsonl`（ReAct 每 step；Module 4 實作時開始填）
- `token_usage.jsonl`（D-021 + `usage-tracker-dedup`）：唯一寫入路徑是 `TrackedProvider`，`module` 欄由構造時的 `default_module` 寫入（目前值：`kb_build` / `kb_query` / `reasoning` / `judge` / `chat`）
- `llm_calls.jsonl`（D-022）：完整 wire payload，`sanitizer_pass2_applied` M2 後真會翻 true
- `~/.codebus/authorization_audit.jsonl`（跨 workspace，App-level，見 `docs/authorization.md §五`）

## 關鍵不變式（寫 spec / code 時必守）

1. **雙模 discriminator day 1**（D-002）：`workspace_type: "folder" | "topic"` 欄位從一開始就寫進 schema；MVP 只實作 `folder`，但 `topic` 加進來不能造成 breaking change。`ToolContext`（`sidecar/src/codebus_agent/sandbox.py` + `docs/tool-sandbox.md §三`）、`POST /scan`（`docs/sidecar-api.md §三`）、`authorization_audit`（`docs/authorization.md §五`）都遵守此約。
2. **Sanitizer 單向**：placeholder `<REDACTED:kind#N>` 無 reverse mapping，一旦替換即不可逆；原值「不額外儲存」，原檔在本機原處，不 copy 到 KB/log/網路。
3. **LLM 看到的一定是 Sanitize 過的**：`llm_calls.jsonl` 記的是 post-Sanitizer Pass 2 版本，不還原 pre-sanitize 原文（D-022）。`sanitizer_pass2_applied` 欄位在 sanitizer-safety-chain 後才真會翻 true（M1 舊行為 false）；不變的是此欄位永遠存在、型別不變。
4. **Provider 必包 TrackedProvider + allowlist 同步**：registry guard 在實例化階段攔截 unwrapped provider（`sidecar/src/codebus_agent/providers/`）；而且只有 `TrackedProvider.ALLOWED_INNER_TYPES` 裡列的 inner class 才能包入。目前 allowlist 是 `{MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}`——要加新 live provider 必須同步修 `openspec/specs/llm-provider/spec.md` 的 `Outbound LLM traffic gated by TrackedProvider whitelist` Requirement，spec 列的 allowlist 與 code 列的 ALLOWED_INNER_TYPES 不可分歧。
5. **Bearer + loopback 不可鬆綁**：sidecar 只 bind `127.0.0.1:0`（ephemeral）、bearer token 記憶體常駐不落盤（D-local-2）；任何 endpoint 不得跳過 bearer middleware。
6. **ensure_in_workspace 紅線**：所有檔案操作必須先過 `ensure_in_workspace(path, ctx)`（`Path.resolve(strict=False)` + `is_relative_to`）——覆蓋 `..` 逃逸、symlink、Windows UNC、`\\?\` long-path 全譜系。紅隊 fixture 在 `sidecar/tests/sandbox/`。
7. **檔名 kebab-case**：`docs/*.md`、`design/*.html`、`design/screenshots/*.png` 一律 `{代號}-{語意}`。舊版直接刪，不留 `-v1` 後綴（歷史去 git log 找）。
8. **Spec 為主、mockup 其次**：`design/*.html` 與 `docs/*.md` 衝突時以 spec 為準，回頭修 mockup。
9. **Sanitizer rules 改動必 bump version**：`docs/sanitizer.md` 的 rule pattern 有任何增減，必須同步 bump rules version；`docs/authorization.md §六` 規定使用者同意需依版本重取。不得「靜默改 rule」——會造成既有 workspace 套用新 rule 但未重授權，稽核鏈斷裂。

## 決策記憶 — `docs/decisions.md`

所有非 trivial 的技術取捨都寫成 **D-XXX ADR**（脈絡 / 選項 / 理由 / 後續）。Spec 首行必引相關 D-XXX。改決策時**先改 `decisions.md`，再改引用它的 spec**。常查：
- D-001 混合架構 / D-002 Topic mode 不進 MVP / D-003 LLM Provider 抽象（2026-04-20 role routing 落地 → 2026-04-23 `chat-provider-wiring` 接上 chat-ish live provider，M1 「No outbound」不變式退役）
- D-011 資安 / D-012 自寫 ReAct / D-014 uv toolchain
- D-015 Sanitizer / D-016 Q&A add_to_kb / D-017 ToolSandbox
- D-021 token_usage / D-022 llm_calls（`module` 欄由 `usage-tracker-dedup` 收束成單一寫入路徑）
- D-026 前端 toolchain 改 npm（原本 bun）
- D-027 Qdrant 走 local binary 主路徑（docker compose 降為 fallback）
- D-028 LLM Vision 延後至 Phase 2（介面不預埋 Capability enum）
- D-029 Module 5 多檔輸出（MOC + `stations/s0X-slug.md` + frontmatter + stable station id）+ 拒絕 Obsidian 整合
- D-032 KB build production wiring（`text-embedding-3-small` dim 1536 hard-coded、`CODEBUS_OPENAI_API_KEY` 唯一 env 來源、不 fallback `OPENAI_API_KEY`、SDK retry 不再疊）

## 常用指令

**Python Sidecar**（`sidecar/`，uv toolchain · D-014）
```bash
cd sidecar
uv sync                            # 裝依賴 + 建 venv
uv run pytest                      # 全測（~524 passed + ~17 skipped；Qdrant / symlink 相關環境相依會自動 skip）
uv run pytest tests/sandbox/       # 紅隊 path-escape 專測
uv run pytest tests/providers/     # Mock / Tracked / OpenAIEmbedding / OpenAIChat / UsageTracker / LLMCallLogger
uv run pytest tests/qdrant/ -v     # smoke test（需 Qdrant 起來才會跑）
uv run pytest tests/test_wire_kb_dependencies.py  # DI hook 八 slot + 兩 healthz probe
uv run pytest -k healthz           # 按關鍵字挑單測
uv run python -m codebus_agent.api.main           # 獨立起 sidecar（讀 port/bearer 看 stdout）
uv run python -m codebus_agent.api.main --healthz # 自檢模式，印 JSON 不起 HTTP
# Real-endpoint smoke test — loads CODEBUS_OPENAI_API_KEY from repo-root .env 但不 echo 到 stdout
uv run python scripts/smoke_chat_provider.py      # 驗 /healthz openai_chat + 一次真實 chat call + token_usage 拆帳
```

**Qdrant 本地 binary**（D-027）
```bash
# 先把 qdrant binary 放到 ~/.codebus/bin/qdrant(.exe)，或設 $CODEBUS_QDRANT_BIN
bash sidecar/scripts/start-qdrant.sh      # POSIX
pwsh sidecar/scripts/start-qdrant.ps1     # Windows
# 存放路徑走 env var（Qdrant 1.x 無 --storage-path flag）：
#   QDRANT__STORAGE__STORAGE_PATH / QDRANT__STORAGE__SNAPSHOTS_PATH
# Fallback：docker compose -f sidecar/docker-compose.qdrant.yml up -d
```

**前端**（`web/`，npm — D-026）
```bash
cd web
npm install
npm run dev          # http://localhost:3000（cargo tauri dev 也會自動跑這個）
npm run typecheck
npm run generate     # 出 SPA 到 .output/public，給 cargo tauri build 吃
```

**Tauri 殼**（`tauri/src-tauri/`，Rust stable ≥ 1.80）
```bash
cd tauri/src-tauri
cargo tauri dev      # 自動 spawn web + sidecar（透過 externalBin）
cargo test
cargo tauri build    # 產 MSI + NSIS（Windows）/ AppImage / dmg；依賴 sidecar/dist/codebus-sidecar-<triple>(.exe)
cargo build --release -- ...  # 只編 codebus.exe，不重打 installer
```

**PyInstaller 打包鏈**（必須先產 sidecar binary 才能 cargo tauri build）
```bash
cd sidecar
uv run pyinstaller codebus-sidecar.spec
# 產出到 sidecar/dist/codebus-sidecar-<triple>(.exe)，被 tauri.conf.json externalBin 引用
```

**Commit gate**
```bash
uv tool install pre-commit    # 首次 setup
pre-commit install            # 裝 git 原生 hook
pre-commit run --all-files    # 全檔跑 stage-0 hook（trailing-ws / eof / check-yaml / check-json / mixed-line-ending）
bash tests/precommit_gate_test.sh          # 乾淨 repo 應全綠
bash tests/precommit_violation_test.sh     # 負測：故意違規 commit 應被擋
```

## Spectra worktree 慣例

用 `/spectra-apply <change>` 起手時，skill 會在 `.spectra/worktrees/<change>/` 開 git worktree。收尾後：
```bash
git merge --ff-only change/<name>
git worktree remove .spectra/worktrees/<name>   # 若殘留目錄就加 --force
git branch -d change/<name>
```
`.spectra/` 已在 `.gitignore`，worktree 不會汙染主 repo。

## 常見引用關係

改 spec 時容易漏掉的連動（`docs/README.md §五` 完整對照）：
- 改 `sanitizer.md` → 檢查 `authorization.md §六`（rules version bump 政策）、`sidecar-api.md` audit endpoints、`security.md` §3.x
- 改 `authorization.md` → 檢查 `sidecar-api.md` POST /scan schema（`workspace_type`）、`tool-sandbox.md §三` ToolContext、`design/o-01-grant-modal.html`
- 改 `agent-core.md` → 檢查 `agent-explorer-spec.md` §十二 trait、`qa-agent.md` §二、`prompts.md`
- 改 Module spec → 檢查 `implementation-plan.md` 依賴圖 + `sidecar-api.md` 對應 endpoint
- 改 M1 已封存的 capability（`openspec/specs/<cap>/spec.md`）→ 必須走 `/spectra-propose` 開新 change；不可直接改 archive 過的 spec
