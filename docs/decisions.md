# Decisions Log

> 已討論過的取捨、決策與待辦。ADR 風格，按 ID 追蹤。
> **狀態**：已決 / 未決 / 待動作

---

## D-001: 技術棧（全 Rust vs Tauri + Python sidecar）

**狀態**：✅ 已決 — **Tauri 殼 + Python Sidecar** 混合架構（2026-04-17）
**長期演進**：Sidecar HTTP API 定死，Phase 3+ 逐模組 port 到 Rust + Rig
**討論日期**：2026-04-17

**脈絡**
Rust + Tauri + Rig + Qdrant 全是新技術，且要從「學 Rust 基礎」開始。MVP 時程緊，若 Rust 學習曲線吃掉工期，Agent 品質調校就沒時間。

**選項**

| 選項 | 優 | 缺 |
|---|---|---|
| A. 全 Rust（原計畫） | 架構乾淨、Tauri 原生整合、performance | 學習曲線陡、Rig 生態新、除錯難 |
| B. Tauri 殼 + Python sidecar | Python 生態成熟（有現成 Agent 框架）、debug 快 | IPC 複雜度、打包變重、隱私 claim 需謹慎 |
| C. 全 Python + Electron | 最快上手 | 性能、打包、與 Rust claim 衝突 |

**決策理由**
使用者主力 Python。考量以下因素決定混合：
1. Agent 邏輯的教材品質最關鍵——用 Python 主力語言 iteration 快、AI 輔助產出可 review
2. Rig 生態新、AI 訓練資料少，遇坑排不掉
3. Rust 編譯慢（10-30 秒 × N 次），debug 成本高
4. Rust 還是學得到——Tauri 殼那層有 ownership / trait / async 基本功
5. 升級路徑：HTTP sidecar API 定死後可逐模組遷 Rust，不用重寫

**後續**
- [x] README 技術棧更新（2026-04-17）
- [x] Python sidecar 框架選定 **FastAPI**（由 D-014 `uv add fastapi` 實質定案；`agent-core.md §十五` 目錄 `api/` 用 FastAPI routes）
- [ ] Tauri ↔ Python HTTP IPC schema 草案
- [ ] PyInstaller 打包流程驗證

---

## D-002: Topic mode 是否進 MVP

**狀態**：已決 — **不進 MVP**
**討論日期**：2026-04-17

**理由**
1. MVP 工期已緊
2. Topic mode 需多實作：web fetch、HTML parse、source 品質判斷、YouTube API、爬蟲節流——每個都是坑
3. Folder mode + self-review loop 的 agentic 故事**已夠硬**，不需要 Topic 撐
4. 做半成品 Topic 比沒做更傷

**緩解**
架構 day 1 就用 trait 抽象好（`ExplorerTools` / `Judge` / `CoverageChecker`），Phase 2 加 Topic 不用動核心。spec 已完整寫在 `agent-explorer-spec.md` 第十二章。

**對外話術**
「Topic mode 是工期問題，不是設計問題。trait day 1 就支援雙模式。」

---

## D-003: 本地 LLM 備援（Ollama）

**狀態**：✅ 已決（2026-04-17）— **MVP 做 LLM Provider 抽象層，只實作指定 LLM 供應商 API；Phase 2 視需求加 Ollama**
**討論日期**：2026-04-17

**脈絡**
目前使用外部 LLM 供應商 API = embedding 與對話都送雲端。但 README 的「本地優先、資料不外流」宣傳會**被這件事打折**——企業資安角度，code 送去第三方 API 就算外流。

**決策理由**
- Provider 抽象 1-2 天工期，很便宜，保留未來切換空間
- 不做完整 Ollama 整合：模型品質差太多（Claude/GPT vs 本地 Llama3/Qwen），demo 路線品質會砸招牌；Ollama 吃記憶體，demo 機未必跑得動
- README 已用精確說法（D-009）：「程式碼不落地雲端；LLM / Embedding 透過 API（Phase 2 可選自架）」

**後續**
- [ ] 設計 LLM Provider 抽象介面（Python Protocol / ABC）
- [ ] Phase 2 評估 Rig 是否原生支援 Ollama（若屆時已 port 到 Rust）
- [ ] Phase 2 評估 Ollama 本地模型品質（對 golden sample 跑一次看召回/雜訊）

---

## D-004: MVP 硬上限（1 repo + 1 task）

**狀態**：✅ 已決（2026-04-17）
**Demo Repo**：Timeline（`~/projects/timeline`）
**Demo Task**：新增 Google Drive Adapter 同步功能
**討論日期**：2026-04-17

**決策**
Demo 就用 **1 個預選 repo + 1 個預選任務** 走通 end-to-end。其他泛化能力**不在 MVP**。

**選 Timeline 的理由**
- 使用者熟（自己的專案）
- 規模剛好：45 個 vue/ts 檔案，分層清楚
- 有明顯的 **Adapter Pattern**（`MockStorageAdapter` + `LocalFileAdapter`）→ Agent 探索 demo 有戲
- 技術棧（Nuxt 4 + TS + Tailwind + PWA）主流可辨識
- 本身就是我們 CodeBus 前端參考藍本，self-referential 話題點

**選此任務的理由**
- 任務導向：「要加一種儲存後端」每個 RD 30 秒懂
- Agent 探索路徑豐富：trace_import Storage 介面、find_callers Adapter、讀兩個現有實作當範例
- 路線自然 4-5 站
- 任務指向**尚未實作的功能**，正好 demo CodeBus 的主情境（「要動手做 X 前，該懂哪些現有程式碼」）
- 第 5 站可留 Quiz，考使用者「看完前四站，GoogleDriveAdapter 該長什麼樣？」

**後續**
- [x] 選定 demo repo + task（2026-04-17）
- [ ] 起草 golden sample ideal route（Harry 主 review）
- [ ] 決定放在 `tests/golden/timeline-gdrive-adapter/` 結構

---

## D-005: 加入 DeepWiki 等關鍵競品對照

**狀態**：✅ 已完成（README 更新 2026-04-17）
**討論日期**：2026-04-17

**脈絡**
README 現有對照表列 NotebookLM / Code2Tutorial / GitHub Copilot，**漏掉最直接對手 DeepWiki**（Cognition/Devin 出品的 repo wiki 自動生成）。觀眾很可能問。

**要補的競品**
- **DeepWiki**：重疊度最高，差異在任務導向動態路線 + agent 可視化 + 本地
- **Greptile**：codebase Q&A + PR review，無學習路線
- **Sourcegraph Cody**：企業 codebase 問答，無教材生成
- **Swimm**：onboarding 平台，但教材是人寫的

**準備話術**
> DeepWiki 是**靜態**的 repo 說明書，看完還要自己規劃學什麼。CodeBus 是**任務導向動態路線**，同一 repo 不同任務走不同站牌；DeepWiki 沒 agent 可視化，看不到決策理由。

**後續**
- [ ] README「與現有工具差異化」對照表加入這四項
- [ ] 常見問答章節加一題「跟 DeepWiki 差在哪」

---

## D-006: Golden sample 評估機制

**狀態**：✅ 已補進 README（2026-04-17）；待執行 golden samples 建立
**討論日期**：2026-04-17

**脈絡**
「反覆調 prompt」不是計畫。沒有評估標準，MVP 結束前一週才會發現教材品質爛。

**決策**
MVP 啟動時就選 **2 個熟悉 repo + 3 個任務**，人工寫 ideal route，每次改 prompt 都自動評分：
- 核心檔案召回率
- 雜訊率（多餘檔案比例）
- 依賴完整度
- 人工評分 rubric（1-5）：順序合理性、粒度、新人可讀性

**後續**
- [ ] spec 裡已有（`agent-explorer-spec.md` 十一、評估方式），README 補一段提到這機制
- [ ] 建立 `tests/golden/` 放 ideal routes

---

## D-007: Cost 估算

**狀態**：待動作
**討論日期**：2026-04-17

