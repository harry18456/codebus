# Module 5 — Markdown Generator Spec

> 把 Explorer Agent 的 stations 轉成投影片風格的多檔教材（MOC `tutorial.md` + 每站一檔 `stations/s0X-slug.md`）+ `route.json`。
> 關聯決策：D-011（資安）、D-015（Sanitizer）、D-016（前後接 Q&A）、**D-029（多檔輸出 + frontmatter + stable station id，拒絕 Obsidian 整合）**。
> 關聯文件：`interactive-tutorial.md`（MD 互動元件契約）、`agent-explorer-spec.md`（stations 輸出）、`qa-agent.md`（教材後 Q&A 入口）。

---

## 一、職責

### 輸入
- `ExplorerResult.stations`（站清單 + 每站的 related_files / reasoning 摘要）
- `KnowledgeBase`（查每站相關 chunks 做引用）
- `task`（使用者任務描述）
- `options`：`mode ∈ {interactive, plain}`、`target_persona`

### 輸出（多檔結構 — D-029）

- `tutorial.md` — **MOC（Map of Content）索引頁**：站列表、metadata、路線總覽、結尾 `<QAEntry>`；**不重複站內容**
- `stations/s{NN}-{slug}.md` — **每站一檔**，含 frontmatter + markdown + 互動元件
  - `{NN}`：zero-padded 2-digit index（`s01` ~ `s99`）
  - `{slug}`：kebab-case、ASCII、≤ 40 char 的 station title 摘要（e.g. `s02-storage-contract`）
  - 組合後的 `s{NN}-{slug}` 是 **stable station id**，跨檔引用 / Q&A 錨點 / URL 路由都依賴此穩定性
- `route.json` — 結構化路線（含 `station_id` 與 `file_path`，見 §八）
- `generator_log.jsonl` — 每站生成過程、重試、degraded 紀錄

**輸出根目錄**：`<workspace-root>/codebus-tutorials/{task_id}/`（`module-5-generator-p0` Decision 3：用 `codebus-tutorials/` 取代原本 generic 的 `tutorials/`，明確標示 CodeBus 產出 + 不撞使用者既有 `tutorials/` 目錄）。實際 layout：

```
codebus-tutorials/{task_id}/
├── tutorial.md              # MOC 索引
├── stations/
│   ├── s01-repo-overview.md
│   ├── s02-storage-contract.md
│   └── s03-adapter-pattern.md
├── route.json
└── progress.json            # 前端讀寫（§ interactive-tutorial 五）
```

`generator_log.jsonl` 是 per-Module operational log，落 `<workspace-root>/.codebus/generator_log.jsonl`（與 `reasoning_log.jsonl` 同層；非 user-facing 教材產品）。

**檔名不可變式**（D-029 §十六.2）：station 檔一經寫出不改名；slug 碰撞時後綴 `-2`, `-3`（e.g. `s03-storage-contract-2`）。

---

## 二、整體流程

```
for station in stations:
    ├─ assign stable id (s{NN}-{slug})
    ├─ 準備 context（related files 內容、KB hits、前站摘要 + 前站 stable ids）
    ├─ call LLM with station prompt
    ├─ validate output（元件格式、schema）
    ├─ retry if invalid (max 3)
    ├─ render frontmatter (YAML) + station markdown
    └─ write stations/s{NN}-{slug}.md

整合：tutorial.md（MOC）← 頁首 + 站列表（每行一個 [title](./stations/s{NN}-{slug}.md) 連結）
產出：route.json 從 stations 衍生（含 station_id + file_path + related_stations）
```

**不一次產整份**——每站獨立 LLM call + 獨立檔案，失敗重試僅該站，degraded fallback 亦僅影響該站檔案（§十）。

---

## 三、每站 Prompt 架構

### System prompt 骨架
詳見 `docs/prompts.md` §五（generator_station）。關鍵約束：
1. 每站**至少 1 個 `<Checkpoint>`**
2. `<Quiz>` **最多 1 個**（非必要，避免每站都考試變疲勞）
3. 使用者層級（`target_persona`）決定語言複雜度
4. 每站**長度上限 ≤ 800 字**（中文字元數），超過觸發頁內 `###` 分頁
5. 程式碼片段 ≤ 30 行，長檔用 `<CodeRef>`（P1）
6. `###` 分頁符：內容量 > 300 字時插入

### 輸入 context 組成

