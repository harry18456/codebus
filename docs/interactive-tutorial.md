# Interactive Tutorial Spec

> CodeBus 最終產出的教材 `tutorial.md` 如何在 App 內變成「投影片 + QA」互動體驗。
> 這份 spec 是 Module 5（Markdown Generator）與 Module 7（前端互動層）的介面契約。

---

## 一、設計目標

- 教材在 App 內像「投影片課程」：一頁一頁走、有 QA、可解鎖
- 教材的 `.md` 檔**獨立可讀**仍然成立（GitHub / VS Code 打開通順）
- Markdown Generator 產出、前端解析，兩端契約單純
- 借鑑 Timeline PWA 的 `@nuxtjs/mdc` + 自訂 slot 元件模式

---

## 二、核心策略：Markdown 章節 + 自訂元件混合

**內容頁**用純 Markdown 的 `###` 分頁 → 獨立可讀 + 投影片模式都成立
**互動部分**用自訂元件 → App 內有互動，純 `.md` 渲染成空也無傷大雅

### 範例

```markdown
## 🚏 站 2: MQTT Client

### 核心概念
MQTT 是輕量級訊息協定...

### 這個專案怎麼用
本專案用 paho-mqtt，初始化在 `src/mqtt/client.py`：

\`\`\`python
client = mqtt.Client()
\`\`\`

### 檢核站

<Checkpoint>
- [ ] 我能解釋 MQTT QoS 三種等級
- [ ] 我能在 code 裡找到 broker 位置
</Checkpoint>

<Quiz correct="b">
這個專案用哪個 broker？
- a) Mosquitto
- b) EMQX
- c) HiveMQ
</Quiz>
```

---

## 三、View Mode（兩種切換）

| Mode | 適合情境 |
|---|---|
| 🎞️ 投影片模式（預設） | 照著 Agent 規劃走完，一頁一頁翻 |
| 📄 文件模式 | 回看、快速瀏覽、搜尋 |

- 投影片模式：`###` 當分頁符，left/right 鍵切換，進度條顯示
- 文件模式：整份 scroll，站牌當 anchor

---

## 四、MVP 自訂元件清單

放在 `components/content/`，`@nuxtjs/mdc` 會自動掛載。

| 元件 | 用途 | Phase |
|---|---|---|
| `<Checkpoint>` | 勾選式自評 checklist | P0 |
| `<Quiz correct="...">` | 單選題，前端比對答案 | P0 |
| `<QAEntry>` | 跳至 Q&A Agent 對話（D-016 / Module 8） | P0 |
| `<CodeRef file="..." lines="...">` | 點開在 side panel 看真實檔案 | P1 |
| `<Reveal hint="...">` | 漸進式揭露（點擊才顯示答案） | P1 |

### 元件契約

**`<Checkpoint>`**
```markdown
<Checkpoint id="station-2-check">
- [ ] 項目 1
- [ ] 項目 2
</Checkpoint>
```
- 勾選狀態存 `progress.json.checkpoints[id]`
- 全勾完 = 這站 Checkpoint 通過

**`<Quiz>`**
```markdown
<Quiz id="s2-q1" correct="b">
問題內容
- a) 選項 A
- b) 選項 B
- c) 選項 C
</Quiz>
```
- 使用者選錯顯示提示，可重試
- 答對狀態存 `progress.json.quizzes[id]`

**`<QAEntry>`**
```markdown
<QAEntry prompt="這段 retry 策略為什麼不會產生重複扣款？">
還有疑問？問 Q&A Agent
</QAEntry>
```
- 點擊把 `prompt` 當預填問題帶進 Q&A session（Module 8）
- 若 Agent 沉澱了新 chunk，KB growth 會通知當站 console（詳見 `qa-agent.md`）
- Generator 端在「值得延伸探索」的段落尾自動插入（`prompts.md` Generator prompt 管控用法）

**`<CodeRef>`** — 點擊在右側 panel 開檔案，不離開當前站
**`<Reveal>`** — `<details>` 風格，但樣式配合深色主題

