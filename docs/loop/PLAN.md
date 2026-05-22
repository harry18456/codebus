# Loop Plan — codebus 只讀優化

每 20 分鐘由 `/loop` 喚起一次。每輪挑「最上面一個 `TODO`」執行，完成後標 `DONE`，記一筆到 `WORKLOG.md`，commit & push。

## 鐵則（自主邊界）

1. **只讀 + 寫 doc。** 唯一允許寫入的路徑是 `docs/**`（含本 PLAN、WORKLOG、產出的分析 / spike / backlog 文件）。
2. **絕不**編輯 `codebus-core/`、`codebus-cli/`、`codebus-app/`、`Cargo.*`、`.github/`、`openspec/specs/`、`openspec/changes/` 或任何 docs/ 以外的檔案。任務若需要動到這些 → 標 `BLOCKED`。
3. 不跑破壞性 git 指令；只 `add` docs/ + `commit` + `push -u origin claude/backlog-review-HTtCI`。
4. 遇到模糊 / 高風險 / 需要動實作的 → 在該任務標 `BLOCKED` 並在 WORKLOG 寫原因，**跳過去挑下一個 TODO**，不硬幹。
5. 每輪**完成一個** DONE 就停（中途若有任務變 BLOCKED 不算數，繼續挑下一個直到一個成功 DONE）。佇列全空時，**進入「自我再規劃協定」**（見下方），不要憑空發明大改造。
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

> 優先序由**物理順序**由上而下（loop 挑「最上面一個 TODO」，ID 只是標籤不代表順序）。spike = 實作前探勘：盤點現況、提方案、列 file-level 任務拆解 + 工程量 + 風險。**不寫實作**。

| # | 狀態 | 任務 | 產出 | 驗收標準 |
|---|---|---|---|---|
| PE1 | DONE | ★ 診斷: Codex 輸出不理想屬哪類成因（prompt / parser 保真度 / 模型行為） | [docs/2026-05-22-provider-prompt-diagnosis.md](../2026-05-22-provider-prompt-diagnosis.md) | ✅ 定位為「prompt 指示失準 + parser 保真度」兩類疊加；模型行為差異待 harry 樣本 |
| PE2 | DONE | ★ 設計: provider-specific prompt 策略（依 PE1） | [docs/2026-05-22-provider-prompt-design.md](../2026-05-22-provider-prompt-design.md) | ✅ C1 skill 機制無關化（輕）+ C2 codex parser 擴充（輕-中，blast radius 僅 codex_parser.rs）。CLAUDE/AGENTS 不用動。卡 ground-truth 樣本 + harry 未決問題 |
| T1 | DONE | spike: settings-chat-model（chat verb 的 model/effort 設定） | [docs/2026-05-22-settings-chat-model-spike.md](../2026-05-22-settings-chat-model-spike.md) | ✅ 發現方案 A 在 Claude 已實作、只缺 Codex 端 chat hint 列；方案 B 因 codex 加入範圍變大，有 Verb::Verify 現成範本 |
| T2 | DONE | spike: app-stream-verbose-detail（app 對齊 CLI verbose） | [docs/2026-05-22-app-stream-verbose-spike.md](../2026-05-22-app-stream-verbose-spike.md) | ✅ 2026-05-21 backlog 設計已收斂、對現碼核對屬實（純前端、6 surface 共用）。新發現：與 PE2-C2 有順序耦合（codex 編輯無 event 可展開） |
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

## 自我再規劃協定（RP）

任務佇列無剩餘 `TODO` 時觸發。**全程仍受鐵則約束（只讀 + 寫 doc）**，所以自我發明的任務頂多產低價值 doc，動不了 code——但仍要克制 churn。依序判斷該輪做哪一步：

**RP-A — 收斂 / review 已完成的產出（佇列剛清空後的第一輪一定先做這個）**
- 重讀本輪期所有產出的 spike / review / audit doc，檢查：彼此矛盾？有缺口？該交叉連結？哪些 spike 已成熟到可轉成真正的實作 change？
- 寫 `docs/2026-05-DD-loop-roundup.md`：總結已完成什麼、未解問題、**「建議 harry 核准的下一步實作清單」**（標明哪些需要解除「只讀」邊界）。
- 這輪標 DONE。

**RP-B — 提出並執行新的只讀任務（RP-A 之後的輪次）**
- 從這些來源找**真正有價值**的新 read-only/doc 工作：BACKLOG.md 還沒探勘的開放項目、RP-A roundup 點出的缺口、前面 review 發現但還沒深挖的 smell。
- 每輪最多新增 **3** 個候選，寫進下方「候選任務（自我生成）」表，類型限 spike / review / audit（產 doc，不碰實作）。**不得與已 DONE 的主題重複。**
- 然後就地挑一個候選執行（升為該輪的 DONE），其餘留在候選表等下一輪。
- 候選表的內容會在 WORKLOG 留痕，harry 可隨時刪減。

**RP-C — 真的沒有有價值的新工作時**
- 不要為了跑而捏造 filler。在 WORKLOG 記「無新增有價值任務 — 等 harry」並停這輪。下一輪再評估一次。

### 候選任務（自我生成 — RP-B 寫入）

| # | 狀態 | 任務 | 產出 | 來源 / 理由 |
|---|---|---|---|---|
| _（loop 於 RP-B 階段追加）_ | | | | |

---

harry 回來檢視 docs/ 產出（尤其 `loop-roundup`），決定哪些 spike 要轉成真正的實作 change——那需要另起任務並明確解除「只讀」邊界。
