# 🚌 CodeBus 產品規劃與 Spec

> 「XX 哥哥，我們來接你了呦～來囉來囉～」
> 新人工程師的 Agentic AI 學習夥伴

---

## 一、產品定位

### 一句話說明

CodeBus 是一個**桌面 AI 應用**，幫助工程師快速上手陌生的程式碼專案或技術領域。使用者選擇資料夾或輸入主題，Agent 會自動分析內容、根據任務規劃客製化學習路線，產出結構化的 Markdown 教材，並以互動介面呈現。

### 產品 Slogan

**「給它目的地，它帶你上車」**

### 目標使用者

- 剛加入專案需要快速上手的 RD 工程師
- 需要跨部門支援、面對陌生 codebase 的資深工程師
- 想學習新技術領域但不知從何下手的學習者
- 企業內部負責 onboarding 的技術主管

---

## 二、解決的痛點

### 核心痛點

**工程師被丟到陌生的 codebase 或技術領域時，沒有好的學習路徑**

- 讀文件：不知道順序、重點、跟自己任務的關聯
- 問人：打擾同事、答案零散、不系統
- 靠直覺翻 code：效率低、容易漏掉關鍵模組
- 搜尋教學：資訊爆炸、難以判斷品質和順序

### 量化問題

業界數據顯示，工程師上手新 codebase 平均要 **4-6 週**。即使有 AI 輔助，也要 **1-2 週**才能開始產出。對大型企業來說，這是顯著的人力成本。

---

## 三、核心價值主張

### 與現有工具的差異化

| 功能 | NotebookLM | DeepWiki | Code2Tutorial | Greptile | Copilot | Swimm | **CodeBus** |
|---|---|---|---|---|---|---|---|
| 讀本地 codebase | ❌ 需上雲 | ❌ 公開 repo | ❌ 只吃 GitHub | ⚠️ 雲端索引 | ✅ | ✅ | ✅ |
| 程式碼結構分析 | ❌ 當純文字 | ✅ | ✅ | ✅ | ✅ | — | ✅ |
| **任務導向動態路線** | ❌ | ❌ 靜態 wiki | ❌ | ❌ 偏 Q&A | ❌ | ❌ 人工設計 | ✅ |
| **Agent 決策可視化** | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 本地知識庫持久化 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 互動式分站學習 | 部分 | ❌ 靜態 | ❌ 靜態 .md | ❌ | ❌ | ✅ | ✅ |
| 教材自動生成 | ⚠️ summary | ✅ | ✅ | ❌ | ❌ | ❌ 人工寫 | ✅ |
| 程式碼不落地雲端 | ❌ | ❌ | ❌ | ⚠️ | ⚠️ | ⚠️ | ✅ |
| Topic 通用學習 | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅（Phase 2） |

### 四大差異化賣點

**1. Agentic 探索式讀 code（核心差異化）**
不是無腦 chunk 全掃的 RAG，而是 **Explorer Agent** 像工程師辦案：grep 找入口 → 看到重要 import 自己決定追進去 → 關鍵字查不到換詞再試 → 產路線後 self-review 發現 gap 自己補查。每步決策都印出 thought，Demo 時可視化 Agent 思考過程。詳見 `docs/agent-explorer-spec.md`。

**2. 任務導向客製路線**
同一個 repo，不同任務產出不同學習路線。「我要改 OTA」跟「我要修 Bug」走不同的站牌。

**3. 程式碼與產出本地處理**
桌面 App，codebase 與產出的教材、進度、知識庫索引均存本地，不上雲；LLM / Embedding 透過 API 呼叫（Phase 2 可選自架 / 本地模型 Ollama）。code 本身不落地第三方雲端，適合企業機密專案。

**4. 持久化且會成長的知識庫**
每個專案用 Qdrant 建立**獨立**的本地向量資料庫，持久化累積、不是 session based；同一專案反覆開啟仍然是同一份 KB。**Q&A Agent（Module 8）在使用者問答時會判斷新資訊是否值得沉澱，自動把新發現加進 KB**——KB 不是凍結快照，是活的。（跨專案共用索引留 Phase 3 評估，詳見 `docs/module-2-kb-builder.md`）

