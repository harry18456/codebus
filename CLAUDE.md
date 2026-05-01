<!-- SPECTRA:START v1.0.2 -->

# Spectra Instructions

This project uses Spectra for Spec-Driven Development(SDD). Specs live in `openspec/specs/`, change proposals in `openspec/changes/`.

## Use `/spectra-*` skills when:

- A discussion needs structure before coding → `/spectra-discuss`
- User wants to plan, propose, or design a change → `/spectra-propose`
- Tasks are ready to implement → `/spectra-apply`
- There's an in-progress change to continue → `/spectra-ingest`
- User asks about specs or how something works → `/spectra-ask`
- Implementation is done → `/spectra-archive`
- Commit only files related to a specific change → `/spectra-commit`

## Workflow

discuss? → propose → apply ⇄ ingest → archive

- `discuss` is optional — skip if requirements are clear
- Requirements change mid-work? Plan mode → `ingest` → resume `apply`

## Parked Changes

Changes can be parked（暫存）— temporarily moved out of `openspec/changes/`. Parked changes won't appear in `spectra list` but can be found with `spectra list --parked`. To restore: `spectra unpark <name>`. The `/spectra-apply` and `/spectra-ingest` skills handle parked changes automatically.

<!-- SPECTRA:END -->

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 溝通語言

使用者偏好 **繁體中文（zh-TW）** 回覆。Spec 內文、commit message、code comment 的 prose 也是 zh-TW；schema / 識別字 / filename / test name 維持英文。

## 專案概要

CodeBus 是「把陌生 codebase 一鍵變成可走訪的 tutorial」的桌面 App，由 Tauri 殼（Rust）+ Python Sidecar（FastAPI）+ Qdrant（本地向量 DB）混合架構（D-001）組成。M1 通電後資料層 + Module 4 Explorer + Module 5 Generator + Module 8 Q&A 已通電；**Phase 6 前端**（Trust Layer 四站 R-01 / O-01 / O-04 / O-05）30 step + step 31 D-033 B 全 ✅ landed（archive 2026-04-27 ~ 2026-05-01），**當前進入 Phase 7（打磨與 Demo 準備，~5d）**：prompt 品質迭代 + 跨平台測試 + 打包 smoke（PyInstaller + cargo tauri build 端到端）+ demo 素材腳本。詳見 `docs/implementation-plan.md §二第七階段` 與 `README.md §九 第五階段`。**D-033 Change A** 已於 2026-04-29 archive（純後端拆三介面 + PII Provider；spec 套用至 `openspec/specs/{llm-provider,sanitizer,usage-tracking,pii-provider}/`）。**D-033 Change B** 已於 2026-05-01 archive（`provider-settings-and-onboarding`：Setting page + Onboarding wizard + Tauri keyring IPC + sidecar `/internal/startup-config` + `RegistryHolder` hot-swap + `/healthz.dependency` 三 lane + Nuxt onboarding redirect middleware + LLM Call Inspector PII filter）— **manual 驗證留 12.4 onboarding wizard e2e + 12.5 macOS/Linux keyring 實機**（archive 後手動 TODO，待實機環境）。**O-05 P1+ follow-up `sanitizer-audit-unlock`**：3-pane raw diff / unlock-with-grant flow / auto-relock / raw retention / `audit_session_id` chain 整套 defer，啟動前須先做 ADR 評估 raw retention threat model（守 D-015）。**Q&A P1+ follow-ups**：cross-session memory / citation file:line click → side panel / `add_to_kb` rollback button + KB ops UI / drawer resize 多 instance 整套 defer 至 Phase 2（per `docs/qa-agent.md §十`）。

**查 archive 進度**：`ls openspec/changes/archive/` 是時間軸索引；每個 archive 目錄內含 `proposal.md` + `tasks.md` + `design.md` + `specs/` delta 與當時的決策脈絡，不需要在 CLAUDE.md 重複維護。