---

## 五、進度與解鎖規則

```json
{
  "current_station": 2,
  "completed_stations": [1],
  "checkpoints": {
    "station-1-check": { "done": true, "ts": "..." }
  },
  "quizzes": {
    "s1-q1": { "answer": "b", "correct": true, "attempts": 1 }
  }
}
```

### 解鎖邏輯（Phase 1）

一站「完成」 = Checkpoint 全勾 + 所有 `<Quiz>` 答對 → 解鎖下一站

### 已完成站可回看
狀態顯示已通過，QA 保留答題紀錄。

---

## 六、Markdown Generator 端的責任

Module 5 產出 tutorial.md 時必須：

1. 每站 (`## 🚏 站 N`) 下至少 1 個 `<Checkpoint>` 或 `<Quiz>`
2. 每個 `<Quiz>` 給 `id` 和 `correct`
3. `###` 當內容分頁符，一頁內容量 ≤ 約 300 字或 1 個程式碼區塊
4. `route.json` 的 `stations[n].required_checks` 列出本站必過 id 清單

### route.json 擴充

```json
{
  "stations": [
    {
      "id": 2,
      "title": "MQTT Client",
      "required_checks": ["station-2-check", "s2-q1"],
      "markdown_anchor": "站 2: MQTT Client"
    }
  ]
}
```

---

## 七、MVP 不做（明確記錄）

以下項目**MVP 明確不做**，但 spec 已保留介面讓未來能加：

| 項目 | 延後原因 | 落在 |
|---|---|---|
| `<AgentThought step="5">` 內嵌 Agent 決策到教材 | 是 agentic 錦上添花，但 Module 5 的 prompt 要串 `reasoning_log.jsonl`，複雜 | Phase 2 |
| `<DependencyMap>` 依賴圖視覺化 | 需要額外渲染引擎（mermaid/d3），工期壓力 | Phase 2 |
| `<RelatedVideo>` / YouTube 嵌入 | 要 Agent 去找影片 + 品質評估 | Phase 2（Topic mode 同步） |
| LLM 判題（使用者打字回答 → Agent 判對錯 + 回饋） | Quiz 比對 answer 已經夠 MVP | Phase 3 |
| 多選題、填空題、拖拉題 | 單選題 + Checkbox 已覆蓋 MVP demo 需求 | Phase 3 |
| Agent 決策回放 UI（slider 倒帶探索歷程） | 屬於「探索過程」那條線，不是教材互動 | Phase 2 |
| 筆記功能（使用者在教材旁寫註解） | 不是 MVP 核心路徑 | Phase 3 |
| 教材匯出為 PDF / slide deck | 可用瀏覽器列印 fallback | Phase 3 |

---

## 八、風險

| 風險 | 對策 |
|---|---|
| LLM 產出的 Quiz 答案不穩定 | Generator prompt 強約束輸出 JSON schema + 驗證 |
| `<Quiz>` 選項 ABC 順序被 LLM 打亂導致 correct 對不上 | Generator 端規定：選項一定 a/b/c/d，correct 用值 |
| 自訂元件讓 `.md` 在 GitHub 變醜 | 提供 Generator `--plain` mode 產出純 Markdown 版 |
| 章節分頁 `###` 被 LLM 寫得太長 | Generator 限制一頁字數，超過自動切 |

---

## 九、實作順序建議（對齊 Module 5 + Module 7）

### P0 — MVP 必做
1. Module 5 Generator 產出含 `<Checkpoint>` 與 `<Quiz>` 的 tutorial.md
2. Module 7 前端 `@nuxtjs/mdc` 掛 `Checkpoint.vue` / `Quiz.vue` / `QAEntry.vue`
3. 投影片模式（`###` 分頁翻頁）
4. `progress.json` 讀寫 + 解鎖邏輯

### P1 — 時間夠加
5. 文件模式切換
6. `<CodeRef>` + side panel 檔案瀏覽
7. `<Reveal>` 漸進揭露

### P2+ — 見第七節