**脈絡**
Embedding + Explorer Agent 每步 LLM + Judge + Coverage Checker + Generator，大型 repo 可能幾百萬 tokens。所選 LLM 供應商 API 是否有限額？超量怎麼辦？

**後續**
- [ ] 確認 LLM 供應商 API 限額與計費規則
- [ ] 實作完 Module 2/4 後做大/中/小 repo 各一次 cost benchmark
- [ ] 若超量，Budget 收斂機制加嚴

---

## D-008: First-run UX 三個等待點

**狀態**：✅ 已設計方向（2026-04-17）
**討論日期**：2026-04-17

**脈絡**
使用者流程：選資料夾 → embedding（慢）→ Explorer 探索（慢）→ 產教材（慢）→ 結果。中間三個等待可能各 1-5 分鐘，沒進度可看會被關掉。

**設計決定**

**顯示內容（Q1）**：進度條 + 細節 + Agent thought（分階段給）
- **階段 1（Embedding）**：進度條 + 當前處理檔案（低調）
- **階段 2（Exploring）**：**Agent console 全公開**——這階段等最久，也最能展示 agentic，把缺點變賣點
- **階段 3（Generating）**：進度條 + 當前寫哪一站

**UI 架構（Q2）**：一個 timeline 橫跨三階段並列顯示，當前階段高亮
讓使用者知道自己在整個流程哪個位置，不是黑盒等待

**取消 / 暫停（Q3）**：支援「取消」，不支援「暫停」
Cancel 實作簡單；Pause 要處理 state 保留、LLM in-flight request 等，MVP 不值得

**連動**
Exploring 階段的 console 正是最強的 demo 資產——**UX 和 demo 魔法同一個東西**。Agent console 實作後 demo 時直接用。

**後續**
- [ ] 三階段進度元件 Vue 元件 spec
- [ ] Python sidecar emit progress event 到 Tauri → 前端的 schema
- [ ] Agent console 元件（顯示 reasoning_log 即時 stream）
- [ ] Cancel 按鈕流程（前端點 → Tauri → sidecar 收 signal → 清理）

---

## D-009: 「本地優先」claim 的精確度

**狀態**：✅ 已完成（README 更新 2026-04-17）
**討論日期**：2026-04-17

**脈絡**
現在寫「本地優先、資料不外流」會誤導——LLM 和 embedding 都送外部 API = 外流。企業資安角度不成立。

**決策**
改成精確說法：

> 「**程式碼與文件本地處理**；LLM / Embedding 透過 API 呼叫（模型供應商，可選企業自架 / 本地模型）；產出的教材、進度、知識庫索引均存本地，不上傳雲端。」

**後續**
- [ ] README 差異化賣點改寫本地段落
- [ ] README 對照表欄位從「本地」改成「code 不落地雲端」更精確
- [ ] 連動 D-003（Ollama 備援路徑）

---

## D-011: 資安與合規設計

**狀態**：✅ 已決方向（2026-04-17）— 開 `docs/security.md` 當實作 checklist
**討論日期**：2026-04-17

**脈絡**
專案需遵循一般性 Agentic AI 安全規範（區隔環境、低權限沙箱、不開埠、PII 去識別化、稽核 trail）。以下列為關鍵要求，對 CodeBus 有多項直接影響：
1. **禁送 PII 至公有雲 LLM API** → Scanner 必須有 PII 去識別化
2. **限用完全模擬資料、不涉公司名** → Demo repo 選擇要確認（使用者已確認 Timeline 無問題）
3. **不可開放服務埠對外** → Sidecar bind 127.0.0.1 + token auth
4. **遠端熔斷機制** → Cancel + config kill switch

**決策**
- 開 `docs/security.md` 當實作 checklist（不是 marketing 文案）
- 所有合規要求寫進對應 Module spec（Module 1 PII / Sidecar 設定 / First-run UX）
- Demo 前 checklist 固定在 security.md 第四章

**對既有決策的影響**
- D-004：Demo repo（Timeline）使用者確認合規，不用換
- D-008：First-run UX 增加授權 modal 要求
- D-001：Sidecar 設計強化（127.0.0.1 + token + 生命週期）
- 新增：Module 1 Scanner 多一層 PII 去識別化（之前只有 secret）

**後續**
- [x] `docs/security.md` 寫成（2026-04-17）
- [ ] README 加資安章節指向 security.md
- [ ] 實作時逐項 tick checklist
- [ ] 提交前走一次 Demo Checklist

---

## D-012: Agent 框架選型 — 自寫 ReAct + Instructor 輔助

**狀態**：✅ 已決（2026-04-17）— 開 `docs/agent-core.md` 寫實作 spec
**討論日期**：2026-04-17

**脈絡**
AI 層要處理 Agent loop、Judge、Coverage Checker、tool calling、prompt 管理、重試、budget 等。選型影響其他子問題長什麼樣。

**選項**

| 選項 | 優 | 缺 |
|---|---|---|
| A. LangChain / LangGraph | 生態大、tool calling 現成、有 agent 範例 | 抽象厚、debug 難、「wrap LangChain」對 agentic 故事扣分、讀文件繞抽象的時間不輸自寫 |
| B. Instructor 為主 + 自寫 loop | 只借 structured output、Agent 邏輯全自寫 | 工具註冊、重試等要自己刻 |
| C. 完全自寫（包括 JSON parse） | 最純 | 自寫 schema 驗證 + 重試浪費時間，學不到 Agent 原理 |

**決策**
選 **B**——自寫 ReAct loop / Judge / Coverage Checker / tool registry，但借 Instructor + Pydantic 處理 structured output 與 schema 驗證。

**決策理由**
1. 使用者偏好自寫體會原理 → 核心邏輯自寫
2. 「Agent core 自己刻」對 Demo / agentic 故事加分，比「wrap framework」有說服力
3. Schema 驗證 / retry parsing 不是 Agent 原理，那塊讓 Instructor 做工期省
4. Instructor 薄封裝不影響「自寫 Agent」的敘事
5. 工期跟選 LangChain 其實沒差多少（LangChain 要讀文件繞抽象）

**後續**
- [x] `docs/agent-core.md` 寫成（2026-04-17）
- [ ] 實作階段按 §十六 順序推進
- [ ] Provider 層 `chat_structured` method 加上（`llm-provider.md` §二 需擴充）

---

## D-013: 專案組織 — Monorepo + 目錄分層（不用 git submodule）

**狀態**：✅ 已決（2026-04-17）
**討論日期**：2026-04-17

**脈絡**
CodeBus 有 Tauri 殼（Rust）/ Python sidecar / Nuxt3 前端三塊，要決定用 monorepo 還是 submodule 拆多 repo。

**選項**

| 選項 | 優 | 缺 |
|---|---|---|
| A. Multi-repo + git submodule | 獨立版本化、強制介面乾淨 | submodule 操作繁瑣、CI `--recurse-submodules`、commit 要雙層推、AI 輔助跨 submodule context 斷 |
| B. Monorepo + 目錄分層 | 單一 clone、CI 簡單、Claude 可跨整個專案 | 需自律維持邊界 |
| C. Monorepo + workspace（pnpm / uv） | B 的好處 + 真正 workspace | 工具配置略多 |

**決策**
選 **C**（monorepo + 目錄分層 + 各語言 workspace）：
```
codebus/
├── tauri/              # Rust 殼
├── sidecar/            # Python（uv pyproject.toml）
│   └── src/codebus_agent/{agent,modules,providers,api}/
├── web/                # Nuxt3
├── docs/
└── tests/
    ├── golden/
    └── fixtures/       # Timeline 固定 commit clone 進來
```

**理由**
1. Solo / MVP 工期緊，submodule overhead 純虧
2. 「模組化」心智效果靠目錄分層 + Python package + `Protocol` 抽象就夠
3. AI 輔助（Claude）對 monorepo 友善，跨 submodule context 會斷
4. 單一 commit = 一個完整可跑的快照，debug / rollback 簡單
5. Phase 3 真要開源 agent core 再拆也來得及