## 子系統

- `sidecar/` — uv-managed Python 3.12 FastAPI sidecar；ToolSandbox + Sanitizer + LLM Provider + Qdrant + Modules 1/2/4/5/8 + SSE task skeleton + auth subpackage + PyInstaller spec
- `tauri/src-tauri/` — Rust host；`sidecar_ping` / `sidecar_handshake` IPC commands + `tutorial::{read_tutorial_file, write_progress_file, list_tutorial_tasks}` 三個檔案 IPC（共用 `validate_path` helper、紅隊 14 case 在 `tests/path_safety.rs`）；`SidecarState` Mutex cache
- `web/` — Nuxt 4 + Tailwind + TypeScript（npm，D-026）；`tailwind.config.ts` 從 `design/v1/tokens.css` port；composables: `useSidecar` / `useSseTask`（sidecar HTTP）+ `useTutorialFiles` / `useStationRoute` / `useTutorialProgress`（tutorial 檔案 IPC，後者是 progress.json 唯一寫入路徑）+ `useExplorerStream`（agent-console-p0 SSE bucket-fill state）+ `useAuditJsonl`（llm-call-inspector-p0 七層 audit JSONL reader + live-tail，依 `audit_kind` enum 載 `<ws>/.codebus/<file>.jsonl`；`liveTailFromExplorerStream` 對 `kind === 'llm'`、`liveTailFromQaSession` 對 `kind === 'kb_growth'`，dedup key 分別是 `request_id` / `entry_id`）+ `useSanitizeAudit`（sanitizer-audit-inspector-p0 thin wrapper over `useAuditJsonl('sanitize')`，augments rows with `sourceView` / `placeholderToken` / `passLabel` + 反應式 `kindSummary` / `sessionTimeline`；`PASS_LABELS` 是 pass 1/2/3 → 人類可讀 label 的單一常數）+ `useSanitizerRules`（registry snapshot 一次性 fetch via `useSidecar().fetch('/sanitizer/rules')`，module-level cache 守 sidecar process 生命週期）+ `useQaSession`（qa-overlay-p0 module-level singleton；state 寫在 module scope，每個 caller 拿到同一份 ref；`start(prompt, originatingStationId)` 流程：POST /qa → 解 task_id → useSseTask → 5 種 SSE event 分派到 turn-shaped state；turns FIFO cap 50；私有 `__sseEvents` 聚合流暴露給 `useAuditJsonl('kb_growth')` 訂閱）；`components/content/{Checkpoint,Quiz,QAEntry}.vue` mdc 自動掛載（`<QAEntry>` imperative 呼 `useQaSession().start`，由 R-01 station page `provide('currentStationId', stationId)` 注入）+ `components/tutorial/{StationLayout,StationNav,StationContent,MOCIndex}.vue` page-level shell + `components/console/{ConsoleTimeline,StepCard,ProgressStrip,CoverageBanner}.vue` Explorer console + `components/audit/{AuditPanel,LlmCallInspector,SanitizerAuditInspector}.vue` audit views + `components/audit/sanitizerAuditBanner.ts`（D-015 banner 字面唯一 source；source-grep 鎖死「`raw values are not retained per D-015`」字串只在這支檔案出現）+ `components/qa/{QAOverlay,QaTurnCard,QaCitations}.vue` Q&A drawer overlay；`pages/tutorial/[workspace_id]/{index,[station_id]}.vue` 兩條 R-01 路由 + `pages/explorer/[task_id].vue` Explorer console + `pages/audit/{llm,sanitizer}.vue` 兩個 standalone audit reviewer page；`layouts/default.vue` 在 layout 樹底 mount `<QAOverlay />` 並監聽全域 `Cmd+K`（召喚 drawer，已開時 no-op）+ `Escape`（關 drawer）keydown
- `openspec/specs/` — 18+ 個 capability spec（單一事實來源；M1 後不可直接改 archive 過的 spec，要走 `/spectra-propose`）
- `docs/` — Module / Agent / 橫切層 spec + `decisions.md` ADR + `implementation-plan.md` 動工順序
- `design/v1/` — Phase 6 14 mockup + 共用骨架 tokens.css/shell.css/shell.js（前端動工原件）
- `tests/golden/` — `demo-synthetic/` + `timeline-storage-adapter-synthetic/`（Explorer 評估 fixture）