**5. Markdown + 互動介面雙軌**
產出的 .md 可以獨立閱讀、分享、放 Wiki。也可以在 App 內用互動介面學習、追蹤進度。

---

## 四、功能規劃

### Phase 1 — MVP（必須完成）

**核心流程**

- [ ] 選擇本地資料夾作為學習來源
- [ ] 自動判斷內容類型（程式碼、技術文件、混合）
- [ ] 掃描並讀取資料夾內容
- [ ] 用 LLM embedding 建立本地向量知識庫（Qdrant）
- [ ] 使用者輸入任務目標
- [ ] Agent 分析知識依賴、規劃客製化學習路線
- [ ] 產出結構化 Markdown 教材
- [ ] 產出 route.json 描述路線結構
- [ ] 前端解析並以互動介面呈現教材
- [ ] 站牌狀態管理（待解鎖 / 閱讀中 / 已完成）
- [ ] Q&A Agent：教材後可繼續問問題，Agent 走 RAG + 必要時補查並自動累積 KB

**介入點**

- [ ] 路線調整（跳過已會的、改變順序）
- [ ] 重新生成（內容不滿意時）
- [ ] 換資料夾重新開始

**品質保證（Golden Sample 評估機制）**

- [ ] 2 個熟悉 repo + 3 個任務，人工寫 ideal route 當 benchmark
- [ ] 自動評分：核心檔案召回率 / 雜訊率 / 依賴完整度
- [ ] 人工 rubric（1-5 分）：順序合理性 / 站牌粒度 / 新人可讀性
- [ ] 每次改 prompt 跑 regression，避免品質退步

### Phase 2 — 完整版（時間充裕再做）

- [ ] Topic 模式（Agent 自己找網路資料）
- [ ] 知識庫管理介面（新增 / 刪除來源）
- [ ] 每站嵌入相關 YouTube 教學影片
- [ ] 文件模式切換（投影片 ↔ scroll；`<CodeRef>` / `<Reveal>` 元件，interactive-tutorial.md §九 P1）

> 註：`<Checkpoint>` / `<Quiz>` / `progress.json` 已收進 Phase 1 MVP（`interactive-tutorial.md §九 P0`），不再列於此。

### Phase 3 — 未來規劃（MVP 後）

- [ ] LLM 判題（使用者回答 → Agent 判對錯 + 給回饋）
- [ ] TTS 語音版本
- [ ] 多語言教材產出
- [ ] 團隊協作（分享學習路線、追蹤新人進度）
- [ ] IDE Plugin 整合

---

## 五、系統架構

### 技術棧（混合架構：Tauri 殼 + Python Sidecar）

| 層級 | 技術 | 說明 |
|---|---|---|
| 桌面框架 | Tauri 2.0 | Rust 殼 + 系統整合 + 前端渲染 + sidecar 生命週期管理 |
| 前端 | Nuxt3 + TypeScript | SPA，借鑑 Timeline PWA 架構 |
| UI 樣式 | Tailwind CSS | 深色主題，Vercel / Linear 風格 |
| Markdown 渲染 | @nuxtjs/mdc | 支援程式碼高亮、YouTube 嵌入、Mermaid |
| 狀態管理 | Pinia | — |
| Tauri Commands（Rust） | Rust | 檔案系統、對話框、sidecar 啟停、進度事件 emit |
| **Agent 核心（Sidecar）** | **Python** | Module 1 / 2 / 4 / 5 / 8；Explorer / Q&A Agent 全部邏輯 |
| Agent 實作方式 | 自寫 ReAct loop + Instructor | ReAct / Judge / Coverage 自寫（展示 agentic 原理）；structured output 借 Instructor + Pydantic schema，詳見 D-012 / `docs/agent-core.md` |
| 向量資料庫 | Qdrant | Python client；本地持久化到 `kb/` |
| LLM / Embedding | 外部 LLM API（OpenAI / Claude 等） | OpenAI 或 Claude（Phase 2 可選 Ollama 本地模型） |
| IPC | HTTP（localhost） | Tauri ↔ Python sidecar，JSON schema 明確 |
| Python toolchain | uv | venv / install / run / workspace 一站式，Rust 寫、極快 |
| 專案組織 | Monorepo + 目錄分層 | `tauri/` / `sidecar/` / `web/` / `tests/`；不使用 git submodule |
| 打包 | Tauri + PyInstaller | Python runtime 內嵌進 installer，使用者端零依賴 |
| **長期演進** | 逐步 port 到 Rust | Sidecar 介面固定，Phase 3+ 模組逐個改 Rust + Rig |