**Timeline demo repo 處理**
不用 submodule，走 `tests/fixtures/timeline/` 固定 commit hash clone（記在 README / test fixture metadata），CI 可快取。

**後續**
- [ ] 建立 monorepo 目錄骨架（tauri/ sidecar/ web/ tests/fixtures/）
- [ ] `tests/fixtures/` README 記錄 Timeline commit hash
- [ ] `.gitignore` 分層：各子目錄自己加，root 保留共用

---

## D-015: Sanitizer 規則與架構

**狀態**：✅ 已決方向（2026-04-17）— 詳見 `docs/sanitizer.md`
**討論日期**：2026-04-17

**脈絡**
D-011 定了「要做 sanitizer」沒定「怎麼做」。AI 層所有 LLM call 前都要過這層，是橫切關注點，要先定清楚 schema / 觸發點 / placeholder 格式，其他模組才能平行實作。

**關鍵決策（摘要）**

| 項 | 決定 |
|---|---|
| 偵測範圍 | Secret + 基礎 PII（email / 台灣手機 / 身分證）+ 內部識別符（IP / 域名） |
| 工具 | detect-secrets（secret）+ 自刻 regex（PII / 內部）+ 使用者 config（公司特定） |
| 公司內部清單 | **走使用者 `~/.codebus/sanitizer.local.yaml`**，不進 git，不進對話 |
| 高熵字串 | 開但走「suspect」等級，不直接替，列入稽核讓使用者 review |
| Test 檔 | 不自動跳過，靠路徑白名單宣告 |
| 觸發點 | **三段式**：Scanner 入庫前 + Provider pre-flight + Q&A `add_to_kb` 寫入前（連動 D-016） |
| 儲存 | KB / reasoning_log / 教材全存清理版；不存 reverse mapping |
| Placeholder 格式 | `<REDACTED:kind#index>`，index 單檔 scope |
| 白名單 | 路徑 glob / 檔名 / pattern 三層，皆透過 yaml config |
| 稽核 log | `sanitize_audit.jsonl` 記類別數量，不記原文 |
| 授權 modal | 首次選資料夾彈 modal 告知替換範圍 + 同意 checkbox |
| Demo 揭露 | UI 「🛡️ 稽核報告」tab 顯示本 session 替換統計 |

**後續**
- [x] `docs/sanitizer.md` 寫成（2026-04-17）
- [ ] 實作按 sanitizer.md §九 工期排（P0 約 3d / P0+P1 約 7d）
- [ ] `tests/fixtures/sanitizer/` 準備
- [ ] 使用者 review 本版（有無需調整）

---

## D-016: Q&A Agent + 持續成長 KB（進 MVP）

**狀態**：✅ 已決（2026-04-17）— 詳見 `docs/qa-agent.md`
**討論日期**：2026-04-17

**脈絡**
教材完成後使用者仍會有問題（尤其 git history / 未涵蓋細節）。單純 RAG 查詢不夠，需要 Agent 判斷 RAG 不足時即時補查並決定是否沉澱進 KB。這正是「agentic + 持久化 KB」賣點的使用者端延續。

**替代方案（之前討論過）**
- Case A：前端按鈕 +Tauri command 跑 `git log` — 純 UI，不 agentic
- Case B：Q&A Agent + 專用 git exec 工具 — 開了 exec surface，合規要加新章節
- **Case C（選定）**：Q&A 走 RAG，不足時 Agent 用 Explorer 同組 read-only tools 補查 + 自主判斷 `add_to_kb`

**四項關鍵抉擇**

| 抉擇 | 決定 |
|---|---|
| 即時補查 scope | **只在原 workspace 內**，Phase 2 才評估外部網路 |
| KB 寫入審核 | **Agent 自動加 + 稽核頁透明 + 單筆 rollback**（符合 agentic 敘事） |
| 「值得沉澱」規則 | Prompt 要求 Agent 滿足三條：可復用 / stable fact / 非重複；系統再用 similarity > 0.95 去重 |
| 進 MVP 或 Phase 2 | **進 MVP**（3-5d 工期，demo 有戲、agentic 在使用者端持續可感） |

**決策理由**
- 完美契合 README「持久化知識庫」賣點（KB 不是凍結快照，是活的）
- Reuse Explorer trait 抽象（D-002）—— 換 prompt + 加 `add_to_kb` tool 即可
- 不開 exec surface，合規故事不變（仍 read-only + 一個寫 KB 的 tool）
- Sanitizer 覆蓋自然延伸到 `add_to_kb` pipeline
- Demo 金句：「問它教材沒教的細節，它自己去找並記起來」

**後續**
- [x] `docs/qa-agent.md` 寫成（2026-04-17）
- [x] 連動更新：sanitizer.md / agent-core.md / sidecar-api.md / interactive-tutorial.md / README（詳見 `qa-agent.md §十二`）
- [ ] 實作按 qa-agent.md §十一 工期排（P0 約 3.5d / P0+P1 約 5d）
- [ ] KB growth 防呆閾值（見 §七）實作時確認

---

## D-017: Tool Sandbox — Agent 執行邊界

**狀態**：✅ 已決（2026-04-17）— 詳見 `docs/tool-sandbox.md`
**討論日期**：2026-04-17

**脈絡**
Sandbox（Agent 能摸什麼）與 Sanitizer（送 LLM 的字清不清）是**兩層獨立防線**，之前散在 security.md / agent-core.md / sanitizer.md 三處，沒有集中 spec。Scanner / Explorer / Q&A 都需要同一套路徑驗證 / 寫入邊界，不集中會實作不一致。

**關鍵決策（摘要）**

| 項 | 決定 |
|---|---|
| 檔案讀取 | 限 `workspace_root` 子樹 + `~/.codebus/`（config 層）+ `.git/`（只讀） |
| 檔案寫入 | **完全禁止**——無寫 filesystem 的 tool |
| Shell / exec / subprocess | **完全禁止**（MVP read-only 承諾） |
| 網路存取 | 只允許 Provider 層 LLM API + localhost Qdrant |
| 路徑驗證 | `ensure_in_workspace()` helper（resolve + is_relative_to 雙檢查） |
| Tauri 端 | `fs.scope` allow + deny；workspace_root runtime 加入 |
| Git metadata | 用 `pygit2`（C binding，不 spawn subprocess），只讀 |
| 重複違規 | 同 session 5 次 escape 停迴圈 + UI 警告 |
| 稽核 | `tool_audit.jsonl` 記呼叫 / 拒絕；UI 多一個 🔒 Tool Sandbox tab |

**決策理由**
1. 「Agent 不破壞使用者 code」合規承諾靠的是「沒有寫 tool」，白紙黑字寫清楚比口頭承諾有力
2. `ensure_in_workspace` 集中一處，所有 tool reuse，漏寫就是 lint error
3. `pygit2` 讓 git metadata 不開 exec surface
4. Red team fixture 進 CI，path escape 攻擊每次 PR 都檢

**後續**
- [x] `docs/tool-sandbox.md` 寫成（2026-04-17）
- [ ] 實作按 §十五 工期排（P0 ~2.5d / P0+P1 ~4.75d）
- [ ] `tests/sandbox/attacks/` red team fixture
- [ ] security.md 加指向 tool-sandbox.md 的章節

---

## D-018: Module 1 Scanner 細節定案

**狀態**：✅ 已決（2026-04-17）— 詳見 `docs/module-1-scanner.md`
**討論日期**：2026-04-17

**脈絡**
README Module 1 有清單但細節散；Scanner 是資料流 entry point，也是 Sanitizer / Sandbox / git metadata 的實際整合點，需獨立 spec。

**關鍵決策（摘要）**