## 架構快照

**Sidecar 啟動協定**（M1）：Tauri spawn binary `--parent-pid <pid>` → sidecar 首行 stdout 印 `{"port":<int>,"bearer":"<≥32 chars>"}` → Tauri 解析後打 `/healthz` → 200 通電。Parent 消失 5 秒內 sidecar 自殺（watchdog）。

**八大 Module**（`README.md §五`、`docs/implementation-plan.md`）：Module 1 Scanner → Module 2 KB Builder → Module 4 Explorer → Module 5 Generator → Module 7 前端 → Module 8 Q&A。Module 3（Topic Explorer）Phase 2；Module 6（Intervention）前端實作期決定。Module 5 輸出 `<ws>/codebus-tutorials/{task_id}/tutorial.md` MOC + `stations/s{NN}-slug.md` + `route.json`（D-029）。

**Agent 核心**（D-012）：自寫 ReAct loop + Instructor/Pydantic structured output。Explorer 與 Q&A **共用** ReAct core，靠 `ExplorerTools` / `Judge` / `CoverageChecker` Protocol 抽象（`docs/agent-explorer-spec.md §十二`）。

**LLM 呼叫鏈**：所有 provider 必須包 `TrackedProvider` — registry 在實例化階段 raise 拒絕 unwrapped；inner class 走 `TrackedProvider.ALLOWED_INNER_TYPES = {MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}` 顯式 allowlist（spec 與 code 同步）。分派走 `ProviderRole`（reasoning / judge / chat / embed），每筆 audit 記 `role` + `module`（後者由 `default_module` kwarg 寫入，是 `module` 欄唯一寫入路徑）。

**三段 Sanitizer**（D-015，`docs/sanitizer.md §三`）：Pass 1 Scanner 入 KB 前 → Pass 2 Provider pre-flight 每次 LLM call 前 → Pass 3 Q&A `add_to_kb` 寫入前。Placeholder `<REDACTED:kind#N>` 無 reverse mapping，單向不可逆。**D-033 後**：SanitizerEngine 消費 `PIIProvider` Protocol（預設 `RuleBasedPIIProvider` 包既有 rules，未來可注入 `LocalLLMPIIProvider` / `OpenAIPIIDetectionProvider`）；Pass 2 在 LLM/Embedding inner 仍強制執行，PIIProvider inner（藉 `TrackedProvider.PII_ALLOWED_INNER_TYPES` marker）**自動 bypass** Pass 2 — D-015 invariant 的唯一例外，外部無 flag 可開。`sanitize` 已改 `async`，三段呼叫端皆 `await`。

**Trust Layer 四站**（Phase 6，敘事核心）：R-01 Workspace（六層 audit 面板）/ O-04 LLM Call Inspector / O-05 Sanitizer Diff / O-01 Grant Modal。

**Setting / Onboarding 啟動流程**（D-033 B archive 2026-05-01）：