### 為什麼選混合架構

- **Rust 學習風險降低**：Agent 邏輯用 Python（主力語言），AI 輔助產出我們也能 review
- **生態成熟度**：Python 有 openai-python / Instructor / Pydantic 全部穩定；Rig 生態新、AI 訓練資料少、坑難排
- **迭代速度**：Python 秒級改測；Rust 編譯 10-30 秒 × 百次 debug 會吃掉工期
- **Rust 還是有學到**：Tauri 殼那層有 ownership / trait / async 基本功
- **升級路徑開放**：Sidecar HTTP API 定死，之後可以一個模組一個模組換成 Rust，不用重寫全部

### 模組拆解

**Module 1：資料夾掃描器（Folder Scanner）**
- 輸入：資料夾路徑
- 做的事（摘要，完整 spec 見 `docs/module-1-scanner.md`）：
  - `.gitignore` / `.dockerignore` / `.codebusignore` 階層繼承
  - Binary 偵測（副檔名黑名單 + 前 8KB null byte + 非可印字符比例）
  - 編碼偵測（utf-8 → utf-16 → big5 → gbk → shift_jis → charset-normalizer）
  - 符號連結預設不跟隨（sandbox 驗證 resolve 後目標仍在 workspace）
  - Monorepo 偵測（pnpm / lerna / cargo / go.work / uv workspace）+ 子包清單
  - Lockfile / generated 只記 metadata 不讀內容，不進 KB
  - Git metadata 透過 **pygit2**（非 subprocess，符合 Sandbox §十一）
  - **Sanitizer 第一段**：每個 text 檔 scrub 後才進 KB（D-015）
  - **Content-type summary**：輸出 code/docs/config 比例，讓 Explorer 知道 repo 性質
- 輸出：`ScanResult` Pydantic schema（給 Module 2 的唯一輸入）

**Module 2：知識庫建構器（Knowledge Base Builder）**
- 輸入：檔案內容
- 做的事：文字切 chunk → Embedding → 存進 Qdrant
- 輸出：可查詢的向量資料庫

**Module 3：Topic Explorer Agent（Phase 2）**
- 輸入：Topic 關鍵字 + 使用者程度
- 做的事：**與 Module 4 共用同一個 ReAct loop**，只是換工具集與 Judge prompt——web_search / fetch_page / search_docs / search_youtube / evaluate_source / add_subtopic / mark_material
- 特有決策：source 優先序（官方文件 vs 部落格）、品質判斷（權威性 / 時效）、衝突處理、陌生術語 → 子主題
- 架構 day 1 就抽象好（Python `Protocol` / ABC：`ExplorerTools` / `Judge` / `CoverageChecker`；未來 port 到 Rust 時對應 trait 不變），Phase 2 加 Topic 模式不改核心
- 完整 spec：`docs/agent-explorer-spec.md` 第十二章

**Module 4：Explorer Agent（探索式分析引擎）⭐ Agentic 核心**
- 輸入：codebase + 任務描述
- 做的事：**像工程師探案**——grep 找入口 → 讀檔 → Relevance Judge 評估 → 決定追 import / 找 callers / 換關鍵字 → Coverage Checker 檢核 gap → 不滿意自己補查
- 決策迴圈：Think → Act（工具呼叫）→ Judge → Update state → Converge check
- 三層 Agent：Explorer（主決策）/ Relevance Judge（讀後評估）/ Coverage Checker（收斂檢核）
- 可用工具：search / list_dir / read_file / trace_import / find_callers / mark_station / add_to_queue / stop
- 輸出：學習路線規劃 + `reasoning_log.jsonl`（每步決策紀錄）
- 完整 spec：`docs/agent-explorer-spec.md`

