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
| 2026-05-22 | Bash hook 只檢查前兩 token，shell 串接可能繞過 sandbox（含 spec 補 metacharacter 拒絕條款 D5） | 安全（sandbox bypass，待驗 Claude Code 串接行為） | 輕（拒 shell 元字元 + 測試 + spec requirement，約半天） | [cli-quality-review F4](2026-05-22-cli-quality-review.md) + [spec-drift D5](2026-05-22-spec-drift-audit.md) |
| 2026-05-23 | 大於 5 MiB 檔案被靜默排除出 raw mirror（無 warn 行，使用者不知檔不見） | 透明度（silent gap，無安全後果） | 輕（加 oversized_skipped 計數 + 一行 stderr，半天） | [core-quality-review F2](2026-05-22-core-quality-review.md) |
| 2026-05-23 | `changed_paths_under` 把刪除頁也算 changed（content-verify 對刪除頁會 Read 失敗） | 邊緣正確性 | 輕（加 `--diff-filter=d` + 測試，半天） | [core-quality-review F3](2026-05-22-core-quality-review.md) |
| 2026-05-23 | Codex 端 hard read 隔離（`workspace-write` 設計上允許讀 workspace 外任意檔，含 `~/.ssh/` `~/.aws/` 等敏感檔；agent-hook-hardening 只給 AGENTS.md soft constraint，hard enforcement 留此條） | 安全（codex path 缺 read enforcement，僅靠 model 自律） | 重（待研究：writable_roots Mac/Linux 實機驗 → Windows ACL/chmod → container 化 / sandbox-of-sandbox） | [bash-hook-and-codex-sandbox-discussion §10](2026-05-23-bash-hook-and-codex-sandbox-discussion.md) |
| 2026-05-23 | prompt surface deep review 後續行動（PE1/PE2 落地：拆 claude/codex SKILL + Layer 1 batch + SpawnSpec 重構 + verb 設計 fixes，5-phase 計畫） | 輸出品質 / multi-provider 完成度（PE1/PE2 接續）+ 含 4 個 🔴 CRITICAL finding | 重（5 phases × 半天-3 半天 = 約 1-2 週；分階段 propose） | [prompt-surface-review-followup](2026-05-23-prompt-surface-review-followup-backlog.md) |
| 2026-05-28 | Claude-trace 分析 long propose prompt 的 token / cache / context 用量（每 change 200+ 行 prompt × 多 session 累積成本未量化） | workflow efficiency / 複利成本 | 半天 | [claude-trace-prompt-analysis](2026-05-28-claude-trace-prompt-analysis-todo.md) |
| 2026-05-28 | RunId source-of-truth 統一（IPC 跟 verb 兩處獨立 `Utc::now()` 派生 RunId 跟 RunLog started_at、極端時鐘抖動下仍可能差 1ms、list_runs orphan-detection 偶誤標 interrupted；長期解需 plumb RunId 進 verb signature） | 邊緣正確性 / latent invariant | 中（5 verb signature + 5 CLI entrypoint） | [runid-source-of-truth](2026-05-28-runid-source-of-truth-todo.md) |

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

## 已決定不做

無對應 change，但留 backlog 文件以保決策脈絡（之後再翻出來不會以為「沒人想過」）。

| 結案日期 | 標題 | 理由 | 詳細文件 |
|---|---|---|---|
| 2026-05-20 | PII-aware git context tool | 替代「什麼都不做」可接受：source code 已 mirror 進 raw/，wiki 不缺；`raw-sync-nested-git-leak` 已把「不複製 .git」安全 floor 收掉 | [git-context-tool](2026-05-14-git-context-tool-backlog.md) |
| 2026-05-20 | Wiki 網路圖（Obsidian-style graph view） | 改用「按鈕直接開 Obsidian」取代當下需求；in-app graph 等 v2 真有沒裝 Obsidian 的使用者再開（見 [wiki-open-in-obsidian](2026-05-20-wiki-open-in-obsidian-backlog.md)） | [wiki-graph-view](2026-05-20-wiki-graph-view-backlog.md) |
| 2026-05-21 | 確認 swap 對 subagent 的控制與限制 | 2026-05-21 實測確認：`--tools` 正確排除 `Task`，spawn 出來的 agent 拿不到 Task、無法開 subagent，無逃逸途徑——無漏洞、無需修補（驗證紀錄留檔） | [subagent-sandbox-control](2026-05-21-subagent-sandbox-control-backlog.md) |

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