| 項 | 決定 |
|---|---|
| 遍歷工具 | `pathlib.rglob` + `pathspec`（階層 gitignore）|
| 預設不跟隨 symlink | 記 symlink 但不讀內容，config 可 opt-in |
| Binary 偵測 | 副檔名黑名單 + 前 8KB null byte + 非可印字符比例 30% |
| Encoding fallback | utf-8 → utf-16 BOM → big5 → gbk → shift_jis → `charset-normalizer` |
| Lockfile / generated | 記存在與大小，**不讀內容**、**不進 KB** |
| 超大檔 | 記 metadata + 前 200 行 preview，Explorer 需要時 partial read |
| 語言識別 | 副檔名 + shebang（不用 pygments / linguist） |
| Monorepo | pnpm / lerna / cargo / go.work / uv workspace 訊號偵測 + 子包清單 |
| Git metadata | **pygit2**（非 subprocess）、recent 100 commits、per-file activity、top-20 檔 blame |
| Sanitize orchestration | Scanner pass = D-015 第一段；失敗進 quarantine 不進 KB |
| 效能 target | 5000 檔 < 30s |

**後續**
- [x] `docs/module-1-scanner.md` 寫成（2026-04-17）
- [ ] `tests/fixtures/scanner/` 五組 fixture
- [ ] 實作按 §十六 工期排（P0 ~2d / P0+P1 ~4.5d）

---

## D-019: Module 2 / Module 5 / Prompts / Dev Setup 文件定案

**狀態**：✅ 文件 v1 完成（2026-04-17，待使用者 review）
**討論日期**：2026-04-17

**脈絡**
使用者同意「你能自己處理所有文件，我們再來 review」→ 一次把剩餘文字產出補齊。

**產出**

| 文件 | 關鍵預設 | 我不決的（待 review） |
|---|---|---|
| `module-2-kb-builder.md` | token window 600/60、Qdrant collection `codebus_{workspace_id}`、content-hash + 0.95 similarity 兩層去重、batch 32、rebuild 預設 drop | embedding 模型 / dim（LLM 供應商 API） |
| `module-5-generator.md` | 每站 ≤ 800 字、每站至少 1 Checkpoint、最多 1 Quiz、重試 3 次、degraded fallback on、結尾接 Q&A 入口 | 長度上限 / degraded 是否預設啟用 |
| `prompts.md` | 五份骨架 + 變數 + 輸出 schema + Sanitize 提醒片段 + versioning | 細節調教交 golden sample |
| `dev-setup.md` | uv + npm + cargo 三路 onboarding（D-026 起 npm 取代 bun）、Qdrant Docker、smoke test 流程、CI job 清單 | LLM 供應商 API .env 模型名、CI provider |
| `docs/README.md` | 文件導覽 + review 順序 + onboarding 順序 + 完成度表 | — |

**後續**
- [x] 四份新 spec + 索引（2026-04-17）
- [ ] 使用者 review 一輪，標註要改的點
- [ ] 實作階段補 TODO review 的 LLM API 細節

---

## D-020: Module 6 介入控制器 — 延後至前端實作階段

**狀態**：✅ 已決方向（2026-04-17）— MVP 功能做，spec 留白到前端動手時再定
**討論日期**：2026-04-17

**脈絡**
README §四 MVP 明列三個介入點：
1. 路線調整（跳過已會的、改變順序）
2. 重新生成（內容不滿意時）
3. 換資料夾重新開始

但 `docs/README.md` §四已把「Module 6 介入控制器 spec」標為「待補」——因為介面契約要在前端開始寫時才定得準，現在寫會過早。

**決策**
- **功能仍在 MVP scope**（使用者能調整路線 / 重生 / 換資料夾）
- **不預先寫 Module 6 spec**——介面在前端實作階段自然浮現
- 已有的底層能力已足以支援：
  - 路線調整：前端改 `route.json` + `progress.json`，無後端變更
  - 重新生成：reuse Generator pipeline（`module-5-generator.md` §二），傳新 context 即可
  - 換資料夾：reuse sidecar `/scan` + `/explore` + `/generate` 三支 API（`sidecar-api.md`）
- 前端實作時若發現需要新 endpoint（例如「只重生某一站」），再回頭補 spec

**決策理由**
1. 介入控制是**前端驅動**的互動，後端只是 reuse 既有 API
2. 現在寫 spec 只是把前端尚未決定的 UX 硬抽象化，容易寫錯重寫
3. 既有的 Explorer / Generator / Scanner spec 已涵蓋所有後端能力，沒有遺漏
4. 避免 over-engineering：MVP 介入點前端自己用 Pinia state + 既有 API 組合即可

**後續**
- [ ] 前端實作階段建立 `docs/module-6-intervention.md`（若有新 endpoint 需求才寫）
- [x] `docs/README.md` §四「Module 6」狀態維持「待補」，已加註 D-020 連結

---

## D-014: Python toolchain — uv

**狀態**：✅ 已決（2026-04-17）
**討論日期**：2026-04-17

**脈絡**
Python sidecar 需 package / venv / lockfile 管理，選工具。

**選項**

| 選項 | 優 | 缺 |
|---|---|---|
| pip + venv + requirements.txt | 最原生 | 慢、無 lockfile、手動 activate |
| poetry | 成熟、lockfile 穩 | 慢、resolver 有時卡住 |
| pdm | 符合 PEP 標準 | 社群較小 |
| **uv** | **Rust 寫、極快（10-100x pip）、單工具三用（venv/install/run）、workspace 支援、lockfile 可重現** | 較新（但 Astral 出品穩定性可接受） |
| conda | 科學計算強 | 重、對純 Python 專案多餘 |

**決策**
選 **uv**。

**理由**
1. 速度顯著（developer onboarding 一次 `uv sync` 秒完）
2. `uv run` 省 activate 步驟，CI / script 單行
3. `uv lock` reproducible，跟 golden sample regression 相容（環境固定）
4. `[tool.uv.workspace]` 支援 monorepo 下多 package（D-013 連動）
5. Rust 寫的剛好跟 Tauri 殼呼應，敘事統一
6. 與 PyInstaller 相容（`uv run pyinstaller`）

**MVP 使用方式**
```bash
# sidecar/ 目錄
uv init --package
uv add fastapi instructor openai pydantic qdrant-client
uv add --dev pytest ruff pyright
uv sync      # 新 dev onboarding
uv run pytest
uv run python -m codebus_agent.api  # 跑 sidecar
```

**後續**
- [ ] `sidecar/pyproject.toml` 用 `uv init --package` 建立
- [ ] `uv.lock` 進 git
- [ ] CI 用 `uv sync --frozen` 確保 lockfile 一致
- [ ] PyInstaller 打包流程驗證（連動 D-001 後續）

---

## D-010: Module 1 / 5 細節補完

**狀態**：✅ 已補進 README（2026-04-17）
**討論日期**：2026-04-17

**脈絡**
Module 1（資料夾掃描）只寫「過濾垃圾檔案」，實際坑很多。Module 5（Markdown Generator）是教材品質一半來源，被兩行帶過。

### Module 1 要處理
- `.gitignore` / `.dockerignore` 繼承
- Binary 偵測（讀前 N bytes 判斷）
- 檔案大小上限（超過跳過或切段）
- 文字編碼（utf-8 / big5 / gbk 嘗試）
- 符號連結、循環參考
- Monorepo 子模組
- 超大檔（lockfile、generated code）特殊處理

### Module 5 要補
- Prompt 架構（站牌結構、檢核題生成、程式碼摘錄策略）
- 長度控制（每站字數上限）
- 輸出格式驗證（`<Quiz>` 格式對、`correct` 欄位存在）
- 重試機制（格式跑掉重生）
- `--plain` 模式（產出無自訂元件版本，給獨立閱讀）

**後續**
- [ ] README Module 1 / 5 擴充
- [ ] 考慮是否開 `docs/modules-detail.md`

---

## 待辦匯總（從所有 D-00X 抽出）