```python
{
  "task": "新增 Google Drive Adapter 同步功能",
  "target_persona": "有程式基礎但不熟此 repo 的工程師",
  "station_index": 2,
  "station_title": "Storage 介面契約",
  "station_duration_minutes": 15,
  "related_files": [
    {"path": "app/types/index.ts", "content_range": "109-122", "content": "..."},
  ],
  "kb_hits": [
    {"text": "...", "source": "app/types/index.ts:109"},
  ],
  "previous_stations_summary": [
    {"idx": 1, "title": "...", "one_liner": "..."},
  ],
  "mode": "interactive"  # or "plain"
}
```

---

## 四、輸出元件規則（`interactive-tutorial.md` 第四節的契約）

### `<Checkpoint>` — 必出
```markdown
<Checkpoint id="station-{N}-check">
- [ ] 項目 1
- [ ] 項目 2
</Checkpoint>
```
- 每站至少 1 個，可多個
- `id` 必須：`station-{station_index}-check` 或加後綴
- 項目 2-5 條，每條是可自我驗證的學習指標

### `<Quiz>` — 0 或 1 個
```markdown
<Quiz id="s{N}-q1" correct="b">
這個專案用哪個 broker？
- a) Mosquitto
- b) EMQX
- c) HiveMQ
</Quiz>
```
- **`correct` 欄位必須存在**（`b`、`c` 等）
- 選項固定 a/b/c/d 順序，`correct` 值為字母
- 3-4 個選項
- 問題要能從本站內容推得答案

### `<CodeRef file="..." lines="...">` — 可選（P1）
```markdown
<CodeRef file="app/services/LocalFileAdapter.ts" lines="45-78" />
```
- file 必須是 workspace 內路徑
- lines 格式 `start-end`

### `<Reveal hint="...">` — 可選（P1）
```markdown
<Reveal hint="想想 interface 有幾個 method">
答案：12 個
</Reveal>
```

### 圖片引用（D-028 — MVP 範圍）

MVP 只允許 **inline markdown `![]()` 相對路徑** 引用 workspace 內既有圖片
（e.g. `![架構圖](docs/arch.png)`），**不對圖做 LLM 解讀**：

- Generator **不把圖像餵進 LLM**——provider 介面（`llm-provider.md §二`）沒有
  `images` 參數，也沒有 `supports_vision` capability（D-028 已決不預埋）
- Scanner 仍保留圖片檔的 path / size / mtime metadata，但不做 OCR / captioning
- 圖片路徑仍要過 workspace sandbox 驗證（§三 `ensure_in_workspace`），禁 `..` 逃逸
- Phase 2 若要支援 vision，走 additive 擴充：Provider Protocol 加 `images` 參數、
  `RoleConfig` 加對應 role、Generator 才開放圖像解讀 prompt

---

## 五、格式驗證

每站 LLM 輸出後，跑驗證管線：

```python
def validate_station_markdown(md: str, station_idx: int) -> ValidationResult:
    issues = []

    # 1. 至少 1 個 <Checkpoint>
    checkpoints = parse_checkpoints(md)
    if not checkpoints:
        issues.append("missing_checkpoint")

    # 2. <Quiz> 最多 1 個，且格式正確
    quizzes = parse_quizzes(md)
    if len(quizzes) > 1:
        issues.append("too_many_quizzes")
    for q in quizzes:
        if q.correct not in {"a", "b", "c", "d"}:
            issues.append(f"quiz_bad_correct: {q.correct}")
        if not all(opt in q.options for opt in ["a", "b"]):
            issues.append("quiz_missing_options")

    # 3. 長度
    if char_count(md) > 800:
        issues.append("too_long")

    # 4. 程式碼片段 < 30 行
    for block in code_blocks(md):
        if block.line_count > 30:
            issues.append("code_block_too_long")

    # 5. `<CodeRef>` 路徑必須在 workspace
    for ref in parse_coderefs(md):
        if not is_in_workspace(ref.file):
            issues.append(f"coderef_escape: {ref.file}")

    return ValidationResult(issues=issues, parsed={
        "checkpoints": checkpoints, "quizzes": quizzes, ...
    })
```

### 重試策略

```python
for attempt in range(MAX_RETRIES := 3):
    md = await llm_generate_station(context)
    result = validate_station_markdown(md, idx)
    if not result.issues:
        return md
    # 把 issues 塞進下輪 prompt 當 correction
    context["previous_issues"] = result.issues

raise StationGenerationFailed(station_idx=idx, last_issues=result.issues)
```

