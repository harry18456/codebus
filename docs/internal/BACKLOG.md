# Backlog

> **這是 codebus 唯一的待辦權威清單 —— 要決定「接下來做什麼」只看這份。**

本檔分三段：**開放項目**（真正還沒做、按主題分組，每條附「起點」可直接定位要改哪）、**已完成 / Archived**（曾是開放項目、後來做掉的，留對應 change 當脈絡）、**已決定不做**（評估後放棄、留決策理由）。詳述見各自的 `docs/internal/<date>-<slug>-backlog.md`。新發現的 design smell / UX 缺陷 / feature gap 先記成開放項目，之後再決定要不要 `/spectra-propose` 起 change。

> **最後校正：2026-06-26** —— 對 19 條開放 + 3 個 session todo 逐條 grep 真實程式碼 review，確認全部仍真開放（無 stale）、新增 TOOL-1（spectra archive `@trace` 污染）。（2026-06-16：Tier 0 三條完成移入已完成段。2026-06-14：逐條 grep 驗證、移除 check-read / Windows 打包、標註部分完成項。）
> 嚴重度／工程量為相對估值；組內由上而下大致為建議優先序。

---

## 開放項目

### 🔒 安全

- **SEC-2 · Codex 端 hard read + 命令/工具隔離** — 嚴重度：高（Windows confirmed）· 工程量：重（待研究）
  - **問題**：codex `workspace-write` 設計上可讀 workspace 外任意檔（含 `~/.ssh`/`~/.aws`），只靠 `-s` OS sandbox + AGENTS.md soft constraint，**無硬性 read enforcement**。2026-05-28 Windows PoC 確認 threat C OPEN；macOS/Linux 未測。PII raw_sync mirror 框住實際嚴重度（agent 讀 redacted 鏡像、非 live repo）。
  - **起點**：`agent/codex_backend.rs:146-169`（isolation flags）、`:187-192`（sandbox flag `-s` / `-c sandbox_mode`）；全 `src` grep `AppContainer|icacls|LowBox|separate-user` 0 命中。
  - **方案**（待研究）：separate-user / ACL deny / AppContainer / container；+ macOS/Linux 等價 PoC。AppContainer 留升級路徑。
  - 詳細：[Windows PoC](2026-05-28-codex-windows-sandbox-read-poc.md) + [§10 discussion](2026-05-23-bash-hook-and-codex-sandbox-discussion.md) + [hard-gate spike](2026-05-28-codex-hook-hard-gate-spike.md) + [cli-applicability](2026-05-30-cli-capability-applicability-backlog.md)

### 🎯 正確性 / telemetry

- **COR-1 · RunId source-of-truth 統一（剩 quiz/chat）** — 邊緣正確性 · 工程量：輕-中
  - **已完成**：goal 路徑（最嚴重的「完成後卡載入」regression）已根治——IPC 取樣一次、以 slug 回傳前端、colon 形式下傳 `run_goal`。change `goal-run-id-unify-stuck-rundetail`（2026-06-07 archived）；`verb/goal.rs:139-159` 已接 caller-provided started-at。
  - **剩**：quiz（`verb/quiz.rs:583` + `ipc/quiz.rs:115`）、chat（`verb/chat.rs:128` + `ipc/chats.rs:262`）仍「IPC 一次 + verb 一次」各自 `Utc::now()`、未下傳；影響上限是 orphan/interrupted 標籤（無 goal 那種卡載入症狀、嚴重度較低）。
  - 詳細：[runid-source-of-truth](2026-05-28-runid-source-of-truth-todo.md)

- **COR-2 · claude-code-config spec 殘留 stale `codebus-azure`** — spec↔code drift · 工程量：輕（先 ground 雙層 default）
  - **問題**：spec 兩個 requirement（Endpoint Profile Schema + OS Keyring Integration）仍寫 `codebus-azure`，CLI resolver 實際 default = `codebus-claude-azure`。額外發現：core loader 對 claude azure `keyring_service` 是**必填、無 default**，codex loader 卻有 `codebus-codex-azure` default → 兩層 default 不一致，可能本身是 bug。
  - **起點**：spec `claude-code-config/spec.md:11,246`（寫 `codebus-azure`）；CLI `codebus-cli/src/commands/config.rs:24`（`codebus-claude-azure`）；core `config/endpoint.rs:400`（claude 必填無 default）vs `config/codex.rs:157`（codex 有 default）。
  - **方案**：先 ground core loader vs CLI resolver 雙層 default（決定是否本身是 bug）再對齊 spec（+ starter 範例、test yaml）。
  - 源自 `windows-uninstaller-opt-in-purge` review（無 detail doc）。