### 需要更新 README
- [x] D-005：加入 DeepWiki / Greptile / Swimm 對照（2026-04-17）
- [x] D-006：評估機制段落（2026-04-17）
- [x] D-009：「本地優先」改精確說法（2026-04-17）
- [x] D-010：Module 1 / 5 擴充（2026-04-17）

## D-021: LLM Token / Cost 追蹤（UsageTracker）

**狀態**：✅ 已決（2026-04-18）— **MVP 加入 `UsageTracker`，統一收集每次 chat / embed / structured 的 token 用量與成本，寫 `token_usage.jsonl` + SSE 即時廣播**

### 脈絡
- D-007 已規劃 cost benchmark，但目前只有 `Usage(prompt_tokens, completion_tokens, cost_usd)` 資料型別散落在 `ChatResponse`，沒人收、沒人聚合
- `Budget(max_tokens=200k)` 只靠 Explorer 內部估算，缺乏跨模組（Judge / Coverage / Generator / Q&A / Embedding）的真實用量
- Demo 時無法秀「這條路線花了 X tokens / $Y」具體數字
- 教訓：若 MVP 不做，D-007 benchmark 要另寫工具撈 provider log，等於重工

### 決策
1. **新增 `UsageTracker`**（與 `ReasoningLogger` 平行的稽核元件）
2. **強制所有 Provider 呼叫都走 tracker**：
   - `chat()` / `chat_structured()` / `chat_stream()` / `embed()` 回傳都帶 `Usage`
   - Embedding 若 provider 沒回 token 數，用 `tiktoken` 本地估算後標 `estimated=true`
3. **`session_id` 範圍定義**（2026-04-18 補充，避免跨模組對不上帳）
   - **一個 `session_id` = 使用者從 open workspace 到 close workspace 的整個生命週期**
   - Scan → kb_build → Explore → Generate → Q&A 全部掛同一個 `session_id`
   - Q&A 單一提問的 `qa_sess_abc` 是 `session_id` 底下的 **sub-session**（對應 `phase=qa`），Q&A 多輪對話 / 重新整理 UI 不換 `session_id`
   - 路線跑一次花多少錢 = `session_total()`（到 Generate 結束為止），不需手動拼湊
4. **落地五件事**：
   - `{workspace}/token_usage.jsonl`（第五層稽核 JSONL）
   - SSE event type `usage_delta`（前端 Agent console 即時顯示）
   - Session 結束算 summary（總 tokens / 總 $ / `by_module` + **`by_phase`** 兩種 breakdown）
   - `phase` ∈ `scan / kb_build / explore / generate / qa`（跨模組統計維度）
   - `ToolContext` 加第 9 欄 `usage_tracker: UsageTracker`
5. **Budget 控制升級**：以 tracker 即時總計取代 Explorer 內部估算；接近上限時 prompt 注入「收斂」訊號
6. **不做**（留 Phase 2+）：
   - 跨 session 累計（只做單 session，session 結束寫 summary）
   - 成本預算上限強制中止（MVP 只顯示、不強制；Budget 仍用 token 數上限）
   - UI dashboard（MVP 印 summary 文字即可）

### 理由
- **成本近零**：約 1-1.5d 工期，對齊 D-007 benchmark 需求
- **Demo 加分**：具體數字比「本地運作省成本」更說服人
- **Budget 真實化**：從估算改成實測，避免跑飛
- **對齊合規**：第五層稽核 JSONL 強化「稽核 trail 可查驗」claim

### 連動更新
- [x] `llm-provider.md §二`：Usage 加 `embed_tokens` / `estimated` 欄位，`embed()` 回 `EmbedResponse`
- [x] `agent-core.md` §新增 UsageTracker class + §十一 Budget 接 tracker + session_id / phase 範圍定義
- [x] `tool-sandbox.md §五`：ToolContext 加第 9 欄 `usage_tracker`，session_id 語意註記
- [x] `sidecar-api.md §四`：加 `usage_delta` event + `usage_summary` 含 `by_phase`
- [x] `security.md §二`：四層 JSONL → 五層 JSONL（加 `token_usage.jsonl`）
- [x] `implementation-plan.md`：第一階段插入步驟 8.5「UsageTracker 骨架」
- [ ] D-007 連動：benchmark 直接讀 `token_usage.jsonl`（實作期補）

---

## D-022: LLM Call Inspector — 全請求 / 回應稽核畫面

**狀態**：✅ 已決（2026-04-18）— **MVP 加入 `LLMCallLogger` 記錄所有 LLM call 的完整 request / response，配合 UI 新分頁展示，作為稽核 trail + Demo 透明度武器**

### 脈絡
- D-021 UsageTracker 只記聚合數字（tokens / cost / module），但使用者（和 stakeholder）會想看「到底送了什麼、LLM 回了什麼」
- Demo 場景：有人質疑「你怎麼證明沒偷打 API / 沒送敏感資料？」→ 直接開 LLM Calls 分頁 + `<REDACTED:*>` placeholder 原樣展示 = 實證透明
- Debug 場景：Agent 決策怪異時，最關鍵的資訊是「那一步實際送 / 收了什麼」——散落在 reasoning_log（agent 視角）與 token_usage（聚合）都不夠

### 決策
1. **新增 `LLMCallLogger`**（與 `UsageTracker` 平行，都掛在 `TrackedProvider` wrapper 內）
2. **第六層稽核 JSONL：`llm_calls.jsonl`**
   - 每筆：`request_id` / `ts` / `session_id` / `module` / `step_id` / `provider` / `model` / `call_type` / `request` / `response` / `usage` / `latency_ms` / `error`
   - `request.messages` 為 **post-Sanitizer Pass 2** 版本（實際 wire payload）
3. **SSE event `llm_call`** — 即時廣播給 UI（list view 不需等檔案 tail）
4. **大小控制**：單筆 100KB / 單 session 50MB，超出截斷標 `truncated:true` 或 rotate 檔名
5. **不保留 pre-sanitize 原文**（D-022 隱私關鍵）：log 就是 wire payload，零額外落地點
6. **UI MVP 範圍**：List view + Detail modal + module/step/model filter；search / export / diff 延後

### 理由
- **不增加隱私面積**：記錄等於已送出的 wire payload，Sanitizer Pass 2 已經過濾，零額外洩漏風險
- **Demo 武器**：比「口頭保證去識別化」有力得多 —— 直接指給 stakeholder 看 placeholder
- **Debug 效率**：問題多數發生在 prompt / response 層；有 raw 資料可重現
- **成本低**：約 1.5d 工期，~90% 在 UI

### 不做（留 Phase 2+）
- 全文 search（MVP 只有 module / step / model filter）
- Export as JSONL / curl 腳本
- Session 間 diff 對照
- Pre-sanitize 原文保留（安全權衡：不值得多開資料面）

### 連動更新
- [x] `llm-provider.md §二`：ChatRequest / ChatResponse 明定序列化格式供 logger 使用
- [x] `agent-core.md §十三`：新增 `LLMCallLogger` 與 `UsageTracker` 並列；TrackedProvider 同時持有兩者
- [x] `sidecar-api.md §四`：加 `llm_call` SSE event
- [x] `security.md §二`：五層 JSONL → 六層 JSONL（加 `llm_calls.jsonl`）
- [x] `implementation-plan.md`：步驟 8.5 擴充為「UsageTracker + LLMCallLogger」；前端階段加步驟 28.5「LLM Calls 分頁」
- [x] `README.md`：Demo checklist 加「LLM Calls 分頁可展示 request/response」項
- [ ] D-020 連動：前端稽核 tab 分頁數改 6（sanitize / sandbox / kb_growth / reasoning / usage / llm_calls），前端期落地

---

## D-023: Topic mode 綁容器資料夾 + 四層誤刪防線

**狀態**：✅ 已決（2026-04-19）— **Topic mode 的 workspace 從 day 1 綁定一個實體容器資料夾**（預設 `~/.codebus/topics/{slug}/`），所有 per-workspace 資源住裡面；以四層防線（modal 明寫 / README.txt / Settings 入口 / 孤兒偵測）防止使用者誤刪。
**實作細節**：見 `workspace-lifecycle.md §三` + `§七`。