重試仍失敗 → **fallback**：產出精簡版（只留核心文字 + 一個 checkpoint，不出 quiz），標記 `degraded: true` 記在 generator_log。不讓整份 tutorial 失敗。

---

## 六、`--plain` Mode

當 `mode: "plain"`：
- **不輸出** `<Checkpoint>` / `<Quiz>` / `<CodeRef>` / `<Reveal>`
- `<Checkpoint>` 內容改為普通 `- [ ]` Markdown task list
- `<Quiz>` 改為 `> 思考題：...` 格式（答案附在段末）
- `###` 分頁符保留（GitHub 顯示正常）

目標：產出的 `.md` 在 GitHub / VS Code 預覽友善，適合丟 Wiki 或獨立分享。

Generator 走兩套 prompt 模板（interactive / plain）；驗證規則也對應調整。

---

## 七、整體結構（多檔 — D-029）

### 7.1 MOC：`tutorial.md`（純索引頁）

```markdown
# {task} — CodeBus 學習教材

> **目標**：{task}
> **預估時長**：{total_minutes} 分鐘
> **產出時間**：{iso_timestamp}
> **Repo**：{workspace_name}

## 🚌 路線總覽

1. 🚏 [{station_1_title}](./stations/s01-{slug_1}.md)（{duration} min）
2. 🚏 [{station_2_title}](./stations/s02-{slug_2}.md)（{duration} min）
...

---

## 🎯 下車（完成）

恭喜走完全程。還想深入問問題？

<QAEntry prompt="整條路線我最想再追一下的是：">
繼續問 Q&A Agent
</QAEntry>
```

**MOC 規則**：

- **純索引**：不重複站內容；每項只有 title + duration + 相對連結
- **連結用標準 markdown**：`[text](./stations/sXX-slug.md)` —— 不用 `[[wikilinks]]`（D-029 §十六.1 不變式）
- `<QAEntry>` 只在 MOC 結尾出一次（interactive mode）；plain mode 改為純文字「本專案有 Q&A 功能可對話式繼續學習」
- MOC 本身可不帶 frontmatter；若要帶，欄位限於 `{schema_version, repo_name, task, generated_at, total_stations, estimated_minutes}`

### 7.2 Station 檔：`stations/s{NN}-{slug}.md`

```markdown
---
schema_version: 1
station_id: s02-storage-contract
station_index: 2
title: Storage 介面契約
duration_minutes: 15
workspace_type: folder
repo_name: timeline-app
task: "新增 Google Drive Adapter 同步功能"
generated_at: "2026-04-20T10:30:00Z"
tags: [architecture, interfaces]
related_stations: [s01-repo-overview, s03-adapter-pattern]
related_files:
  - app/types/index.ts:109-122
  - app/services/LocalFileAdapter.ts
required_checks: [station-2-check, s2-q1]
degraded: false
---

# 🚏 {title}

{station_markdown}

<Checkpoint id="station-2-check">
- [ ] ...
</Checkpoint>
```

**Station 檔規則**：

- **第一層 heading（`#`）** 是 station title（含 🚏），不含 "站 N" 前綴——`station_index` 已在 frontmatter
- 內部 `###` 分頁符規則同 §三.6
- 站間互連用相對路徑：`[參考儲存介面](./s02-storage-contract.md)`——**不跨目錄**（station 檔都在 `stations/` 平級）

### 7.3 frontmatter schema（YAML）

| 欄位 | 型別 | 必填 | 語意 |
|---|---|---|---|
| `schema_version` | int | ✅ | 目前 `1`；未來擴充 additive，移除欄位前 bump version（§十六.4 不變式） |
| `station_id` | string | ✅ | `s{NN}-{slug}`，與檔名一致 |
| `station_index` | int | ✅ | 1-based，對應 `ExplorerResult.stations[i]` |
| `title` | string | ✅ | Station 顯示標題（可含中文） |
| `duration_minutes` | int | ✅ | Explorer 給的預估時長 |
| `workspace_type` | `"folder"` \| `"topic"` | ✅ | D-002 雙模 discriminator |
| `repo_name` | string | ✅ | workspace 根目錄名 |
| `task` | string | ✅ | 使用者原始任務描述 |
| `generated_at` | ISO-8601 | ✅ | 本站產出時間 |
| `tags` | string[] | ❌ | LLM 生成的 2-4 個主題標籤（kebab-case） |
| `related_stations` | string[] | ❌ | 前後相關站的 stable ids |
| `related_files` | string[] | ❌ | 站相關檔案路徑（可帶 `:start-end` 行號） |
| `required_checks` | string[] | ✅ | 從 markdown 解析出的 checkpoint / quiz ids |
| `degraded` | bool | ✅ | 重試失敗 fallback 產出時為 `true`（§十） |

