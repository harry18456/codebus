# Loop Plan — codebus 只讀優化

每 20 分鐘由 `/loop` 喚起一次。每輪挑「最上面一個 `TODO`」執行，完成後標 `DONE`，記一筆到 `WORKLOG.md`，commit & push。

## 鐵則（自主邊界）

1. **只讀 + 寫 doc。** 唯一允許寫入的路徑是 `docs/**`（含本 PLAN、WORKLOG、產出的分析 / spike / backlog 文件）。
2. **絕不**編輯 `codebus-core/`、`codebus-cli/`、`codebus-app/`、`Cargo.*`、`.github/`、`openspec/specs/`、`openspec/changes/` 或任何 docs/ 以外的檔案。任務若需要動到這些 → 標 `BLOCKED`。
3. 不跑破壞性 git 指令；只 `add` docs/ + `commit` + `push -u origin claude/backlog-review-HTtCI`。
4. 遇到模糊 / 高風險 / 需要動實作的 → 在該任務標 `BLOCKED` 並在 WORKLOG 寫原因，**跳過去挑下一個 TODO**，不硬幹。
5. 每輪**完成一個** DONE 就停（中途若有任務變 BLOCKED 不算數，繼續挑下一個直到一個成功 DONE）。佇列全空時，在 WORKLOG 記「PLAN exhausted — 等 harry 補新任務」然後停，不要自己發明大改造。
6. 產出文件命名 `docs/2026-05-DD-<slug>.md`，並在對應 backlog 行尾補連結（編輯 `docs/BACKLOG.md` 屬寫 doc，允許）。

## 每輪流程

1. `git pull origin claude/backlog-review-HTtCI`（確保最新）
2. 讀本 PLAN + `WORKLOG.md`
3. 挑最上面狀態為 `TODO` 的任務
4. 在鐵則內執行；產出 doc
5. 把該任務狀態改 `DONE`（或 `BLOCKED`）
6. append 一筆 WORKLOG（時間、任務、做了什麼、產出檔、下一步）
7. `git add docs/ && git commit && git push`
8. 結束這一輪

## 任務佇列

> 優先序由上而下。spike = 實作前探勘：盤點現況、提方案、列 file-level 任務拆解 + 工程量 + 風險。**不寫實作**。

| # | 狀態 | 任務 | 產出 | 驗收標準 |
|---|---|---|---|---|
| T1 | TODO | spike: settings-chat-model（chat verb 的 model/effort 設定） | `docs/2026-05-DD-settings-chat-model-spike.md` | 盤出 chat verb 目前 model/effort 在哪解析、方案 A/B 各動哪些檔、task 拆解 + 工程量 |
| T2 | TODO | spike: app-stream-verbose-detail（app 對齊 CLI verbose） | `docs/2026-05-DD-app-stream-verbose-spike.md` | 比對 CLI verbose 渲染 vs app activity stream 來源、列前端需改的元件 + 資料流 |
| T3 | TODO | spike: chat-display-polish（GFM 表格 + `[[wikilink]]`，app+CLI） | `docs/2026-05-DD-chat-display-polish-spike.md` | 盤現有 chat 渲染管線（app 與 CLI 兩側）、缺口、proposed 改法 + 風險 |
| T4 | TODO | spike: github-repo-setup（CI / release / issue template） | `docs/2026-05-DD-github-repo-setup-spike.md` | 草擬 workflow YAML 內容 + release/issue template 方案（寫在 doc 裡，不建 .github/） |
| T5 | TODO | spike: goal-subagent-delegation（Task 工具委派） | `docs/2026-05-DD-goal-subagent-delegation-spike.md` | 盤 goal verb 現有 tool 白名單、開 Task 需動什麼、ground-truth 風險、最小實驗版設計 |
| T6 | TODO | 品質檢查: codebus-core | `docs/2026-05-DD-core-quality-review.md` | 逐 module 讀，列 bug / design smell / 缺測試，每條給嚴重度 + backlog 候選 |
| T7 | TODO | 品質檢查: codebus-cli | `docs/2026-05-DD-cli-quality-review.md` | 同上，針對 cli commands |
| T8 | TODO | 品質檢查: codebus-app（前端） | `docs/2026-05-DD-app-quality-review.md` | 同上，針對 components/hooks/store |
| T9 | TODO | spec drift 檢查（specs vs code） | `docs/2026-05-DD-spec-drift-audit.md` | 比對 openspec/specs 與實際 code，列出漂移點（純讀，不改 spec） |
| T10 | TODO | README / docs 新鮮度稽核 | `docs/2026-05-DD-docs-freshness-audit.md` | README 與實際功能 / 指令對齊，列過時段落 |
| T11 | TODO | spike: mcp-server（codebus 當 MCP server，query-only） | `docs/2026-05-DD-mcp-server-spike.md` | 盤 query 路徑可重用性、MCP server 介面草案、工程量 |
| T12 | TODO | spike: rag-index-search（LanceDB） | `docs/2026-05-DD-rag-index-search-spike.md` | 盤 vault 內容來源、index 設計選項、工程量 + 風險 |
| T13 | TODO | spike: openai-privacy-filter（local 語意層 PII） | `docs/2026-05-DD-openai-privacy-filter-spike.md` | 盤現有 pii module、語意層方案、與既有 regex filter 的關係 |

## 完成後

佇列全 DONE 後 loop 進入 no-op 等待狀態。harry 回來檢視 docs/ 產出，決定哪些 spike 要轉成真正的實作 change（那需要解除「只讀」邊界，另起任務）。