### 脈絡
Topic mode 原本只定義抽象的 `workspace_source: { query, seed_urls, domain_allowlist }`（D-002 / `authorization.md §一`），沒說 KB / audit log / tutorials 落地到哪。若分散存（App-level 加 workspace_id 目錄），使用者完全看不到 Topic workspace 存在於磁碟上 —— 誤刪 `~/.codebus/` 會直接帶走所有 topic 的 audit 紀錄，且缺乏「workspace 實體錨點」的心智模型。

### 選項

| 選項 | 優 | 缺 |
|---|---|---|
| A. Topic 不綁資料夾，一切住 App-level 加 workspace_id 目錄 | 使用者零心智負擔 | 誤刪風險高、看不到 workspace 實體、備份/搬家無目標 |
| B. Topic 一律綁容器資料夾，使用者自選位置 | 心智模型最清楚 | 摩擦高（「我只想學 uv 幹嘛選資料夾」） |
| **C. 隱式建 `~/.codebus/topics/{slug}/` 為預設容器，進階可變更** | 低摩擦 + 心智模型統一 + 備份直觀 | 需做誤刪防線 |

### 決策：C + 四層防線

1. **O-01 Modal 明寫容器路徑**（可複製可點擊）
   > 將在 `~/.codebus/topics/uv/` 建立 workspace — 包含知識庫、教材、稽核紀錄。
2. **容器內放 `README.txt`** — 使用者真的打開資料夾第一眼看到警告與正確操作方式
3. **Settings → Workspace 列表** — 正式刪除入口（含二次確認），避免「只能手動刪」
4. **啟動孤兒偵測** — 發現 `~/.codebus/topics/*/` 裡有 `.codebus-workspace.json` 但不在 `workspaces.json` 的，在 R-00 通知使用者

### 連動更新
- [x] 新建 `docs/workspace-lifecycle.md`（本 ADR 的 spec 細節）
- [ ] `sidecar-api.md §三` Phase 2 實作期：`POST /scan` topic mode 的 `workspace_source` 加 `path` 欄位
- [ ] `authorization.md §一` Phase 2：topic mode 授權涵蓋「容器資料夾路徑」而非只是 URL
- [ ] `tool-sandbox.md §三` Phase 2：topic 模式的 `workspace_root` = 容器資料夾路徑
- [ ] R-00 Start Page Design mockup（額度回來再做）

---

## D-024: Workspace 資料分級儲存（App-level / Workspace-level / Pointer）

**狀態**：✅ 已決（2026-04-19）— **三層切分**：App-level 資料在 `~/.codebus/` 根、Workspace-level 資料住 workspace 自己的資料夾內、Folder mode 用 Pointer（repo 根的 `.codebus/pointer.json`）當視覺錨點。
**實作細節**：見 `workspace-lifecycle.md §二`。

### 脈絡
前身：`README.md §六` 的 `codebus-workspace/` 結構暗示把 KB / tutorials 放進使用者 repo；`dev-setup.md` 卻用 `CODEBUS_WORKSPACE=~/.codebus`。兩者不一致，且 folder mode 若直接把 Qdrant storage（幾百 MB）塞進使用者 repo 有三個雷：

1. Git 掃到會卡
2. 污染使用者 repo（.gitignore 要強制）
3. 部分 repo 是 network mount / 唯讀 Docker volume，沒寫入權

### 選項

| 選項 | 優 | 缺 |
|---|---|---|
| A. 一切住 `~/.codebus/`，使用者 repo 零污染 | repo 乾淨 | 使用者看不到「這個 repo 有 CodeBus workspace」、備份不直觀 |
| B. 一切住使用者 repo（`.codebus/` 內） | 搬家直觀、git sync | Qdrant storage 大、污染 repo、唯讀 mount 不能用 |
| **C. 混合：repo 放 pointer、實質資料在 `~/.codebus/workspaces/{id}/`** | 視覺錨點 + 不污染 + 相容唯讀 mount | 兩邊遺失情境要處理（→ D-025） |

### 決策：C + Topic 特例

- **Folder mode**：pointer in repo（`.codebus/pointer.json` < 1KB）+ 實質資料 `~/.codebus/workspaces/{id}/`
- **Topic mode**：容器資料夾 = workspace root，實質資料直接住裡面（見 D-023）
- **App-level**（永遠 `~/.codebus/` 根）：`authorization_audit.jsonl` / `sanitizer.local.yaml` 全域預設 / `workspaces.json` registry / `sanitizer_rules_meta.json`

### 關鍵不變式
1. **Qdrant storage 不進使用者 repo** — 只放輕量 pointer
2. **Workspace-level audit 跟著 workspace 搬家**（folder mode 也搬 `~/.codebus/workspaces/{id}/` 這份）
3. **App-level audit 跨 workspace 所以住 user home**
4. **Pointer 是視覺錨點不是資料本身**

### 連動更新
- [x] 新建 `docs/workspace-lifecycle.md`（本 ADR 的 spec 細節）
- [ ] `README.md §六` 更新資料夾結構說明（目前仍是舊版，實作期前改）
- [ ] `security.md §二` 七層 audit 路徑對齊（六層 workspace-level + 一層 App-level）
- [ ] `.gitignore` 模板：新建 workspace 時自動寫 repo 根 `.codebus/.gitignore`

---

## D-025: Workspace 整合性與遺失恢復策略

**狀態**：✅ 已決（2026-04-19）— **六種遺失情境各自定義修復選項，遵守五條鐵律（不靜默修復 / 不重建 audit / 不自動刪 / 啟動時 integrity check / 孤兒掃描）**。
**實作細節**：見 `workspace-lifecycle.md §七` + `§八`（audit 事件 schema）。

### 脈絡
D-024 的 folder mode 混合策略帶來「pointer + 實質資料兩邊」的代價：任一邊遺失都要有明確處理。Topic mode 雖然單一容器但容器本身也可能被刪。另外 `workspaces.json` registry 與 `authorization_audit.jsonl` 也有獨立遺失風險。

### 六種情境 + 處理（完整表見 `workspace-lifecycle.md §七`）

| 情境 | 處理 |
|---|---|
| A. Pointer 孤（folder） | R-00 卡片標 🔴，三選一：重 scan / 重授權 / 移除 pointer |
| B. 實質孤（folder） | R-00 孤兒通知，三選一：指定新 repo / detached / 刪除 |
| C. Path 不一致（folder repo 搬家） | 開啟時 modal 確認 + 寫 `workspace_path_updated` audit |
| D. Topic 容器遺失 | R-00 卡片標 🔴，從 `topic_seed` 重爬 / 移除 |
| E. Registry 遺失 | Walk 目錄重建 + 寫 `registry_rebuilt` audit |
| F. App-level audit 遺失 | **不補寫**，新檔記 `audit_log_initialized{prior_log_lost:true}` + R-00 全域 warning |

### 五條鐵律

1. **永遠不靜默修復** — 任何不一致在 R-00 讓使用者看到決策點（情境 E 例外但仍寫 audit）
2. **永遠不重建 audit log 內容** — 合規紀錄斷鏈寧可明告也不偷補
3. **永遠不自動刪** — 只標 broken / detached，刪除走使用者 + 二次確認
4. **啟動時 integrity check** — walk `workspaces.json` 驗兩邊健在，broken 標記不 crash
5. **孤兒掃描納入啟動流程** — `~/.codebus/workspaces/*/` + `~/.codebus/topics/*/` 沒在 registry 的通知使用者

### 新增 Audit 事件（寫入 `~/.codebus/authorization_audit.jsonl`）
- `workspace_path_updated` / `registry_rebuilt` / `audit_log_initialized` / `workspace_tombstoned` / `workspace_deleted`