**Module 5：教材生成器（Markdown Generator）**
- 輸入：Explorer Agent `stations` + KnowledgeBase
- 做的事（摘要，完整 spec 見 `docs/module-5-generator.md`）：
  - **每站獨立 LLM call**（不一次產整份，失敗只重跑單站）
  - 互動元件輸出 `<Checkpoint>`（每站至少 1）/ `<Quiz>`（每站最多 1，`correct` 欄位必填）
  - 格式驗證管線（長度 ≤ 800 字、code block ≤ 30 行、`<CodeRef>` 路徑需在 workspace）
  - 重試 3 次仍失敗 → **degraded fallback**（精簡版 + 標 `degraded:true`，不讓整份垮）
  - `--plain` 模式拔掉自訂元件，產 Wiki / GitHub 預覽友善的純 markdown
  - 結尾接 Q&A Agent 入口（Module 8，D-016）
- 輸出：`tutorial.md` + `route.json` + `generator_log.jsonl`

**Module 6：介入控制器（Intervention Handler）**
- 做的事：處理使用者介入（調整路線、重新生成等）

**Module 7：前端互動層（Nuxt3）**
- 做的事：解析 Markdown + route.json、站牌列表、內容渲染、進度追蹤

**Module 8：Q&A Agent（對話式問答 + KB 自動成長）⭐ Agentic 延續**
- 輸入：使用者在教材後輸入的問題
- 做的事：先走 RAG 查 KB → 不足則用 Explorer 同組 read-only tools 即時補查（workspace 內）→ Agent 判斷「值得沉澱」則呼叫 `add_to_kb` 把新 chunk（過 Sanitizer）加進 Qdrant
- 與 Explorer 共用 ReAct core / tools（trait 抽象），只換 prompt + 加 `kb_search` / `add_to_kb`
- Demo 金句：「問它教材沒教的細節，它自己去找並記起來」
- 輸出：自然語言答案 + 引用（file:line）+ KB growth 稽核事件
- 完整 spec：`docs/qa-agent.md`

### 資料流

```
選資料夾
  ↓
[Module 1] 掃描內容
  ↓
[Module 2] 建立向量知識庫（Qdrant）
  ↓
[介入點 A：知識庫管理]
  ↓
輸入任務描述
  ↓
[Module 4] Agent 分析 + 路線規劃
  ↓
[介入點 B：路線調整]
  ↓
[Module 5] 產出 tutorial.md + route.json
  ↓
[Module 7] 前端渲染互動介面
  ↓
使用者學習
  ↓
[介入點 C：產出後調整]
  ↓
下車（完成）或 重新上車
```

---

## 六、檔案結構與資料格式

### 專案資料夾結構（D-024）

CodeBus 資料分三層（完整規格見 `docs/workspace-lifecycle.md`）：

```
~/.codebus/                                  ← App-level（跨 workspace）
├── authorization_audit.jsonl                授權 + workspace 生命週期 audit
├── sanitizer.local.yaml                     全域 sanitizer 預設
├── workspaces.json                          workspace registry
├── topics/                                  Topic mode 容器的家（Phase 2）
│   └── {slug}/                              容器 = workspace root
└── workspaces/                              Folder mode 實質資料的家
    └── {workspace_id}/
        ├── kb/                              Qdrant 向量儲存
        ├── tutorials/{task-id}/
        │   ├── tutorial.md                  學習教材（乾淨 Markdown）
        │   ├── route.json                   路線結構
        │   └── progress.json                學習進度
        ├── sanitize_audit.jsonl             ┐
        ├── tool_audit.jsonl                 │
        ├── kb_growth.jsonl                  │ workspace-level 六層 audit
        ├── reasoning_log.jsonl              │
        ├── token_usage.jsonl                │
        ├── llm_calls.jsonl                  ┘
        └── .codebus-workspace.json          metadata（含 origin_path 反指回 repo）

{使用者的 repo}/                              ← Pointer（Folder mode only）
└── .codebus/
    ├── pointer.json                         { workspace_id, type } — 視覺錨點
    └── .gitignore                           預設 ignore 自己
```

**關鍵不變式**：Qdrant storage 不進使用者 repo（相容 network mount / 唯讀 volume）、Topic workspace 搬家搬容器即完、Folder workspace 搬家需 pointer + 實質資料兩處。

### tutorial.md 格式（給人讀的）