### 🚀 能力 / capability

- **CAP-1 · goal 引入動態 subagent 委派（Task 工具）** — capability enhancement · 工程量：中
  - **現況**：`verb/goal.rs:58` `GOAL_TOOLSET = [Read, Glob, Grep, Write, Edit]` **不含 Task**，無 agents 目錄、agent 無法自主開 subagent。機制 claude-only（codex 有內建 `spawn_agent` 不受 `--tools` 約束、安全需重驗）。
  - **方案**：先 ground-truth 測 + 最小實驗版。
  - 詳細：[goal-subagent-delegation](2026-05-21-goal-subagent-delegation-backlog.md)

- **CAP-2 · provider-specific prompt engineering（剩 C2 parser 保真度）** — 輸出品質 · 待研究
  - **已完成**：C1 skill 觸發機制 native 化——codex 已用 `$codebus-<bundle>` native explicit-invocation（省 ~24.8% input token）、非 `/skill` description-match；SpawnSpec 已重構成 verb+sub_mode+input。隨 `prompt-surface-layer-3-spawnspec-restructure`（2026-05-24 archived）落地。`agent/codex_backend.rs:71-72`。
  - **剩 C2**：codex parser 保真度——`stream/codex_parser.rs:51` 仍寫死 `name:"Shell"`、`tool_kind` 永遠 None（codex wire 不送）、無 reasoning/格式保真度擴充。卡 ground-truth 樣本。
  - 詳細：[provider-prompt-engineering](2026-05-22-provider-prompt-engineering-backlog.md)、loop PE1/PE2（`2026-05-22-provider-prompt-diagnosis.md` / `-design.md`）

- **CAP-3 · codebus 作為 MCP Server（query-only）** — 擴充性 / 生態整合 · 工程量：中-重
  - **現況**：CLI 無 `mcp` 子命令；所有 mcp 命中都是 client 隔離（`--strict-mcp-config`），無「自身作為 server 對外暴露 wiki query 工具」。原設計 after F+RAG，但有 incremental MVP 路線（先做三件唯讀 wiki 工具、不必全卡）。
  - 詳細：[mcp-server](2026-05-14-mcp-server-backlog.md)

- **CAP-4 · 新能力候選（deferred，有具體需求再接）** — new-capability · 工程量：中-重（各別 spike 後定）
  - (a) structured output `--json-schema`/`--output-schema` 跨 provider 對等；(b) codex `--oss` 本地模型 profile；(c) 泛化既有 `content_verify` 成多 spawn orchestrator（現 `verb/content_verify.rs:172` 只被 `goal.rs:557`/`quiz.rs:773` 兩處固定呼叫）。不投機抽象。
  - 詳細：[cli-applicability T3](2026-05-30-cli-capability-applicability-backlog.md)

- **CAP-5 · OpenAI Privacy Filter 整合（local 語意層 PII）** — PII 保護強化 · 工程量：重
  - **現況**：`pii/scanners/mod.rs:8-10` 明文「Presidio/Comprehend 等 deferred」；現只有 regex 4-pattern scanner，無語意層。與 SEC-4 互補。
  - 詳細：[openai-privacy-filter](2026-05-14-openai-privacy-filter-backlog.md)

- **CAP-6 · RAG index + search（LanceDB，after F）** — 知識檢索品質 · 工程量：重（1 週以上）
  - **現況**：Cargo.toml 零 LanceDB/embedding/ONNX 依賴、無實作。ONNX runtime 與 MCP 唯讀工具可共用基礎設施；注入路徑要 provider-neutral。
  - 詳細：[rag-index-search](2026-05-14-rag-index-search-backlog.md)

### 🎨 UX

- **UX-1 · Settings 可編輯 chat verb 的 model/effort（方案 B）** — UX gap · 工程量：1-2 半天
  - **已完成**：方案 A read-only hint——`EndpointSection.tsx:242-255` `endpoint-chat-row` 顯示「chat 沿用 query（model/effort）」，解掉透明度問題。
  - **剩方案 B**：讓 chat 可獨立編輯 model/effort（chat 不在可編輯 `VERBS=[goal,query,fix,verify]`）。刻意延後至 v2 multi-provider 或 user 反映 chat 需獨立 model 時再做。
  - 詳細：[settings-chat-model](2026-05-14-settings-chat-model-backlog.md)