### 7.4 Stable Station ID 規則

格式：`s{NN}-{slug}` —— 例如 `s02-storage-contract`

- `{NN}`：zero-padded 2-digit index（01~99）
- `{slug}`：
  - kebab-case、僅 `[a-z0-9-]`、長度 ≤ 40
  - 由 LLM 從 station title 生成後規範化（去中文、去符號、轉小寫、合併 `-`）
  - 空 slug fallback：`s{NN}-station`
  - 碰撞處理：同 slug 出現時後綴 `-2`, `-3`（e.g. `s03-storage-contract-2`）
- **一經寫出不改名**（D-029 §十六.2）：Q&A `add_to_kb`、cross-link、URL 路由都依賴此穩定性

### 7.5 結尾 Q&A 入口

MOC 結尾的 `<QAEntry>` 元件（契約見 `interactive-tutorial.md` §四）：

- `interactive` mode 下必出，前端掛載後按鈕會把 `prompt` 預填進 Module 8（D-016）session
- `plain` mode 把整個 `<QAEntry>` 段改成純文字「本專案有 Q&A 功能可對話式繼續學習」
- Generator 也可在每站尾「值得延伸探索」處插入 `<QAEntry>`（視 station 的 follow-up hook 而定，非必出）

---

## 八、route.json 產出

依 `interactive-tutorial.md` §六，D-029 後改為多檔引用：

```json
{
  "title": "{task}",
  "task": "{task}",
  "source_type": "folder",
  "source_path": "{workspace_root}",
  "estimated_minutes": 120,
  "generated_at": "{iso}",
  "stations": [
    {
      "station_id": "s02-storage-contract",
      "index": 2,
      "title": "Storage 介面契約",
      "duration": 15,
      "file_path": "stations/s02-storage-contract.md",
      "prerequisites": [],
      "related_files": ["app/types/index.ts:109-122"],
      "related_stations": ["s01-repo-overview"],
      "required_checks": ["station-2-check", "s2-q1"],
      "degraded": false
    }
  ]
}
```

- `station_id`：stable id（§7.4），**取代舊 `id`（整數）**；跨檔引用 / URL 路由 / Q&A `add_to_kb` 都用這個
- `file_path`：相對 `route.json` 的路徑，前端用它載檔
- `required_checks` / `degraded` 由 validation 階段從 md + frontmatter 自動填入
- 舊欄位 `markdown_anchor` 在多檔結構下已無意義，移除

---

## 九、Sanitize 與 Sandbox 連動

- **Scanner / KB 已是清理版**——Generator 拿到的 `related_files[].content` 與 `kb_hits` 原本就乾淨
- **Provider pre-flight**（D-015 第二段）：LLM call 前再掃一次 prompt，防呆
- **Generator output 過 Pass 1 Sanitizer**（`module-5-generator-p0` Decision 1，YES）：每站 markdown + MOC 寫檔前 `SanitizerEngine.sanitize(content, source=FileSource(pass_="generator", path=output_path))`；命中寫 `<workspace>/.codebus/sanitize_audit.jsonl` `pass_num=1` `source.path=codebus-tutorials/{task_id}/stations/sXX-x.md`。Defense in depth — 即便 LLM input 已清理，creative output 仍可能合成 secret-like 樣本或從 placeholder 反推
- **`<CodeRef>`**：路徑必須在 workspace（Sandbox §三驗證），否則驗證階段擋下

---

## 十、失敗處理