- **Keyring trust boundary**：API key 走 OS keychain（`keyring` crate 3.x 直連 macOS Keychain / Windows Credential Manager / GNOME Keyring），三 IPC commands `keyring_set` / `keyring_get` / `keyring_delete`；`provider_id` host-side regex `^[a-z][a-z0-9-]{2,40}$`。Tauri spawn sidecar 後從 keyring 撈 keys → `POST /internal/startup-config`（bearer 守、`include_in_schema=False`、idempotent 一次性、不寫磁碟、不入 audit）。
- **`/healthz.dependency` 三 lane**：`llm_chat` / `llm_embed` / `pii` 各自報 `ready` / `not-configured` / `unreachable`；冷啟動沒 startup-config → `llm_chat` + `llm_embed` 都是 `not-configured`，`pii` 在 mode=`rule` 下永遠 `ready`。Pass-through infra lane（`qdrant`）也用同一三值字典寫進 `dependency`。
- **Middleware redirect**：`web/app/middleware/onboarding-redirect.global.ts` Nuxt route middleware 對每次導航打 `/healthz`，任一 LLM lane `not-configured` → `navigateTo('/onboarding/welcome')`；exclude `/onboarding/*`。`pages/index.vue` 也 mount 時做同樣 check（`router.replace`）守雙保險。
- **Onboarding wizard 三步**：`/onboarding/welcome`（純文案，Next 永遠 enabled）→ `/onboarding/providers`（chat + embed 兩表單，Next disabled until both filled）→ `/onboarding/done`（Start CTA → `/`）。Submit 順序：keyring_set chat → keyring_set embed → upsertProvider chat → upsertProvider embed → setBinding × 4。任一 keyring 失敗中斷不繼續。
- **Setting page 三 section**：`/settings` 路由，`<ProviderPoolList>`（CRUD）+ `<RoleBindingTable>`（4 role dropdown，embed 改 binding 走 `<EmbeddingChangeConfirmModal>` destructive confirm）+ `<PiiModeToggle>`（rule / llm radio，llm 在 P0 disable）。Provider snapshot via `useProviderConfig()` module-level singleton；mutation endpoints 觸發後 sidecar emit `provider_config_changed` SSE event 走 `GET /events?channel=app`，composable 訂閱後 100ms debounce re-fetch。
- **Hot-swap**：`RegistryHolder` 雙層引用（內層 immutable / 外層 atomic swap via `asyncio.Lock`）；`PUT /settings/bindings` → `holder.swap(new_registry)`；in-flight task 持 reference 跑完現場、下個 task 用新 binding。Embed 切換是唯一例外（destructive，綁 KB rebuild）。
- **PII filter（O-04）**：`<LlmCallInspector>` + `<AuditPanel>` 加 `hidePiiDetection` prop（default `true`），llm tab + inspector prev/next 過濾 `role: "pii_detection"`；toggle banner emit `toggle-pii-visible` 讓 page 層 flip。「PII LLM call 仍要 audit」（D-033 不變式 3）落地在共用 `llm_calls.jsonl` + `role: "pii_detection"` 欄位。

## 七層 Audit JSONL

Workspace-level 六層全在 `<ws>/.codebus/`；App-level 一層在 `~/.codebus/`。`.gitignore` 加 `.codebus/` 一行即可全排除。

| 檔名 | 唯一 Writer | 由哪個 Module 寫 |
|---|---|---|
| `sanitize_audit.jsonl` | `SanitizerAuditLogger` | Pass 1 / 2 / 3（帶 `pass` 欄；JSONL key 是 `pass` 不是 `pass_num`）|
| `tool_audit.jsonl` | `sandbox.append_tool_audit_line` | Explorer / FolderTools |
| `kb_growth.jsonl` | `KBGrowthLogger` | Q&A `add_to_kb`（`event_type` P0 永遠 `"add"`）|
| `reasoning_log.jsonl` | `ReasoningLogger` | Explorer / Q&A 每 step（caller-side mkdir）|
| `token_usage.jsonl` | `TrackedProvider` | 8 lane：`kb_build` / `kb_query` / `reasoning` / `judge` / `chat` / `coverage` / `generate` / `qa_agent` |
| `llm_calls.jsonl` | `LLMCallLogger` | TrackedProvider 內；含 `sanitizer_pass2_applied` 欄 |
| `generator_log.jsonl` | Module 5 Generator | Per-Module operational log（與 reasoning_log 同層）|
| `~/.codebus/authorization_audit.jsonl` | `AuthorizationAuditLogger` | App-level 跨 workspace；三事件 `grant_issued` / `grant_denied` / `grant_revoked` |