````markdown
# OTA 更新功能 - 學習教材

## 🚏 站 1: MQTT 基礎

MQTT 是一種輕量級的訊息協定，常用於 IoT 設備...

```python
import paho.mqtt.client as mqtt
```

<Checkpoint id="station-1-check">
- [ ] 能說出 MQTT 的 QoS 三個級別
- [ ] 知道這個專案用的是哪個 broker
</Checkpoint>

<Quiz id="s1-q1" correct="b">
MQTT QoS 1 代表：
- a) 最多送一次
- b) 至少送一次
- c) 剛好送一次
</Quiz>

## 🚏 站 2: OTA Manager
...
````

> `<Checkpoint>` / `<Quiz>` 是 MDC 元件，Module 7 前端會掛對應 Vue 元件；`--plain` 模式（module-5-generator.md §六）會退回純 markdown 版本供 Wiki / GitHub 預覽。

### route.json 格式（給前端用的）

```json
{
  "title": "OTA 更新功能",
  "task": "修改 OTA 功能",
  "source_type": "folder",
  "source_path": "/path/to/project",
  "estimated_minutes": 120,
  "stations": [
    {
      "id": 1,
      "title": "MQTT 基礎",
      "duration": 20,
      "prerequisites": [],
      "markdown_anchor": "站 1: MQTT 基礎",
      "related_files": ["src/mqtt/client.py"]
    },
    {
      "id": 2,
      "title": "OTA Manager",
      "duration": 30,
      "prerequisites": [1],
      "markdown_anchor": "站 2: OTA Manager",
      "related_files": ["src/ota/manager.py"]
    }
  ]
}
```

### progress.json 格式（前端寫入）

```json
{
  "current_station": 2,
  "completed": [1],
  "started_at": "2026-04-17T10:30:00",
  "last_active": "2026-04-17T11:45:00",
  "notes": {
    "1": "已看過，MQTT 部分再複習"
  }
}
```

---

## 七、UI/UX 設計

### 視覺風格

- 深色主題（Vercel / Linear 風格）
- 背景 zinc-950、卡片 zinc-900、文字 zinc-300
- 點綴色依站牌類型決定（基礎 / 核心 / 進階）

### 主要畫面

**1. 首頁 — 選擇模式**
```
┌─────────────────────────────────┐
│  🚌 CodeBus                     │
│  給它目的地，它帶你上車           │
│                                 │
│  ┌─────────────┐  ┌──────────┐  │
│  │ 📁 選資料夾  │  │ 📝 輸入  │  │
│  │             │  │   主題   │  │
│  └─────────────┘  └──────────┘  │
└─────────────────────────────────┘
```

**2. 知識庫建立中**
顯示掃描進度、已處理檔案數、embedding 進度

**3. 任務輸入 + 路線確認**
Agent 產出路線後，使用者可確認或調整

**4. 學習介面（借鑑 Timeline PWA）**
```
┌─────────────────┬────────────────────────┐
│ 站牌列表         │ 當前站牌內容            │
│                 │                        │
│ 🚏 1 ✅         │ # 🚏 站 2: OTA Mgr    │
│ 🚏 2 👀 ←在這   │ ⏱️ 30 min              │
│ 🚏 3 🔒         │                        │
│ 🚏 4 🔒         │ ## 重點摘要            │
│                 │ ...                    │
│                 │                        │
│                 │ ## 檢核站              │
│                 │ [ ] 1. ...             │
│                 │                        │
│                 │ [✓ 完成這站]           │
└─────────────────┴────────────────────────┘
```

### 互動行為

- 點站牌：切換右側內容（已完成可回看，未解鎖不能點）
- 完成這站按鈕：標記為 ✅ + 解鎖下一站
- 檢核題 Checkbox：純前端互動，幫使用者自我確認
- 進度持久化：自動存到 progress.json

---

## 八、商業化評估

### 對企業內部的價值

**人力成本節省**
- 新人 onboarding 從 4-6 週縮短到 1-2 週
- 跨部門支援工程師上手時間大幅降低
- 資深工程師不用花時間帶新人

**知識資產化**
- 每個專案的學習路線可重複使用
- 減少「老員工離職帶走知識」的風險
- 標準化 onboarding 流程