| 情況 | 處理 |
|---|---|
| 單站重試 3 次仍失敗 | 產 degraded 版本寫入 `stations/s{NN}-{slug}.md`（frontmatter `degraded: true`），log warning，**其他站檔案不受影響**（多檔隔離 — D-029） |
| 全部站都 degraded | route.json 頂層補 `degraded: true`（所有站檔案 frontmatter 也都是 true），UI 提示使用者「教材品質可能不佳，是否重跑」 |
| LLM 產出完全無法 parse（markdown 壞掉） | 當作重試觸發，prompt 明示「必須合法 markdown + 元件」 |
| Context 超 LLM window | 縮減 `related_files` 內容至前 100 行 + 斷點摘要 |
| frontmatter YAML 驗證失敗 | 視同 validator issue 觸發重試；fallback 時 frontmatter 用 minimal 必填欄位 + `degraded: true` |
| Station 檔案寫入失敗（disk I/O） | log error 至 `generator_log.jsonl`，該 station 於 route.json 標 `degraded: true, error: "write_failed"`；不重試寫入（避免無窮迴圈）|

---

## 十一、生成進度回報（SSE）

```json
{ "type": "progress", "phase": "generating", "current_station": 2, "total_stations": 5, "status": "generating", "station_id": "s02-storage-contract" }
{ "type": "progress", "phase": "generating", "current_station": 2, "total_stations": 5, "status": "validating", "station_id": "s02-storage-contract" }
{ "type": "progress", "phase": "generating", "current_station": 2, "total_stations": 5, "status": "retry", "attempt": 2, "station_id": "s02-storage-contract" }
{ "type": "progress", "phase": "generating", "current_station": 2, "total_stations": 5, "status": "writing_file", "station_id": "s02-storage-contract", "file_path": "stations/s02-storage-contract.md" }
{ "type": "progress", "phase": "assembling_moc", "total_stations": 5, "status": "writing_file", "file_path": "tutorial.md" }
```

每站進入 / 完成 / 重試 / 寫檔都推一次；MOC 組裝是最後一個階段。`station_id` 從 §7.4 配發後每次事件都帶，前端可依此點高亮目前站。

---

## 十二、測試

### 單元
- validator 各條（quiz bad correct / too_many_quiz / missing_checkpoint ...）
- plain mode 輸出不含自訂元件
- route.json 從 md + frontmatter 解析 required_checks / station_id / file_path 正確
- **Stable id 生成**：title → slug 規範化（中文 / 特殊符號 / 空字串 fallback）
- **Slug 碰撞**：同 title 連續兩站產出 `-2` 後綴
- **frontmatter YAML 驗證**：required 欄位缺失觸發重試；minimal fallback 成立
- **MOC 連結解析**：每條連結對應實際存在的 `stations/*.md`（無斷鏈）

### Fixture
`tests/fixtures/generator/`：
- Mock stations（3 站）+ mock KB hits，驗完整多檔 pipeline（產 MOC + 3 份 station 檔 + route.json）
- 壞格式 Quiz 範例，驗 retry 觸發
- 一站 degraded + 兩站正常：驗 isolation（其他站檔案不受影響 + route.json `degraded` 欄正確）

### Golden sample 整合（D-006）
Timeline + Google Drive Adapter（D-004）跑完整 Explorer → Generator，人工 review 5 站品質；分數退步 > 10% → review。

---

## 十三、MVP 不做

| 項 | 延後原因 |
|---|---|
| 多語言教材（英 / 日） | Phase 3 |
| LLM 判題（Phase 3） | Quiz 比對 correct 已足 MVP |
| 多選題 / 填空題 / 拖拉題 | Phase 3 |
| 圖解生成（Mermaid / 依賴圖） | Phase 2 `<DependencyMap>` |
| 嵌入 YouTube 教學影片 | Phase 2 |
| `<AgentThought>` 回放元件 | Phase 2 |
| 匯出 PDF / slide deck | Phase 3 |

---

## 十四、實作順序

| 優先 | 項目 | 工期 |
|---|---|---|
| P0 | 每站 prompt + LLM call pipeline | 0.5d |
| P0 | Validator（checkpoint / quiz / length） | 0.5d |
| P0 | Stable id + slug 規範化 + collision handling | 0.25d |
| P0 | frontmatter renderer + YAML 驗證 | 0.25d |
| P0 | 重試與 degraded fallback（per-station 隔離） | 0.25d |
| P0 | MOC 組裝（純索引 + 標準 markdown link） + route.json 輸出 | 0.5d |
| P0 | SSE progress emit（含 station_id / file_path 欄位） | 0.25d |
| P0 | Plain mode prompt 模板 + validator 分支 | 0.5d |
| P1 | `<CodeRef>` / `<Reveal>` 支援 | 0.5d |
| P1 | Generator log.jsonl + UI 查看 | 0.25d |
| P1 | Golden sample integration（Timeline） | 0.5d |

