# Module 5 — Markdown Generator Spec

> 把 Explorer Agent 的 stations 轉成投影片風格的 `tutorial.md` + `route.json`。
> 關聯決策：D-011（資安）、D-015（Sanitizer）、D-016（前後接 Q&A）。
> 關聯文件：`interactive-tutorial.md`（MD 互動元件契約）、`agent-explorer-spec.md`（stations 輸出）、`qa-agent.md`（教材後 Q&A 入口）。

---

## 一、職責

### 輸入
- `ExplorerResult.stations`（站清單 + 每站的 related_files / reasoning 摘要）
- `KnowledgeBase`（查每站相關 chunks 做引用）
- `task`（使用者任務描述）
- `options`：`mode ∈ {interactive, plain}`、`target_persona`

### 輸出
- `tutorial.md` — 整份 Markdown，含互動元件
- `route.json` — 結構化路線（`interactive-tutorial.md` §六）
- `generator_log.jsonl` — 每站生成過程、重試紀錄

---

## 二、整體流程

```
for station in stations:
    ├─ 準備 context（related files 內容、KB hits、前站摘要）
    ├─ call LLM with station prompt
    ├─ validate output（元件格式、schema）
    ├─ retry if invalid (max 3)
    └─ collect markdown fragment

整合：tutorial.md ← 頁首 + 各站 markdown 依序拼接
產出：route.json 從 stations 衍生
```

**不一次產整份**——每站獨立 LLM call，失敗重試僅該站。

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

## 七、tutorial.md 整體結構

```markdown
# {task} — CodeBus 學習教材

> **目標**：{task}
> **預估時長**：{total_minutes} 分鐘
> **產出時間**：{iso_timestamp}
> **Repo**：{workspace_name}

## 🚌 路線總覽

1. 🚏 {station_1_title}（{duration} min）
2. 🚏 {station_2_title}（{duration} min）
...

---

## 🚏 站 1: {title_1}

{station_1_markdown}

---

## 🚏 站 2: {title_2}

{station_2_markdown}

...

---

## 🎯 下車（完成）

恭喜走完全程。還想深入問問題？

<QAEntry prompt="整條路線我最想再追一下的是：">
繼續問 Q&A Agent
</QAEntry>
```

**結尾 Q&A 入口**：`interactive` mode 下必用 `<QAEntry>` 元件（契約見 `interactive-tutorial.md` §四），前端掛載後按鈕會把 `prompt` 預填進 Module 8（D-016）session；`plain` mode 把整個 `<QAEntry>` 段改成純文字「本專案有 Q&A 功能可對話式繼續學習」。Generator 也可在每站尾「值得延伸探索」處插入 `<QAEntry>`（視 station 的 follow-up hook 而定，非必出）。

---

## 八、route.json 產出

依 `interactive-tutorial.md` §六：

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
      "id": 1,
      "title": "...",
      "duration": 15,
      "prerequisites": [],
      "markdown_anchor": "站 1: ...",
      "related_files": ["..."],
      "required_checks": ["station-1-check", "s1-q1"]
    }
  ]
}
```

`required_checks` 由 validation 階段從 md 解析出的 checkpoint / quiz id 自動填入。

---

## 九、Sanitize 與 Sandbox 連動

- **Scanner / KB 已是清理版**——Generator 拿到的 `related_files[].content` 與 `kb_hits` 原本就乾淨
- **Provider pre-flight**（D-015 第二段）：LLM call 前再掃一次 prompt，防呆
- **教材產出**：LLM 在清理版 context 上生成，自然產出清理版
- **`<CodeRef>`**：路徑必須在 workspace（Sandbox §三驗證），否則驗證階段擋下

---

## 十、失敗處理

| 情況 | 處理 |
|---|---|
| 單站重試 3 次仍失敗 | 產 degraded 版本，log warning，不中止整份 |
| 全部站都 degraded | 整體標 `degraded: true` 回傳，UI 提示使用者「教材品質可能不佳，是否重跑」 |
| LLM 產出完全無法 parse（markdown 壞掉） | 當作重試觸發，prompt 明示「必須合法 markdown + 元件」 |
| Context 超 LLM window | 縮減 `related_files` 內容至前 100 行 + 斷點摘要 |

---

## 十一、生成進度回報（SSE）

```json
{ "type": "progress", "phase": "generating", "current_station": 2, "total_stations": 5, "status": "generating" }
{ "type": "progress", "phase": "generating", "current_station": 2, "total_stations": 5, "status": "validating" }
{ "type": "progress", "phase": "generating", "current_station": 2, "total_stations": 5, "status": "retry", "attempt": 2 }
```

每站進入 / 完成 / 重試都推一次。

---

## 十二、測試

### 單元
- validator 各條（quiz bad correct / too_many_quiz / missing_checkpoint ...）
- plain mode 輸出不含自訂元件
- route.json 從 md 解析 required_checks 正確

### Fixture
`tests/fixtures/generator/`：
- Mock stations（3 站）+ mock KB hits，驗完整 pipeline
- 壞格式 Quiz 範例，驗 retry 觸發

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
| P0 | 重試與 degraded fallback | 0.25d |
| P0 | tutorial.md 組裝 + route.json 輸出 | 0.5d |
| P0 | SSE progress emit | 0.25d |
| P0 | Plain mode prompt 模板 + validator 分支 | 0.5d |
| P1 | `<CodeRef>` / `<Reveal>` 支援 | 0.5d |
| P1 | Generator log.jsonl + UI 查看 | 0.25d |
| P1 | Golden sample integration（Timeline） | 0.5d |

**合計 P0 ~2.5d / P0+P1 ~3.75d。**

---

## 十五、待 review 的決策

以下我先填預設值，review 時確認：
- **單站長度上限 800 字元**：中英混排，中文字 1 = 英文字 1
- **Quiz 最多 1 個/站**：避免疲勞；若某站天然有兩題可考改「其一放 Checkpoint」
- **Degraded fallback 是否啟用**：預設 on；若你偏好「失敗即停讓人工重跑」可關
- **結尾 Q&A 入口**：預設在 interactive mode 自動加；plain mode 純文字提示