**隱私合規**
- 本地運作，機密 code 不上雲
- 符合企業資安政策

### 潛在擴展場景

- 工程師 onboarding（主要場景）
- 跨部門技術支援
- 產品線知識傳承
- 客戶技術文件學習
- 新技術評估與導入

---

## 九、開發時程規劃

> 以下為高層 5 階段；**跨模組依賴與 30 步細項順序、里程碑檢核點** 見 [`docs/implementation-plan.md`](docs/implementation-plan.md)。

### 第一階段：基礎建設（1-2 週）

- Tauri 2.0 + Nuxt3 整合跑起 Hello World
- 學會 Tauri invoke command（觸碰 Rust 基本語法）
- Python sidecar 模板建立（FastAPI，D-001 / D-014）
- Tauri ↔ Python HTTP IPC 跑通（最小 ping）
- PyInstaller 打包測試（確認 Python runtime 可內嵌）

### 第二階段：AI 串接（1-2 週）

- Python LLM client（LLM 供應商 API）+ Instructor structured output 串好（非 LangChain，詳見 D-012）
- Qdrant 本地跑起來 + Python client
- 能跑通 embedding → 存 Qdrant → 查詢完整鏈

### 第三階段：核心功能（3-4 週）

- Module 1：資料夾掃描（Python）
- Module 2：知識庫建構（Python + Qdrant）
- **Module 4：Explorer Agent**（Python，ReAct loop + 三層 Agent + 工具集）
- Module 5：Markdown 教材生成 + 互動元件輸出
- **Module 8：Q&A Agent**（Python，ReAct core reuse + `add_to_kb`，D-016）
- Golden sample 評估機制建立

### 第四階段：前端與互動（2 週）

- Nuxt3 UI 開發（借用 Timeline PWA）
- Markdown 解析 + 站牌渲染
- 進度追蹤 + 介入點實作

### 第五階段：打磨與 Demo 準備（1-2 週）

- 反覆調 prompt 品質
- 準備 demo 素材（選一個熟悉專案）
- 跨平台測試（Windows / Linux）
- 打包測試
- 準備簡報與 demo 腳本

---

## 十、風險與對策

| 風險 | 對策 |
|---|---|
| Rust + Tauri 學習曲線陡 | 採混合架構：Agent 邏輯寫 Python（主力語言），Rust 只碰 Tauri 殼 |
| Python sidecar 打包複雜 | 前期就驗證 PyInstaller 流程；週週打包一次不要累積 |
| 升級到全 Rust 工期不明 | Sidecar HTTP API 定死，Phase 3+ 模組逐個遷移，任何時間點都能暫停 |
| Module 4 分析品質不穩定 | 加 fallback 機制（按目錄結構排序保底） |
| LLM 產出格式不穩 | Prompt 嚴格約束 + 格式驗證 + 重試機制 |
| 大型資料夾爆 context | Chunking 策略 + 語意搜尋，不全讀 |
| 跟 NotebookLM 重疊 | 收窄強調 Code 分析 + 本地隱私差異化 |
| 跨平台相容問題 | 每週在 Windows 驗證一次 |
| Demo 視覺平淡 | 用「上車舞」梗製造記憶點 |

---

## 十一、資安與合規

CodeBus 遵循一般性 Agentic AI 安全規範（低權限沙箱、不接觸外部系統、不開放服務埠對外、敏感資料去識別化、稽核 trail 可查驗）。完整 checklist 與實作細節見 `docs/security.md`。

### 設計原則（兩層獨立防線）
- **Tool Sandbox**（見 `docs/tool-sandbox.md`）：Agent 只能在 `workspace_root` 子樹讀檔；**完全沒有寫 filesystem 的 tool**、**完全沒有 shell / exec / subprocess**；git metadata 走 pygit2（C binding，非 subprocess）
- **Sanitizer**（見 `docs/sanitizer.md`）：所有送 LLM 的文字前置去識別化（Scanner 入庫 + Provider pre-flight + KB growth 三段）
- **Code 與教材不落地雲端**：產出物、進度、KB 索引全本地
- **Sidecar 封閉**：Python Sidecar bind `127.0.0.1` + 啟動隨機 token，禁對外
- **Tauri fs scope 白名單**：只允許使用者選定的資料夾與 workspace 目錄
- **授權 Modal**：首次執行需使用者確認「送哪些資料、送到哪」才啟動 Agent
- **Kill switch**：設定檔可一鍵停用 LLM 呼叫，Cancel 按鈕即時中止 in-flight