**合計 P0 ~3.0d / P0+P1 ~4.25d。**（D-029 多檔輸出 + frontmatter + stable id 加 0.5d 在 P0）

---

## 十五、待 review 的決策

以下我先填預設值，review 時確認：
- **單站長度上限 800 字元**：中英混排，中文字 1 = 英文字 1
- **Quiz 最多 1 個/站**：避免疲勞；若某站天然有兩題可考改「其一放 Checkpoint」
- **Degraded fallback 是否啟用**：預設 on；若你偏好「失敗即停讓人工重跑」可關
- **結尾 Q&A 入口**：預設在 interactive mode 自動加；plain mode 純文字提示
- **Slug 長度上限 40 char**（§7.4）：太長截斷時優先留前綴語意詞，尾段以 hash 不替代（避免穩定性問題）

---

## 十六、D-029 關鍵不變式

1. **標準 markdown 輸出**：MOC 與 station 檔間連結用 `[text](./path)`，**禁用** `[[wikilinks]]` / `%%comment%%` / `==highlight==` 等 Obsidian-specific 語法；frontmatter 用通用 YAML
2. **檔名穩定**：`s{NN}-{slug}` 一經寫出不改名；後續 Q&A `add_to_kb`、cross-link、URL 路由均依賴此穩定性（改名會破壞所有 outstanding reference）
3. **MOC 純索引**：`tutorial.md` 只有站列表、metadata、`<QAEntry>`；**不重複站內容**（避免雙真相源）
4. **frontmatter schema 版本化**：擴充欄位為 additive；移除或改型別需 bump `schema_version` + 留 migration note

---

## 十七、Partial regen via `target_stations`（介入點 2）

`POST /generate` 接受 optional `target_stations: list[str] | None`（default `None`）：

- **`target_stations is None`**（default）：行為與既有 full path 等價 — iterate 全 `state.stations` + 寫 MOC + 寫 `route.json`
- **`target_stations` 非空**：partial-regen path，只覆寫命中站的 `stations/s{NN}-{slug}.md`；**不**呼 MOC assembler、**不**寫 `route.json`、**不**動 unrelated stations 的檔案

落點：

- `sidecar/src/codebus_agent/api/generate.py` `GenerateRequest` 多 `target_stations` 欄；endpoint 用 `derive_station_ids(stations)` pre-flight：任何 id 不在派生集 → HTTP 400 `GENERATE_TARGET_STATION_INVALID`（帶 offending id）
- `sidecar/src/codebus_agent/generator/runner.py` `run_generator(...)` 加 `target_stations` keyword-only 參數；非空時走 `_run_partial_regen` 分支

關鍵不變式：

1. **byte-identical 不變式**：`tutorial.md` / `route.json` / 不在 `target_stations` 內的 station markdown 檔，partial regen 前後 byte-identical（`generator_log.jsonl` 是唯一例外，會 append 新行）
2. **station_id drift 拒絕**：runner 對命中站重新跑 `_generate_station` 後，若 stable_id 與 request 不符（例如 LLM 換了 slug）→ 拒絕該 id（log `event: station_id_drift` 雙錄 requested + observed），檔案保持原樣，**繼續**處理 `target_stations` 中的其餘 id（no short-circuit）
3. **`mode` discriminator**：partial path 在 `generator_log.jsonl` 的 `run_started` / `run_completed` / `station_id_drift` 行加 `run_mode="partial"`；per-station 完成行 `event="station_partial_regenerated"` 加 `mode="partial"` + `station_id`，與 full-mode 行（`mode=options.mode`，例如 `"interactive"`）區分
4. **無分散 short-circuit**：單一 target station LLM 失敗（validator 跑光 retry budget → degraded）不影響後續 target；`GeneratorResult.station_paths` 只列實際 regenerated 檔案 + `degraded_count` 反映 partial scope 內失敗數

前端 wiring（`phase6-step29-intervention-points`）：

- `<RegenStationButton>` 在 station page header chrome；click 開 `<InterventionConfirmModal>`「重生會覆蓋本站 markdown 與 frontmatter，其他站與 MOC 不變」
- confirm onConfirm → page-level `startRegen(stationId)` → `useSidecar().fetch('/generate', { ..., target_stations: [stationId] })` → `useSseTask` 接 SSE → 完成後 `useTutorialFiles().readTutorialFile()` 重讀 station markdown 讓 `<StationContent>` 重新 render