- **UX-2 · App Activity Stream 對齊 CLI verbose（完整 AI 回覆細節）** — UX 補強 · 工程量：輕-中
  - **現況**：app `ActivityStreamItem.tsx:24-30` 只渲染精簡 cluster（ToolResult/Usage NOT rendered、tool input 縮短）。CLI 端**已有** verbose（`render/stream_event.rs:55,70,81`，來自 `cli-debug-stream-detail`），app 未對齊。與 CAP-2 C2 有順序耦合（codex 編輯無 event 可展開）。
  - 詳細：[app-stream-verbose-detail](2026-05-21-app-stream-verbose-detail-backlog.md)

- **UX-3 · 全域 font-scale / accessibility text size** — accessibility gap · 工程量：中（2-3 個半天）
  - **現況**：`store/settings.ts` 設定 key 無 fontScale/zoom 任何欄位；CSS 是固定 token utility，無使用者可調縮放。
  - 詳細：[app-font-scale](2026-05-14-app-font-scale-backlog.md)

- **UX-4 · UI 無障礙（對比度 + 鍵盤導航）** — accessibility gap · 工程量：中（2-3 個半天）
  - **現況**：191 處 aria/focus 散落 51 檔屬零星（多來自 Radix 預設）；`prefers-contrast`/`focus-trap` grep 0 命中，無系統性高對比模式 / tab-order / skip-link。
  - 詳細：[ui-accessibility](2026-05-14-ui-accessibility-backlog.md)

- **UX-5 · CLI `[[slug]]` 可點連結 + 連結目標 + CLI chat markdown polish** — regression 補回 + capability + UX · 工程量：重
  - **現況**：CLI chat 輸出 raw `println!`（`commands/chat.rs:195-198`），`[[slug]]` 不可點、無 GFM 表格 / markdown 樣式；CLI 唯一 OSC 8 在 lint 輸出（`render/lint_text.rs`），不涵蓋 chat。**注意**：app 端 `markdown-rendering-fidelity`（已完成）是 GUI surface，與本 CLI 條不同、別誤判已完成。`codebus://` 協定吃掉大半工。
  - 詳細：[cli-wikilink-link-target](2026-05-21-cli-wikilink-link-target-backlog.md)

### 📦 發佈 / release readiness

- **REL-2 · Claude-trace 分析 long propose prompt 的 token/cache/context 用量** — workflow efficiency · 工程量：半天
  - **現況**：只有任務描述 todo 檔、無 finding 產出。每 change 200+ 行 prompt × 多 session 累積成本未量化。跟 REL-3 同 batch。
  - 詳細：[claude-trace-prompt-analysis](2026-05-28-claude-trace-prompt-analysis-todo.md)

- **REL-3 · Claude Code 4.8 + ultracode 對開發流程影響評估** — workflow / tooling observability · 工程量：半天
  - **現況**：todo `Status: open`、無評估產出。長 prompt / spectra apply / grep 校準 / memory pattern 是否該調整。跟 REL-2 合做。
  - 詳細：[claude-code-4.8-ultracode-impact](2026-05-28-claude-code-4.8-ultracode-impact-todo.md)

### ⏳ 外部阻塞

- **EXT-1 · MyCoder CLI 整合** — pending（等對方 CLI 長出 contract）· 工程量：spike 後定
  - **現況**：codebase 零整合 code（僅 backlog/spec-trace 提及）。等對方 CLI 長出 codebus contract（見 2026-05-20 spike 結論）再評估。
  - 詳細：[mycoder-cli](2026-05-14-mycoder-cli-backlog.md)

### 🛠️ 工具 / 流程