### Demo 前 checklist（摘要）
- [ ] 使用完全模擬資料（Timeline 已確認）
- [ ] 不出現任何公司內部名稱、未公開資訊
- [ ] Sidecar port 隨機、localhost only
- [ ] PII scanner 對 demo repo 跑過無 leakage
- [ ] Agent console 即時顯示決策，可稽核
- [ ] Token / cost 即時顯示（本次路線 X tokens / $Y，逐模組 breakdown；D-021）
- [ ] LLM Calls 分頁可展示完整 request/response（post-sanitize wire payload，證明去識別化確實生效；D-022）

---

## 十二、常見尖銳問答

**Q：這跟 NotebookLM 差在哪？**
A：NotebookLM 是通用學習工具，CodeBus 是**工程師專用**。我們有三個 NotebookLM 做不到的事——分析程式碼結構與依賴關係、根據你的任務客製學習路線、code 完全本地不上雲。

**Q：這跟 DeepWiki 差在哪？**
A：DeepWiki 是**靜態的 repo 說明書**——看完你還要自己判斷學什麼、學多深、哪些優先。CodeBus 是**任務導向動態路線**：同一個 repo，「我要改 OTA」跟「我要修登入 bug」走完全不同的站牌。而且 CodeBus 有 **Agent 決策可視化**（每步 thought 即時顯示），使用者看得到「為什麼排這站」——DeepWiki 沒這層。本地運作也是關鍵差異，DeepWiki 是雲端服務，企業機密 code 不能送。

**Q：這跟直接問 ChatGPT 差在哪？**
A：ChatGPT 是你問一句它答一句，你要自己知道該問什麼。CodeBus 是你什麼都不知道的時候，它主動幫你規劃該學什麼、用什麼順序、學到哪裡了。**主動權在 Agent，不在你。**

**Q：哪個部分是 Agentic？**
A：核心是 **Explorer Agent 探索式讀 code**——不是 RAG pipeline，而是像工程師辦案的決策迴圈：

1. **自主決定讀什麼**：grep 找入口 → 看到重要 import 自己決定追進去 → 不相關的略過
2. **自我評估與換策略**：關鍵字查不到會換詞；讀完覺得不夠深回頭補查
3. **Self-review loop**：路線產出後 Coverage Checker 扮演新人檢查 gap，不滿意 Planner 自己補站重排

每步決策 Agent 都印出 thought（「我看到 X 所以決定 Y」），Demo 時直接展示 Agent 的思考過程 — 這是 RAG pipeline 做不到的。完整決策紀錄會存成 `reasoning_log.jsonl`，可以回放。

**Q：你怎麼證明沒偷打 API、沒送敏感資料？**
A：三道證據鏈 —— **(1)** Sanitizer 三段防線（Scanner 入庫 + Provider pre-flight + Q&A add_to_kb，D-015）把 secret / PII / 內部識別符統一替成 `<REDACTED:kind#N>`；**(2)** **LLM Calls 分頁**（D-022）直接秀**每一次** request / response 完整 wire payload，placeholder 原樣可見；**(3)** 六層稽核 JSONL（sanitize / tool / kb_growth / reasoning / usage / llm_calls）全部本地存檔可 export。不用相信我們的口頭保證，打開 App 就能自己看。

**Q：商業化價值怎麼算？**
A：以中大型工程團隊規模，每年新進 RD 人力成本乘以 onboarding 時間縮短比例。即使只減少 30% 上手時間，年節省成本都很可觀。

---

## 十三、名稱由來

**CodeBus（程式碼巴士）**

呼應 2025 年爆紅的「上車舞」迷因——「XX 哥哥，我們來接你了呦～來囉來囉～」。新人工程師是乘客，Agent 是司機，資料夾或 topic 是目的地，學習完成就「下車囉」。

既幽默又貼切，Demo 開場就能讓觀眾留下印象。