### R-00 卡片健全性 Badge
🔴 `pointer_orphan` / 🟡 `data_orphan` / 🟠 `path_moved` / 🔵 `detached` / ⚫ `tombstone`（使用者刪除後保留 14 天供後悔）

### MVP 範圍
- 必做：情境 A / C / E / F + 墓碑機制
- Phase 2：情境 B 的修復 UX（MVP 只做偵測 + 通知）、跨機器備份匯入匯出

### 連動更新
- [x] 新建 `docs/workspace-lifecycle.md`
- [ ] `security.md §二` 七層 audit 補 App-level `authorization_audit.jsonl` 的新事件
- [ ] `sidecar-api.md` Phase B 實作期加入 `GET /workspaces` / `POST /workspaces/integrity-check` / `POST /workspaces/{id}/tombstone` 等 endpoints
- [ ] R-00 修復頁 Design mockup（額度回來再做）

---

## D-026: Web toolchain — npm（取代 D-019 的 Bun 預設）

**狀態**：✅ 已決（2026-04-19）
**討論日期**：2026-04-19（M1 power-on apply 期間）

**脈絡**
原 `dev-setup.md` 將 Bun 列為 `web/` 預設 package manager（D-019 dev-setup v1）。M1 power-on 實作要 `bun install` 時發現本機未裝 bun，重新評估 toolchain 取捨。

**選項**

| 選項 | 優 | 缺 |
|---|---|---|
| **npm** | **Node 內建零安裝、`package-lock.json` text diff PR friendly、Nuxt3 first-class default、生態 100% 相容** | install 較慢（中型 repo ~30s） |
| Bun | install 顯著快（~5s）、binary 一包多用、Nuxt v3.6+ 官方相容 | `bun.lockb` binary lockfile 對 PR review / git blame 不友善、多一條外部 binary 鏈、極少 native module 偶有問題 |
| pnpm | symlink 節省磁碟、lockfile text diff 友善 | 多一個工具、Tauri 範例多以 npm 為主 |

**決策**
選 **npm**。Bun 從 dev-setup 預設名單移除（保留個人偏好 opt-in 空間，不寫進 spec）。

**理由**
1. CodeBus `web/` 範圍小（Nuxt3 Hello World 殼 + Trust Layer 四站 mockup-driven），bun 的 install 速度優勢感受不強
2. `bun.lockb` 是 binary lockfile，與 Spectra spec-driven 流程偏好「PR 可 review、依賴變動可審計」相衝
3. Node 在 Windows / macOS / Linux 都有官方 installer，省去要使用者額外裝 bun runtime 的入門門檻
4. Tauri 與 Nuxt3 對 npm 都是 first-class default，後續 `cargo tauri build` 與 `npm ci` 在 CI 鏈最直接
5. Bun 的「all-in-one」優勢（runtime + bundler + test）對本專案無需求 — 不在 Node 內跑業務邏輯（業務在 sidecar）、bundler 由 Nuxt 處理、test 用 vitest

**MVP 使用方式**
```bash
# web/ 目錄
npm init -y                     # 或 nuxi init . 後手調 package.json
npm install nuxt @nuxtjs/tailwindcss typescript
npm install                     # 新 dev onboarding
npm run dev                     # http://localhost:3000
npm test                        # vitest
```

**Tauri 整合**：`tauri.conf.json` `build.beforeDevCommand` 改為 `cd ../web && npm run dev`、`build.beforeBuildCommand` 為 `cd ../web && npm run build`。

**後續**
- [x] `dev-setup.md` 把 Bun 行移除、所有 `bun ...` 命令改 `npm ...`（M1 power-on 同 commit）
- [x] `CLAUDE.md` 工具鏈表把 `Bun` 改 `npm`
- [x] `openspec/config.yaml` toolchain 描述把 `Bun` 改 `npm`
- [x] `openspec/changes/m1-power-on/{proposal,design,tasks}.md` 移除 bun.lockb 引用、改 package-lock.json
- [x] D-019 / D-013 後續事項表「dev-setup.md」列 `uv + bun + cargo` 改為 `uv + npm + cargo`
- [ ] 後續若有效能瓶頸或需求改變可重新評估，但需另開 D-XXX 並 bump dev-setup

---

## D-027: Qdrant 走 standalone binary（推翻 m1-power-on D-local-6 的 Docker Compose 預設）

**狀態**：✅ 已決（2026-04-19）— **M1 起以 Qdrant 官方 standalone binary 為主路徑；Docker Compose 降為 fallback**。Binary 由使用者獨立下載放 known path，**不** 打包進 PyInstaller。

### 脈絡
`openspec/changes/m1-power-on/design.md` D-local-6 原選 Docker Compose，理由有二：
1. Embedded rust binary 打包到 PyInstaller 約 +100 MB，跨平台還要多份
2. Qdrant cloud 違背 D-009「本地優先」claim

實作期才暴露的代價：Docker Desktop 在 Windows / macOS 上入門門檻不低（~700 MB+ 安裝、需啟 WSL2、商用授權非免費）。「插好電」的 M1 驗收卡在裝 Docker，與「降低上手門檻」的專案定位衝突。

重新評估後發現 D-local-6 把「embed into PyInstaller」與「由使用者本機運行」綁在一起討論，是錯誤的二分。第三條路：**使用者獨立放 binary（非 embed）+ sidecar 以 HTTP 連線（與啟動方式解耦）** 同時解掉 embed 體積與 Docker 門檻兩個問題。

### 選項

| 選項 | 優 | 缺 |
|---|---|---|
| A. Docker Compose（原 D-local-6） | 版本隔離乾淨、一指令拉起 | 使用者需裝 Docker Desktop（Windows / macOS ~700 MB + WSL2） |
| B. Embedded rust crate 打包進 PyInstaller | 使用者零裝設 | PyInstaller 體積 +~100 MB × 平台份數；Qdrant 升版綁 app 升版 |
| **C. Standalone binary 由使用者本機放 known path** | 無 Docker 門檻、體積不進 PyInstaller、升版獨立 | 第一次啟動要走「下載解壓」一次性步驟 |

### 決策：C + Docker Compose 作 fallback

- **主路徑**：使用者從 Qdrant 官方 release 下載對應平台 binary（Win `.zip` / mac `.tar.gz` / Linux `.tar.gz`），解壓到 `~/.codebus/bin/qdrant(.exe)` 或 `$CODEBUS_QDRANT_BIN` 指向之路徑
- **啟動腳本**：`sidecar/scripts/start-qdrant.{ps1,sh}` 在 foreground 跑 binary 指向 `~/.codebus/kb/`
- **Fallback**：`sidecar/docker-compose.qdrant.yml` 保留，`dev-setup.md` 列為 CI / advanced 選項
- **sidecar 側**：透過 `CODEBUS_QDRANT_URL`（預設 `http://127.0.0.1:6333`）連線，**與啟動方式完全解耦** — binary 跟 Docker 二擇一皆可

### 關鍵不變式

1. **Qdrant binary 不進 PyInstaller** — 使用者獨立管理，升版不綁 app 升版
2. **Qdrant binary 不進 git repo** — 只進 `.gitignore`；README / dev-setup 列下載指引
3. **sidecar 只看 HTTP endpoint** — 啟動來源（binary / compose / 遠端 dev Qdrant）對 sidecar 透明
4. **Compose 檔仍存在** — CI 以及偏好 Docker 的 advanced user 路徑不斷
5. **health check 仍檢 Qdrant 連通** — `GET /healthz` 的 `dependencies.qdrant` 欄位不因啟動方式改變而刪

### 連動更新