- **TOOL-1 · spectra archive 對 `@trace` 的污染（系統性，每 change 都要手修）** — workflow 複利成本 · 工程量：偵測 MVP ✅ / 還原仍半手動
  - **問題**：每次 `spectra archive` 套用 MODIFIED / ADDED requirement 時，把該 requirement 的 `@trace` `code:` 清單**平攤成整個 dirty 工作樹的檔**（含無關檔）、多 requirement **交叉污染**（互塞對方的檔）、`source:` 被覆寫、原 provenance + `tests:` 丟失；ADDED 新 capability 的 trace 也被平攤。每個 change archive 後都要逐條手動還原。
  - **進度（2026-06-26）**：偵測 MVP 已完成——`scripts/check-trace-pollution.mjs`（純 node、無第三方依賴）掃 `openspec/specs/**/spec.md` 每個 `@trace`，標記高信心污染（`code:`/`tests:` 指向 `docs/` 或 lockfile）、SUSPECT（manifest，需人工確認版號 bump vs 真實依賴）、空 trace（刪不重生）；`--strict` 有高信心污染即 exit 1。**存量掃描：16/22 spec 受污染、63 高信心 block / 411 refs、5 空 trace、36 manifest 待確認**。用法：archive 後跑 `node scripts/check-trace-pollution.mjs` 拿精準污染清單再還原。
  - **剩餘**：(a) 還原仍半手動（依清單還原成每-req 相關檔 + 原 provenance(append 本次) + 原 tests）；進階可做 archive 前 snapshot + 後 restore 半自動化。(b) 關閉 auto-trace **已確認不可行**（`.spectra.yaml` 僅 `tdd`/`audit`/`parallel_tasks`，無 trace 開關）。(c) 回報 spectra upstream 待辦。SOP「archive 前先 commit 實作成 clean tree」仍可減少平攤無關髒檔。
  - **備註（低風險觀察，暫不開條）**：`codebus-app/src/components/workspace/GoalsTab.test.tsx:261` 的「`keyDown` 開 modal → 立即 `getByTestId`（非 `findBy`）」同 QuizTab/QuizReview 已修的 Radix Dialog mount race，CI 慢時理論上可能撞（目前穩定過、RTL 同步 flush）；未來 CI 真撞再改 `findBy`。
  - 源自 2026-06-26 backlog review；完整 SOP 見 memory `project_spectra_archive_drops_trace_on_modified_requirement`（無 in-repo detail doc）。

---

## 已完成 / Archived 項目

曾是開放項目、後來被 change 解決的，移到這裡留脈絡。部分完成的不在此（留開放、條目內標「已完成/剩 X」）。

