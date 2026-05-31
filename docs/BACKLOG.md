# Backlog

未來 TODO 集中表。每條指向 `docs/<date>-<slug>-backlog.md` 看完整描述、proposed fix、工程量。新發現的 design smell / UX 缺陷 / feature gap 都記在這——之後再決定要不要 `/spectra-propose` 起 change。

## 開放項目

| 加入日期 | 標題 | 嚴重度 | 工程量 | 詳細文件 |
|---|---|---|---|---|
| 2026-05-14 | 全域 font-scale / accessibility text size | accessibility gap | 中（2-3 個半天） | [app-font-scale](2026-05-14-app-font-scale-backlog.md) |
| 2026-05-14 | UI 無障礙（對比度 + 鍵盤導航） | accessibility gap | 中（2-3 個半天） | [ui-accessibility](2026-05-14-ui-accessibility-backlog.md) |
| 2026-05-14 | OpenAI Privacy Filter 整合（local 語意層 PII） | PII 保護強化 | 重（3-5 個半天） | [openai-privacy-filter](2026-05-14-openai-privacy-filter-backlog.md) |
| 2026-05-14 | RAG index + search（LanceDB，after F） | 知識檢索品質 | 重（1 週以上） | [rag-index-search](2026-05-14-rag-index-search-backlog.md) |
| 2026-05-14 | codebus 作為 MCP Server（query-only，after F） | 擴充性 / 生態整合 | 中-重（3-5 個半天） | [mcp-server](2026-05-14-mcp-server-backlog.md) |
| 2026-05-14 | MyCoder CLI 整合 | pending（等對方 CLI 長出 contract，見 2026-05-20 spike 結論） | 中（spike 後定） | [mycoder-cli](2026-05-14-mycoder-cli-backlog.md) |
| 2026-05-14 | GitHub 倉庫設定（Actions CI + Release + Issue templates） | release readiness | 輕-中（1-2 個半天） | [github-repo-setup](2026-05-14-github-repo-setup-backlog.md) |
| 2026-05-14 | Settings 缺少 chat verb 的 model / effort 設定 | UX gap（設定不透明） | 輕-中（方案 A 半天 / 方案 B 1-2 半天） | [settings-chat-model](2026-05-14-settings-chat-model-backlog.md) |
| 2026-05-21 | App Activity Stream 顯示完整 AI 回覆細節（CLI 詳細模式的前端對齊） | UX 補強 | 輕-中（觸發 UX 定案後約 1 個半天） | [app-stream-verbose-detail](2026-05-21-app-stream-verbose-detail-backlog.md) |
| 2026-05-21 | 在 goal 引入動態 subagent 委派（Task 工具，AI 自主探索） | capability enhancement | 中（先 ground-truth 測 + 最小實驗版） | [goal-subagent-delegation](2026-05-21-goal-subagent-delegation-backlog.md) |
| 2026-05-21 | CLI `[[slug]]` 可點連結 + 可設定連結目標（app / obsidian，預設 app）+ CLI chat markdown polish（GFM 表格 + 視覺樣式，2026-05-23 自 chat-display-polish 併入） | regression 補回 + capability + UX 補強 | 重（codebus:// 協定吃掉大半 + markdown styling 約 1 個半天） | [cli-wikilink-link-target](2026-05-21-cli-wikilink-link-target-backlog.md) |
| 2026-05-22 | provider-specific prompt engineering（Codex 整合後輸出品質） | 輸出品質 / multi-provider 完成度 | 待研究（loop PE1 診斷 → PE2 設計後定） | [provider-prompt-engineering](2026-05-22-provider-prompt-engineering-backlog.md) |
| 2026-05-23 | 大於 5 MiB 檔案被靜默排除出 raw mirror（無 warn 行，使用者不知檔不見） | 透明度（silent gap，無安全後果） | 輕（加 oversized_skipped 計數 + 一行 stderr，半天） | [core-quality-review F2](2026-05-22-core-quality-review.md) |
| 2026-05-23 | `changed_paths_under` 把刪除頁也算 changed（content-verify 對刪除頁會 Read 失敗） | 邊緣正確性 | 輕（加 `--diff-filter=d` + 測試，半天） | [core-quality-review F3](2026-05-22-core-quality-review.md) |
| 2026-05-23 | Codex 端 hard read + command/tool 隔離（`workspace-write` 設計上允許讀 workspace 外任意檔含 `~/.ssh` `~/.aws`；**2026-05-28 Windows PoC 確認 threat C OPEN** — `workspace-write` 跟 `read-only` 都讀得到家目錄機密、isolation flags 擋不了 filesystem read；agent-hook-hardening 只給 AGENTS.md soft constraint；**2026-05-30 增補**：PII raw_sync mirror 框住讀漏的實際嚴重度（agent 讀 redacted 鏡像非 live repo）、`CODEX_AGENTS_SOFT_CONSTRAINT` efficacy 2026-05-30 對抗式 workflow 測過＝**conditional**（real codex gpt-5.4、with-constraint leak 0/8、唯一乾淨良性檔名 A/B 由 leak 翻 refuse 但 n=1；另發現 codex 對 `id_rsa` 類檔名有內建 credential guard 第二層、但良性檔名 home secret 只剩 soft constraint 獨守）→ solo-dev 接受殘餘風險+誠實記、AppContainer 留升級路徑；codex subagent 隔離未驗=cli-applicability T2-2） | 安全（codex path 缺 read enforcement，僅靠 model 自律；**Windows confirmed**、macOS/Linux 未測） | 重（待研究：separate-user / ACL deny / AppContainer / container；+ macOS/Linux 等價 PoC） | [Windows PoC](2026-05-28-codex-windows-sandbox-read-poc.md) + [§10 discussion](2026-05-23-bash-hook-and-codex-sandbox-discussion.md) + [hard-gate spike](2026-05-28-codex-hook-hard-gate-spike.md) + [cli-applicability 增補](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-28 | Bash check-bash hook 缺 subprocess 層 integration test（Read hook 有 `hook_check_read.rs`、Bash hook 無；6 個 spec scenario 只 by-proxy 驗 bool predicate、decision-JSON 契約 + fail-closed 分支無端對端 test） | 測試完整度（無 live bypass、verification gap） | 輕（建 `hook_check_bash.rs` 鏡像 read hook test，半天） | [cli-quality-review F4](2026-05-22-cli-quality-review.md)（test 完整度段） |
| 2026-05-31 | F49a（prompt-surface #25 triage 殘留）：fix SKILL Step 3 寫 `rule_id`，但 lint JSON 欄位實際是 `rule`（`wiki/lint/output.rs:51` `rule: &str`、`:255` 測試「JSON field name is `rule` (not `rule_id`)」）→ prompt↔reality mismatch。LLM 多半容忍命名差、實際 severity 低，但屬同 doc-vs-reality drift。修＝fix SKILL source `rule_id`→`rule` 後 re-materialize（drift guard 守一致） | 正確性 / 誠實（低 severity） | 輕（SKILL 一處字 + 測試） | [prompt-surface-review-followup](2026-05-23-prompt-surface-review-followup-backlog.md) §F49a |
| 2026-05-28 | Claude-trace 分析 long propose prompt 的 token / cache / context 用量（每 change 200+ 行 prompt × 多 session 累積成本未量化） | workflow efficiency / 複利成本 | 半天 | [claude-trace-prompt-analysis](2026-05-28-claude-trace-prompt-analysis-todo.md) |
| 2026-05-28 | RunId source-of-truth 統一（IPC 跟 verb 兩處獨立 `Utc::now()` 派生 RunId 跟 RunLog started_at、極端時鐘抖動下仍可能差 1ms、list_runs orphan-detection 偶誤標 interrupted；長期解需 plumb RunId 進 verb signature） | 邊緣正確性 / latent invariant | 中（5 verb signature + 5 CLI entrypoint） | [runid-source-of-truth](2026-05-28-runid-source-of-truth-todo.md) |
| 2026-05-28 | Windows 打包 / 安裝流程（app + CLI、Tauri bundler → MSI、PATH 自動加、無 signing / 無 auto-update 為 v1 scope；macOS / Linux 另開） | release readiness / distribution | 樂觀 2-3 天 / 悲觀 3-5 天（含 antivirus 誤判戰）| [windows-packaging-installation](2026-05-28-windows-packaging-installation-backlog.md) |
| 2026-05-28 | Claude Code 4.8 + ultracode 對 codebus 開發流程影響評估（長 prompt / spectra apply / grep 校準 / memory pattern 是否該調整；跟 claude-trace 分析同 batch） | workflow / tooling observability | 半天（跟 claude-trace 合做）| [claude-code-4.8-ultracode-impact](2026-05-28-claude-code-4.8-ultracode-impact-todo.md) |
| 2026-05-30 | codex `[agents]` subagent 隔離 spike（「單一受限 agent」保證只 claude 端驗過、codex `-s` 不依 tool 名 gate、可能 spawn 逃離 root-pin 的子 agent） | 安全 / multi-provider parity（關聯 codex 隔離項） | 中（ground-truth spike 後定） | [cli-applicability T2-2](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-30 | codex chat resume Windows live round-trip 驗證（三件耦合 `--ephemeral`/`-c sandbox_mode=`/`thread.started` 只逐件 unit-test、無整條跑；verification task 非 fix） | 測試完整度 / regression guard | 輕（小） | [cli-applicability T2-3](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-30 | claude `--bare` / `--setting-sources` 隔離 spike（目前只隔離 MCP、user 全域 CLAUDE.md / hooks / keychain 仍可注入、跟 codex `--ignore-user-config` 不對稱；`--bare` 會剝掉自家 vault hook 需先驗相容） | hardening（spawn 確定性） | 中（spike 相容性先） | [cli-applicability T2-4](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-30 | 新能力候選（structured output `--json-schema`/`--output-schema` 跨 provider 對等、codex `--oss` 本地模型 profile、泛化既有 `content_verify` 多 spawn orchestrator）— 有具體需求再接、不投機抽象 | new-capability（deferred） | 中-重（各別 spike 後定） | [cli-applicability T3](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-31 | claude-code-config spec ↔ code drift：spec `Requirement: System Profile Model Aliases` + scenario `Invalid SystemModel value rejected`（`claude-code-config/spec.md:64-67,131`）宣稱 `SystemModel` 是封閉 enum、`model: gpt-4` 會被 `ConfigLoadError` 拒；但 code 實際是 `model: String`（`endpoint.rs:77`，無 SystemModel enum、`system_model_to_cli_flag` 只加 `claude-` 前綴、任意字串放行）→ **spec 說會拒的值 code 其實收**。決定要嘛 spec 對齊 code 的寬鬆 forward-compat（移除假 enum claim），要嘛 code 真閉集化（不建議、會破新 model forward-compat） | spec↔code drift（誠實度；同 quiz/effort 那類） | 輕（多半 spec 對齊 code） | （無專屬 doc；`token-session-effort-hygiene` propose 旁查發現，2026-05-31） |

## 已 archived 項目

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
| 2026-05-23 | Bash hook shell-metachar bypass（F4）+ spec-drift D5 | `agent-hook-hardening`（commit `26ba1d0`：`SHELL_METACHARACTERS` denylist `; & \| $ \` > < ( ) \n \r`、metachar check 在 argv tokenize 前跑、兩 allow form 全覆蓋、fail-closed、73 hook unit tests）。**2026-05-28 adversarial re-audit 確認無存活繞過向量**（12 向量試完；殘留：`is_codebus_binary` 是 basename 字串比對非 binary-identity，`./codebus` 可過、受 vault-root cwd + `--allowedTools` prefix bound、severity 低） | [cli-quality-review F4](2026-05-22-cli-quality-review.md) + [spec-drift D5](2026-05-22-spec-drift-audit.md) |
| 2026-05-29 | Bash check-bash hook denylist over-blocks quiz self-validate heredoc（`<` 擋掉 `codebus quiz validate - <<'CBQZ'`、claude path Mode B 自驗迴圈自 2026-05-23 靜默失效）；fix = 結構化放行單引號 heredoc（body opaque；unquoted / chaining / trailing 仍擋）、F4 未回退、live CDP e2e 確認迴圈恢復 | `quiz-heredoc-selfvalidate-unblock` | [quiz-heredoc-blocked-by-metachar](2026-05-28-quiz-heredoc-blocked-by-metachar-backlog.md) |
| 2026-05-31 | per-run wall-clock timeout（hang/無人值守兜底，重用 `KillHandle.terminate_tree` + `InterruptReason::Timeout`、limit 走 `lifecycle.run_timeout_secs` 預設/0 皆不限）+ codex 內層 sandbox-denial 可觀測性（頂層 exit 0 遮蔽 → `sandbox_denial_count` + warning、MVP 不翻 outcome）（cli-applicability T1-1 + T1-2） | `run-outcome-lifecycle-integrity`（commit `46d2dee`） | [cli-applicability T1-1/T1-2](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-31 | token 累加器 codex cumulative-replace（潛伏 double-count 防護、provider-declared semantics）+ claude 非 chat verb `--no-session-persistence` + claude effort 閉集 5 值（`auto` 判不合法、修真實 spawn 失敗 + GUI 移除 auto + 修錯註解；`claude --help` 實證 CLI 只收 5 值） | `token-session-effort-hygiene`（commit `879723d`） | [cli-applicability T1-3/T1-4/T1-5](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-31 | skill bundle drift 守門（codex body 由 `claude_to_codex_translate` 6× `str.replace` 衍生、改 claude 段落會讓 codex body 靜默留假機制描述）→ guard test：每 from 必 match claude body + codex body claude-only token denylist（`--tools`/`PreToolUse`/`mcp_`/`CLAUDE.md`/`<<'CBQZ'`）+ meta-test；const seam 行為保留 | `skill-bundle-translation-guard`（commit `cf8422a`） | [cli-applicability T2-1](2026-05-30-cli-capability-applicability-backlog.md) |
| 2026-05-24 | prompt surface deep review 5-phase 落地（拆 claude/codex SKILL + Layer 1 batch + SpawnSpec 重構 + chat security + output discipline）；CRITICAL F1/F63（chat model 身分洩漏）/F86/F73 全解，**2026-05-31 triage 確認**、僅餘低 severity F49a（見開放項目） | `prompt-surface-layer-1-batch` + `-layer-2-skill-split` + `-layer-3-spawnspec-restructure` + `-chat-security-batch` + `-output-discipline-batch`（皆 2026-05-24 archived） | [prompt-surface-review-followup](2026-05-23-prompt-surface-review-followup-backlog.md) |

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

1. 在 `docs/` 建 `YYYY-MM-DD-<slug>-backlog.md`，內容仿照既有兩條格式：
   - 觀察 / 問題描述
   - Proposed fix（如有多方案列出）
   - Tasks 粗估 + 工程量
   - Out of scope
   - 何時動 / 優先序
2. 在這份 `BACKLOG.md` 的「開放項目」表加一列
3. 之後若決定動，用 `/spectra-propose <slug>` 把該 backlog 當 pre-discuss 帶進 propose flow

## 怎麼歸檔

對應 change archive 後（`spectra archive <change-name>`），把這項從「開放項目」移到「已 archived 項目」並標明對應 change 名稱 + archive 日期。