- [x] `openspec/changes/m1-power-on/design.md` D-local-6 翻轉主路徑、把 Docker Compose 降為 fallback
- [x] `openspec/changes/m1-power-on/specs/qdrant-client/spec.md` SHALL 條款改「啟動腳本 + binary 路徑」為主、compose 為備
- [x] `openspec/changes/m1-power-on/tasks.md` Phase 5 task 改走 binary
- [ ] `docs/dev-setup.md` 新增 Qdrant binary 下載 / 解壓 / 啟動段落，Docker compose 降為「備援」章節
- [ ] `docs/module-2-kb-builder.md §三` 提到 Qdrant 的句子改中性（HTTP endpoint），不綁 Docker
- [ ] `.gitignore` 補 `*.qdrant-bin/` / `bin/qdrant*` 模板
- [ ] README workspace 結構圖補 `~/.codebus/bin/` 目錄

---

## D-028: LLM Vision 能力延後至 Phase 2（MVP 不做、但介面保留 additive 擴充空間）

**狀態**：✅ 已決（2026-04-20）— **MVP 不實作 vision，但 Scanner 已保留圖片 metadata、Provider 介面未來可 additive 擴充**。

### 脈絡
討論 `ProviderRole` 路由時（見 llm-provider.md 即將更新版本）延伸出「LLM 是否要看得懂圖」問題。列出 call site 後發現：
- Scanner（Module 1）已把 `.png .jpg` 等圖片歸 `binary`，`content: None`，不進 KB、不送 LLM
- Explorer / Judge / Q&A / Generator 所有 call site 輸入都是 **code + text markdown**，沒有一個實際需要看圖
- 使用者不會上傳圖——只指一個資料夾

唯一「可能」需要 vision 的情境：repo 有 `docs/architecture.png` 這類純圖檔，Generator 若要替學生「解讀」該圖，則需看得到內容。但此為**非核心學習循環的延伸**，MVP 不值得現在挖坑。

### 選項

| 選項 | 優 | 缺 |
|---|---|---|
| A. MVP 就做 vision（Capability probe + Provider capability flag + Generator 雙模式） | 未來無需改動 | 工期 +1.5~2d、複雜度高、實際用到的 call site 僅 Module 5 部分情境 |
| B. MVP 不做 vision，**介面也不預埋** capability 機制 | 工期省、複雜度低；未來要做時 Provider Protocol 是 additive 擴充（加屬性 / 參數）不 breaking | 未來補做要動 spec + 實作 + 測試約 1.5~2d |
| C. MVP 介面預埋 `Capability` enum，但不實作 vision | 介面「看起來」完整 | YAGNI；未實際使用的抽象容易設計錯；未來還是要改 |

### 決策：B — MVP 不做、介面不預埋

**主要理由**：
1. **Scanner 已保留資料**：圖片的 `path / size / sha256` 記在 `ScanResult` entry 裡（`kind: "binary"`），未來要處理不需要回頭重掃
2. **Provider 介面擴充 additive**：未來在 `LLMProvider` Protocol 加 `supports_vision: bool` 屬性、`chat()` 加 `images: list[ImageInput] | None = None` 參數，舊呼叫端不動、新呼叫端才用
3. **Role-level config 擴充 additive**：未來在 `roles.reasoning` 加 `"vision": true` 旗標，沒設就維持純文字
4. **僅 Generator 需要改**：未來實作時只有 Module 5 Generator 的「引用圖片那一站」要動，其他 call site 皆不碰
5. **Tutorial 引用圖不需要 vision**：Generator 在 markdown 寫 `![Arch](docs/arch.png)` 讓前端渲染 → 學生自己看圖。這條路 MVP 已可行

### 關鍵不變式

1. **Scanner 不丟圖片 metadata** — 即使 `content: None`，entry 仍保留 path / size / sha256，為未來 vision 保留入口
2. **Provider Protocol 不預埋 Capability enum** — 真要做 vision 時才加，避免未實際使用的抽象設計錯
3. **未來加 vision 是 additive 改動** — 所有既有 call site 不受影響，僅 Module 5 Generator 擴充雙模式（純文字 / 帶圖）
4. **Vision 不影響 Sanitizer 不變式** — 圖片內容送 LLM 前仍須經 Sanitizer Pass 2；屆時需決定「圖片 PII 如何 scrub」（blur 臉 / OCR 後 regex / 拒送），此為 Phase 2 子議題

### 連動更新（MVP 階段 — 現在只需做這些）

- [ ] `docs/llm-provider.md` §八 MVP 不做表補一行「Vision / 多模態 — 延後至 Phase 2，見 D-028」
- [ ] `docs/module-5-generator.md` 引用圖片段落明寫「MVP 只 inline markdown `![]()` 相對路徑，不對圖做 LLM 解讀」

### Phase 2 觸發條件（到那時再開新 change）

- 使用者反饋「希望 tutorial 能幫我看懂架構圖」
- 目標 repo 大量依賴純圖檔文件（罕見，多數 OSS 會用 mermaid）
- 或 Provider 原生支援 vision 成本降到與純文字同量級

### 預估未來工期（真要做時）

- Protocol 擴充（`supports_vision` + `images` 參數）：0.5d
- Module 5 Generator 雙模式：0.5d
- Sanitizer 圖片處理策略決策 + 實作：0.5d
- 測試 + 稽核 log 擴充：0.5d
- **總計：約 2d**

---

### 需要決策動作
- [x] D-001：定技術棧（混合架構）（2026-04-17）
- [x] D-003：決定 Ollama 路徑（Provider 抽象 + 只接指定 LLM 供應商 API）（2026-04-17）
- [x] D-004：選定 demo repo + task（Timeline + Google Drive Adapter）（2026-04-17）
- [x] D-008：First-run UX 方向（2026-04-17）
- [x] D-012：Agent 框架選型（自寫 + Instructor）（2026-04-17）
- [x] D-013：專案組織（monorepo + 目錄分層，不用 submodule）（2026-04-17）
- [x] D-014：Python toolchain 選 uv（2026-04-17）
- [x] D-015：Sanitizer 規則與架構（2026-04-17，待使用者 review）
- [x] D-016：Q&A Agent + 持續成長 KB 進 MVP（2026-04-17）
- [x] D-017：Tool Sandbox 集中 spec（2026-04-17）
- [x] D-018：Module 1 Scanner 細節定案（2026-04-17）
- [x] D-019：Module 2 / 5 / Prompts / Dev Setup 文件 v1（2026-04-17，待 review）
- [x] D-020：Module 6 介入控制器延後至前端實作階段（2026-04-17）
- [x] D-021：LLM Token / Cost 追蹤（UsageTracker，第五層稽核 JSONL）（2026-04-18）
- [x] D-022：LLM Call Inspector（全 request/response 稽核，第六層 JSONL + UI 分頁）（2026-04-18）
- [x] D-023：Topic mode 綁容器資料夾 + 四層誤刪防線（2026-04-19）
- [x] D-024：Workspace 資料分級儲存（App-level / Workspace-level / Pointer）（2026-04-19）
- [x] D-025：Workspace 整合性與遺失恢復策略（六情境 + 五鐵律 + R-00 badge）（2026-04-19）
- [x] D-026：Web toolchain 改 npm 取代 Bun（2026-04-19）
- [x] D-027：Qdrant standalone binary 取代 Docker Compose 為主路徑（2026-04-19）
- [x] D-028：LLM Vision 能力延後至 Phase 2（MVP 不做、介面不預埋 Capability enum）（2026-04-20）

### 需要 spec 動作（實作前再做）
- [ ] D-008：三階段進度元件 Vue spec（Vue 實作）
- [x] D-008：progress event schema（寫進 `docs/sidecar-api.md` §四，2026-04-17）
- [x] D-003：LLM Provider 抽象介面定義（`docs/llm-provider.md`，2026-04-17）
- [x] Python sidecar HTTP API schema（`docs/sidecar-api.md`，2026-04-17）
- [x] D-011：README 加資安章節指向 security.md（2026-04-17）

### 需要執行動作
- [x] D-004 連動：起草 golden sample ideal route（`tests/golden/timeline-gdrive-adapter/ideal-route.md`，2026-04-17，待 Harry review）
- [ ] D-007：cost benchmark（實作到一定程度後）
- [ ] D-001：PyInstaller 打包流程驗證