| Archive 日期 | 標題 | 對應 change | 詳細文件 |
|---|---|---|---|
| 2026-05-14 | skill bundles repo-root copy 改 opt-in | `v3-skill-bundles-vault-only` | [skill-bundles-vault-only](2026-05-14-skill-bundles-vault-only-backlog.md) |
| 2026-05-20 | PII 設定 UI（Settings 內加 extra regex rules） | `settings-config-frontend` | [pii-settings-ui](2026-05-14-pii-settings-ui-backlog.md) |
| 2026-05-20 | .codebus 目錄即時監聽（fs watcher） | `codebus-fs-watcher` | [codebus-fs-watcher](2026-05-15-codebus-fs-watcher-backlog.md) |
| 2026-05-20 | raw mirror 巢狀 .git 未排除（submodule leak） | `raw-sync-nested-git-leak` | [raw-sync-nested-git-leak](2026-05-19-raw-sync-nested-git-leak-backlog.md) |
| 2026-05-20 | PreToolUse Read hook 擋圖片 / binary 檔案 | `pretooluse-image-block` | [pretooluse-image-block](2026-05-20-pretooluse-image-block-backlog.md) |
| 2026-05-21 | Settings 設定面板完整化（config↔UI 覆蓋盤點） | `settings-config-frontend` (Change 1) + `verify-stage-independent-model` (Change 2) | [settings-config-coverage](2026-05-19-settings-config-coverage-backlog.md) |
| 2026-05-21 | Wiki 頁面加按鈕直接開 Obsidian | `wiki-open-in-obsidian` | [wiki-open-in-obsidian](2026-05-20-wiki-open-in-obsidian-backlog.md) |
| 2026-05-23 | multi-provider agent backend（Codex CLI + Azure） | `agent-backend-seam`（Stage 1 seam）+ `codex-backend`（含 Azure profile）+ `codex-settings-ui`（GUI 設定） | [multi-provider-agent-backend](2026-05-14-multi-provider-agent-backend-backlog.md) |
| 2026-05-23 | Chat assistant 文字顯示優化（GFM 表格 + `[[wikilink]]`，app side） | `chat-display-polish-app`（app side only；CLI side 2026-05-23 併入 [cli-wikilink-link-target](2026-05-21-cli-wikilink-link-target-backlog.md)，原因：user 一直想要的是 `[[slug]]` 點下去開 codebus，純 markdown render 與連結化共用同一個渲染路徑，拆兩條會重工） | [chat-display-polish](2026-05-21-chat-display-polish-backlog.md) |
| 2026-05-23 | PII mask 重疊 match 合併（防漏遮 / 輸出損壞）+ pii-filter spec disjoint-after-merge 條款 | 直接 commit 到 `claude/backlog-review-HTtCI`（interval-merge in `mask_matches` + 7 unit tests + spec scenario） | [core-quality-review F1](2026-05-22-core-quality-review.md) |
| 2026-05-23 | raw mirror >5MiB 檔靜默排除加 warn + `oversized_skipped_files` 計數（F2）+ content-verify changed-paths 用 `--diff-filter=ACMR` 排除刪除頁（F3，避免對刪除頁 Read 失敗）；各附測試 | `core-quality-residuals`（commit `66acac7`） | [core-quality-review F2/F3](2026-05-22-core-quality-review.md) |
| 2026-05-23 | Bash hook shell-metachar bypass（F4）+ spec-drift D5 | `agent-hook-hardening`（commit `26ba1d0`：`SHELL_METACHARACTERS` denylist `; & \| $ \` > < ( ) \n \r`、metachar check 在 argv tokenize 前跑、兩 allow form 全覆蓋、fail-closed、73 hook unit tests）。**2026-05-28 adversarial re-audit 確認無存活繞過向量**（12 向量試完；殘留：`is_codebus_binary` 是 basename 字串比對非 binary-identity，`./codebus` 可過、受 vault-root cwd + `--allowedTools` prefix bound、severity 低）。**2026-06-01 test 完整度段收尾**：補 subprocess 層端對端 integration test `codebus-cli/tests/hook_check_bash.rs`（鏡像 `hook_check_read.rs`、30 tests、commit `da45c22`；decision-JSON 契約 + 每個 metachar + fail-closed 分支全覆蓋、對現行 impl 一次全綠無 finding） | [cli-quality-review F4](2026-05-22-cli-quality-review.md) + [spec-drift D5](2026-05-22-spec-drift-audit.md) |
| 2026-05-29 | Bash check-bash hook denylist over-blocks quiz self-validate heredoc（`<` 擋掉 `codebus quiz validate - <<'CBQZ'`、claude path Mode B 自驗迴圈自 2026-05-23 靜默失效）；fix = 結構化放行單引號 heredoc（body opaque；unquoted / chaining / trailing 仍擋）、F4 未回退、live CDP e2e 確認迴圈恢復 | `quiz-heredoc-selfvalidate-unblock` | [quiz-heredoc-blocked-by-metachar](2026-05-28-quiz-heredoc-blocked-by-metachar-backlog.md) |
| 2026-05-31 | per-run wall-clock timeout（hang/無人值守兜底，重用 `KillHandle.terminate_tree` + `InterruptReason::Timeout`、limit 走 `lifecycle.run_timeout_secs` 預設/0 皆不限）+ codex 內層 sandbox-denial 可觀測性（頂層 exit 0 遮蔽 → `sandbox_denial_count` + warning、MVP 不翻 outcome）（cli-applicability T1-1 + T1-2） | `run-outcome-lifecycle-integrity`（commit `46d2dee`） | [cli-applicability T1-1/T1-2](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-31 | token 累加器 codex cumulative-replace（潛伏 double-count 防護、provider-declared semantics）+ claude 非 chat verb `--no-session-persistence` + claude effort 閉集 5 值（`auto` 判不合法、修真實 spawn 失敗 + GUI 移除 auto + 修錯註解；`claude --help` 實證 CLI 只收 5 值） | `token-session-effort-hygiene`（commit `879723d`） | [cli-applicability T1-3/T1-4/T1-5](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-31 | skill bundle drift 守門（codex body 由 `claude_to_codex_translate` 6× `str.replace` 衍生、改 claude 段落會讓 codex body 靜默留假機制描述）→ guard test：每 from 必 match claude body + codex body claude-only token denylist（`--tools`/`PreToolUse`/`mcp_`/`CLAUDE.md`/`<<'CBQZ'`）+ meta-test；const seam 行為保留 | `skill-bundle-translation-guard`（commit `cf8422a`） | [cli-applicability T2-1](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-24 | prompt surface deep review 5-phase 落地（拆 claude/codex SKILL + Layer 1 batch + SpawnSpec 重構 + chat security + output discipline）；CRITICAL F1/F63（chat model 身分洩漏）/F86/F73 全解，**2026-05-31 triage 確認**、僅餘低 severity F49a（見開放項目） | `prompt-surface-layer-1-batch` + `-layer-2-skill-split` + `-layer-3-spawnspec-restructure` + `-chat-security-batch` + `-output-discipline-batch`（皆 2026-05-24 archived） | [prompt-surface-review-followup](2026-05-23-prompt-surface-review-followup-backlog.md) |
| 2026-05-31 | doc-vs-code drift 對齊：(#3) claude-code-config model spec 從假 `SystemModel` 封閉 enum 改成 free-string forward-compat（spec 對齊 code、code 未動）+ (F49a) fix SKILL `rule_id`→`rule`（對齊 lint JSON 欄位、含 guard test、drift guard 守 codex 一致）；順手修 spec effort allowed-set 誤含 `auto` 的矛盾 | `model-and-fix-skill-drift-align`（commit `3a05e83`） | （#3 propose 旁查 / [prompt-surface §F49a](2026-05-23-prompt-surface-review-followup-backlog.md)） |
| 2026-05-31 | codex chat resume Windows live round-trip 驗證（三件耦合 turn1 no `--ephemeral` / resume `-c sandbox_mode=` 非 `-s` / `thread.started` sniff）→ **spike PASS**：real codex gpt-5.4 兩輪 chat 實機跑通（turn2 代稱回憶 turn1 命名的函式、session_id 一致、cache_read 0→12k+ rollout 載入、stderr 無 no-rollout/`-s` 錯）；FINDINGS B 缺口封閉。補綁定 unit guard `chat_resume_binds_all_three_legs_in_one_build_command`（四條件並存一 assertion） | spike（純驗證，無 change）+ guard commit `028f7e3`（純 test） | [cli-applicability T2-3](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-31 | codex `multi_agent`/`spawn_agent` subagent 隔離 spike → **PASS-bounded**：codebus flags 不排除 multi_agent（spawn_agent 仍可用）但 subagent **繼承 session `-s`**（spawn_agent 無 sandbox/cwd 參數、同 process thread）；mock 強驅 worker 真寫：read-only→`blocked by policy`、workspace-write→框 workspace 內、逐格吻合、未逃逸；讀面同 main agent soft-partial（已記 security.md §5）。既有 `isolation_flags_*` guard 足夠、不加 `--disable multi_agent` | spike（無 change）+ security.md `07f90e0` + PoC `a93d43b` | [cli-applicability T2-2](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-06-01 | claude spawn user-global 設定隔離（`compose_claude_cmd` 原只隔 MCP、user `~/.claude` CLAUDE.md/settings/plugins 仍 bleed 進蓋 wiki 的 agent 帶偏行為 → 無條件加 `--setting-sources project,local`、排 user 層保 vault project+local（`check-bash`/`check-read` hook gate + `.codebus/CLAUDE.md` schema）、對齊 codex `--ignore-user-config`；2026-05-31 real-claude spike 三方驗過、否決 `--bare`） | `claude-setting-sources-user-isolation`（commit `56174cc`） | [cli-applicability T2-4](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-06-01 | raw mirror >5MiB 跳過檔對 agent 可見：除既有 warn + `oversized_skipped_files` counter（operator surface 逐字不動）外，walk 後多寫一份彙整 manifest `.codebus/raw/code/_codebus-oversized.md`（header + 每檔 forward-slash path+bytes、依路徑排序、結構上不含內容；有 oversized 才寫、無則不留、stale 靠既有 `remove_dir_all` 全量重建天然消失）；非 BACKLOG 開放項、源自 chat #5 點子（中控評低價值仍做成低噪音結構訊號） | `oversized-files-manifest`（impl `6743452` + archive `7e7620b`） | 無 backlog detail doc；proposal/design 見 `archive/2026-06-01-oversized-files-manifest/` |
| 2026-06-04 | Agent run 稽核 + vault hook integrity（(A) stderr-only sandbox denial 計 0、run 誤標 succeeded；(B) vault `.codebus/.claude/settings.json` hook 可被 inject 的 goal/fix agent 改空、無偵測）→ (A) `agent::invoke` 把 child stderr 也逐行過 `is_sandbox_denial` 計入 `sandbox_denial_count`（observability-only、不改 outcome）；(B) 新增 `vault-gate-integrity` lint 規則偵測兩條必要 hook 被移除/改空（偵測非預防、`fix` 自動帶到）；security.md §6 同步。token-parser 脆弱（2026-06-03 列）仍開放、不在本 change | `agent-run-integrity`（impl `913af73` + archive `1f53671`；specs verb-library + lint-feedback-loop） | 無 backlog detail doc；artifacts 見 `archive/2026-06-04-agent-run-integrity/` |
| 2026-06-04 | Claude path Windows 讀取邊界硬化：`check-read` 改 vault-root containment allowlist（canonicalize-then-contain、vault root 取自 PreToolUse stdin `cwd`）+ Glob/Grep PreToolUse matcher（`REQUIRED_HOOKS` 四條、`vault-gate-integrity` 連動）+ `tool_input.path` 解析 + `hooks.read_path_containment` 開關（預設 on）。F1（絕對路徑讀母 repo + denylist 外憑證）+ F2-escape（Glob/Grep 繞過讀 vault **外**）closed、live smoke 驗。vault **內**敏感檔 Read↔Grep 不對稱屬獨立後續（見開放 SEC-3） | `check-read-vault-containment`（specs lint-feedback-loop + hook；security.md §6） | 無 backlog detail doc；artifacts 見 `archive/2026-06-04-check-read-vault-containment/` |
| 2026-06-14 | Windows 打包 / 安裝（app + CLI）**P1–P5 全完成**：NSIS `-setup.exe`、bundle GUI `codebus-app.exe` + CLI `bin\codebus.exe`、per-user HKCU PATH installerHooks（P1/P2）；opt-in 卸載 purge（MB_YESNO 預設 No、Yes 才清 keyring 兩 entry + app data + `~/.codebus`、vault 永不碰、新增 `codebus config purge-keys`）；tag 觸發 GitHub Releases CI（P4）；README 安裝文件中英雙語（P5）；P3 乾淨 Windows 真機裝/卸/升級驗證（2026-06-14 user 確認）。signing / auto-update / macOS / Linux 仍 out of scope | `windows-installer-foundation`（`120e2d7`）+ `windows-uninstaller-opt-in-purge`（`fdcb0c9` / `9f2389e`）+ `windows-release-ci` | [windows-packaging-installation](2026-05-28-windows-packaging-installation-backlog.md) |
| 2026-06-16 | codex token usage parser 空快照守衛（cumulative 全 0/None snapshot 不覆蓋既有累計 + parser 四欄全不可解碼時 warn）— 原開放 COR-3 | `codex-usage-parser-zero-guard`（commit `1bad5e1`；specs agent-backend + codex-backend） | 無 backlog detail doc；artifacts 見 `archive/2026-06-15-codex-usage-parser-zero-guard/` |
| 2026-06-16 | in-vault 機密讀取邊界硬化：vault `settings.json` 加 sensitive-basename `permissions.deny`（bracket-class、case-insensitive、跨 Read/Glob/Grep）+ `vault-gate-integrity` lint 擴驗 deny + 單一來源 rule set — 原開放 SEC-3 | `vault-sensitive-basename-deny`（commit `41bb21f`；spec lint-feedback-loop） | 無 backlog detail doc；artifacts 見 `archive/2026-06-16-vault-sensitive-basename-deny/` |
| 2026-06-16 | GitHub push/PR CI（windows-latest、cargo test + clippy baseline guard + npm test/typecheck）+ issue/PR templates；連同既有 `windows-release-ci` 讓「GitHub 倉庫設定」整條 close — 原開放 REL-1 | `github-ci-and-templates`（commit `0a3c753`；spec ci-automation new capability） | 無 backlog detail doc；artifacts 見 `archive/2026-06-16-github-ci-and-templates/` |
| 2026-06-26 | PII mirror 完整性：builtin pattern 4→13（GitHub PAT / Slack / Google / OpenAI / Stripe / PEM / JWT / DB 連線字串，OpenAI alternation 不吞 `sk-ant-`）+ 非 UTF-8 改 UTF-16 BOM decode-scan（命中遮罩、真二進位 verbatim + `unscanned_files` counter）+ 前端 pattern 數由後端 `builtin_pattern_count()` 動態驅動（不再 hardcode 14）。實機 CDP smoke 驗 Settings 顯示「13 條規則」。gitignored 透明度未做（非破口、gitignored 不進 mirror 屬正確設計） — 原開放 SEC-4 | `pii-mirror-completeness`（實作 `75018df` + archive `7f789fe`；specs pii-filter / vault / app-shell） | 無 backlog detail doc；artifacts 見 `archive/2026-06-26-pii-mirror-completeness/` |
| 2026-06-26 | Agent spawn env scrub：兩 backend（claude `compose_claude_cmd` / codex `build_command`）spawn 前 `Command::env_clear()` + 共用跨平台 allowlist passthrough（通用 5 / Windows 18 含 `PATHEXT`/`ComSpec`/`SystemRoot` / Unix 4 + `LC_`/`CODEBUS_MOCK_` 前綴），父 shell 機密（`GITHUB_TOKEN`/`AWS_*`/`KUBECONFIG` + codebus 自身 `CODEBUS_*`）不再進 agent child env；`OsString`/`vars_os` 防非 UTF-8 panic；注入順序 env_clear→passthrough→provider；spawn-based sentinel 測試坐實。實機驗真 claude（system）+ 真 codex（`.cmd`→node→`codex.exe` 鏈 + PowerShell shell-out）scrub 後皆正常 spawn — 原開放 SEC-1 | `agent-spawn-env-scrub`（實作 `e16542d` + archive `e7af798`；spec claude-code-config MODIFIED + codex-backend ADDED） | 無 backlog detail doc；artifacts 見 `archive/2026-06-26-agent-spawn-env-scrub/` |

---

## 已決定不做

無對應 change，但留 backlog 文件以保決策脈絡（之後再翻出來不會以為「沒人想過」）。

| 結案日期 | 標題 | 理由 | 詳細文件 |
|---|---|---|---|
| 2026-05-20 | PII-aware git context tool | 替代「什麼都不做」可接受：source code 已 mirror 進 raw/，wiki 不缺；`raw-sync-nested-git-leak` 已把「不複製 .git」安全 floor 收掉 | [git-context-tool](2026-05-14-git-context-tool-backlog.md) |
| 2026-05-20 | Wiki 網路圖（Obsidian-style graph view） | 改用「按鈕直接開 Obsidian」取代當下需求；in-app graph 等 v2 真有沒裝 Obsidian 的使用者再開（見 [wiki-open-in-obsidian](2026-05-20-wiki-open-in-obsidian-backlog.md)） | [wiki-graph-view](2026-05-20-wiki-graph-view-backlog.md) |
| 2026-05-21 | 確認 swap 對 subagent 的控制與限制 | 2026-05-21 實測確認：`--tools` 正確排除 `Task`，spawn 出來的 agent 拿不到 Task、無法開 subagent，無逃逸途徑——無漏洞、無需修補（驗證紀錄留檔） | [subagent-sandbox-control](2026-05-21-subagent-sandbox-control-backlog.md) |
| 2026-05-28 | Bash hook denylist → 正面表列 allowlist 硬化 | 2026-05-28 adversarial audit 結論：denylist（`; & \| $ \` > < ( ) \n \r`）已覆蓋 chaining / substitution / redirection / newline，metachar-free 後剩的只能是空白分隔的 codebus argv；allowlist 需 multi-shell parser + 會破 quiz validate heredoc 自驗 + 每個新 flag 要維護、換來零邊際安全（`--allowedTools Bash(prefix *)` 已是第二閘）。不做。 | [cli-quality-review F4](2026-05-22-cli-quality-review.md) |

---

## 怎麼加新項目

1. 在 `docs/internal/` 建 `YYYY-MM-DD-<slug>-backlog.md`，內容仿照既有格式：
   - 觀察 / 問題描述
   - Proposed fix（如有多方案列出）
   - Tasks 粗估 + 工程量
   - Out of scope
   - 何時動 / 優先序
2. 在本檔「開放項目」對應主題分組（🔒安全 / 🎯正確性 / 🚀能力 / 🎨UX / 📦發佈 / ⏳外部 / 🛠️工具流程）下新增一條，沿用編號慣例（`SEC-/COR-/CAP-/UX-/REL-/EXT-/TOOL-`）+ 附「起點」檔案。
3. 之後若決定動，用 `/spectra-propose <slug>` 把該 backlog 當 pre-discuss 帶進 propose flow。

## 怎麼歸檔

對應 change archive 後（`spectra archive <change-name>`）：

- **整條完成** → 從「開放項目」移到「已完成 / Archived」表，標明對應 change 名稱 + archive 日期。
- **部分完成** → 留在「開放項目」，在該條內加「**已完成**：…（對應 change）」與「**剩**：…」兩段，別整條移走。