**Path constants 集中**：`sidecar/src/codebus_agent/_audit_paths.py` 收 `_WORKSPACE_AUDIT_SUBDIR=".codebus"` + 7 個 `_<NAME>_FILENAME`；`api/_audit_paths.py` 是 backward-compat shim；`auth/paths.py` 是 App-level sister leaf。`tests/test_no_jsonl_literal_drift.py` 用 source-grep 鎖死字面量只能在 canonical leaf。

**同 single-source pattern**：`agent/station_id.py` 是 station id regex 唯一 owner；`agent/qa.py` 是 5 個 `_QA_(MAX|DEDUP)_*` 常數唯一 owner；`sanitizer/config.py::RULES_VERSION` 是 sanitizer rules version 唯一常數。各有 defensive test 用 `is`-identity + source-grep 鎖死。

## 關鍵不變式（寫 spec / code 必守）

1. **雙模 discriminator day 1**（D-002）：`workspace_type: "folder" | "topic"` 從 schema 第一天就在；MVP 只實作 `folder`，加 `topic` 不能 breaking。`ToolContext` / `POST /scan` / `authorization_audit` 都遵守。
2. **Sanitizer 單向不可逆**：placeholder 無 reverse mapping、原值不額外儲存。
3. **LLM 看到的一定是 Sanitize 過的**：`llm_calls.jsonl` 記 post-Pass 2 版本，不還原 pre-sanitize（D-022）。
4. **Provider 必包 TrackedProvider + allowlist 同步**：要新增 live provider，必須同步改 `openspec/specs/llm-provider/spec.md` 的 `Outbound LLM traffic gated by TrackedProvider whitelist` Requirement 與 code 的 `ALLOWED_INNER_TYPES`，兩處不可分歧。
5. **Bearer + loopback 不可鬆綁**：sidecar 只 bind `127.0.0.1:0` ephemeral；bearer token 記憶體常駐不落盤；不得跳過 bearer middleware。
6. **`ensure_in_workspace` 紅線**：所有檔案操作先過 `ensure_in_workspace(path, ctx)` — 覆蓋 `..` 逃逸 / symlink / Windows UNC / `\\?\` long-path 全譜系。紅隊 fixture 在 `sidecar/tests/sandbox/`。
7. **檔名 kebab-case**：`docs/*.md`、`design/*.html`、`design/screenshots/*.png` 一律 `{代號}-{語意}`，不留 `-v1` 後綴（歷史去 git log 找）。
8. **Spec 為主、mockup 其次**：`design/*.html` 與 `docs/*.md` 衝突時以 spec 為準，回頭修 mockup。
9. **Sanitizer rules 改動必 bump version**：`docs/sanitizer.md` rule 增減須同步 bump rules version；`docs/authorization.md §六` 規定使用者依版本重取同意。

## 常用指令

**Python Sidecar**（`sidecar/`，uv toolchain · D-014）
```bash
cd sidecar
uv sync
uv run pytest                                # 全測（baseline ~885 passed / 19 skipped）
uv run pytest tests/sandbox/                 # 紅隊 path-escape
uv run pytest tests/providers/               # Mock / Tracked / OpenAI* / UsageTracker / LLMCallLogger
uv run pytest tests/qdrant/ -v               # smoke（需 Qdrant 起來）
uv run pytest -k healthz                     # 按關鍵字挑單測
uv run python -m codebus_agent.api.main      # 獨立起 sidecar（讀 port/bearer 看 stdout）
uv run python -m codebus_agent.api.main --healthz  # 自檢模式，印 JSON 不起 HTTP
uv run python scripts/smoke_chat_provider.py # 真實 chat call smoke（讀 repo-root .env）
```

**Qdrant 本地 binary**（D-027）
```bash
# 把 qdrant binary 放到 ~/.codebus/bin/qdrant(.exe) 或設 $CODEBUS_QDRANT_BIN
bash sidecar/scripts/start-qdrant.sh         # POSIX
pwsh sidecar/scripts/start-qdrant.ps1        # Windows
# Qdrant 1.x 無 --storage-path flag，走 env：QDRANT__STORAGE__STORAGE_PATH / QDRANT__STORAGE__SNAPSHOTS_PATH
# Fallback：docker compose -f sidecar/docker-compose.qdrant.yml up -d
```

**前端**（`web/`，npm — D-026）
```bash
cd web
npm install
npm run dev          # http://localhost:3000（cargo tauri dev 會自動啟動）
npm run typecheck
npm run generate     # 出 SPA 到 .output/public，給 cargo tauri build 吃
```

**Tauri 殼**（`tauri/src-tauri/`，Rust ≥ 1.80）
```bash
cd tauri/src-tauri
cargo tauri dev      # 自動 spawn web + sidecar（透過 externalBin）
cargo test
cargo tauri build    # 產 MSI/NSIS/AppImage/dmg；依賴 sidecar/dist/codebus-sidecar-<triple>(.exe)
```

**PyInstaller 打包鏈**（必須先產 sidecar binary 才能 cargo tauri build）
```bash
cd sidecar
uv run pyinstaller codebus-sidecar.spec      # → sidecar/dist/codebus-sidecar-<triple>(.exe)
```

**Commit gate**
```bash
uv tool install pre-commit                   # 首次 setup
pre-commit install                           # 裝 git 原生 hook
pre-commit run --all-files                   # 全檔跑 stage-0
bash tests/precommit_gate_test.sh            # 乾淨 repo 應全綠
bash tests/precommit_violation_test.sh       # 負測：違規 commit 應被擋
```

## Spectra worktree 慣例

`/spectra-apply <change>` 會在 `.spectra/worktrees/<change>/` 開 git worktree。收尾後：
```bash
git merge --ff-only change/<name>
git worktree remove .spectra/worktrees/<name>   # 殘留就加 --force
git branch -d change/<name>
```
`.spectra/` 已在 `.gitignore`。

## 決策記憶

所有非 trivial 取捨都在 `docs/decisions.md`，以 **D-XXX ADR**（脈絡 / 選項 / 理由 / 後續）形式維護。Spec 首行必引相關 D-XXX。改決策時**先改 `decisions.md`，再改引用它的 spec**。最常觸碰：D-001（混合架構）/ D-002（雙模 discriminator）/ D-012（自寫 ReAct）/ D-015（三段 Sanitizer）/ D-016（Q&A add_to_kb）/ D-017（ToolSandbox）/ D-021 D-022（token_usage / llm_calls 雙線）/ D-026（前端 npm）/ D-027（Qdrant local binary）/ D-029（Module 5 多檔輸出）/ D-032（KB build production wiring）。

## 引用關係（改 spec 易漏的連動）

完整對照在 `docs/README.md §五`。常見：
- 改 `sanitizer.md` → 檢查 `authorization.md §六`（rules version bump）/ `sidecar-api.md` audit endpoints / `security.md`
- 改 `authorization.md` → 檢查 `sidecar-api.md` POST /scan schema（`workspace_type`）/ `tool-sandbox.md §三` ToolContext / `design/o-01-grant-modal.html`
- 改 `agent-core.md` → 檢查 `agent-explorer-spec.md §十二` trait / `qa-agent.md §二` / `prompts.md`
- 改 Module spec → 檢查 `implementation-plan.md` 依賴圖 + `sidecar-api.md` 對應 endpoint
- 改 M1 已封存 capability（`openspec/specs/<cap>/spec.md`）→ 必須走 `/spectra-propose` 開新 change，不可直接改 archive 過的 spec
