# Interactive Tutorial Spec

> CodeBus 最終產出的多檔教材（`tutorial.md` MOC + `stations/s{NN}-slug.md`）如何在 App 內變成「投影片 + QA」互動體驗。
> 這份 spec 是 Module 5（Markdown Generator）與 Module 7（前端互動層）的介面契約。
> 關聯決策：**D-029（多檔輸出 + stable station id，檔案=一頁的投影片路由）**、D-016（Q&A Agent 入口）、D-028（MVP 不做 Vision）。

---

## 一、設計目標

- 教材在 App 內像「投影片課程」：一頁一頁走、有 QA、可解鎖
- 教材的 `.md` 檔**獨立可讀**仍然成立（GitHub / VS Code 打開通順；多檔結構 + 標準 markdown link，每個 renderer 都吃）
- Markdown Generator 產出、前端解析，兩端契約單純
- 借鑑 Timeline PWA 的 `@nuxtjs/mdc` + 自訂 slot 元件模式
- **檔案 = 一頁**（D-029）：每份 station 檔（`stations/s{NN}-slug.md`）即投影片主單位；URL 路由用 stable station id，跨 session 可重現

---

## 二、核心策略：多檔 + Markdown 章節 + 自訂元件混合

**一站一檔**（D-029）→ 版本控制粒度、per-station cache、URL-per-station 路由自然成立
**內容頁**用純 Markdown 的 `###` 分頁符做檔內次級切頁 → 獨立可讀 + 投影片模式都成立
**互動部分**用自訂元件 → App 內有互動，純 `.md` 渲染成空也無傷大雅

### 範例（單站檔：`stations/s02-mqtt-client.md`）

```markdown
---
schema_version: 1
station_id: s02-mqtt-client
station_index: 2
title: MQTT Client
duration_minutes: 15
workspace_type: folder
repo_name: timeline-app
task: "..."
generated_at: "2026-04-20T10:30:00Z"
related_stations: [s01-repo-overview, s03-broker-selection]
required_checks: [station-2-check, s2-q1]
degraded: false
---

# 🚏 MQTT Client

### 核心概念
MQTT 是輕量級訊息協定...

### 這個專案怎麼用
本專案用 paho-mqtt，初始化在 `src/mqtt/client.py`：

\`\`\`python
client = mqtt.Client()
\`\`\`

### 檢核站

<Checkpoint id="station-2-check">
- [ ] 我能解釋 MQTT QoS 三種等級
- [ ] 我能在 code 裡找到 broker 位置
</Checkpoint>

<Quiz id="s2-q1" correct="b">
這個專案用哪個 broker？
- a) Mosquitto
- b) EMQX
- c) HiveMQ
</Quiz>
```

frontmatter schema 詳見 `module-5-generator.md` §7.3；前端解析 frontmatter 後傳進 `<StationLayout>` 外殼元件。

---

## 三、View Mode（兩種切換）

| Mode | 適合情境 |
|---|---|
| 🎞️ 投影片模式（預設） | 照著 Agent 規劃走完，一頁一頁翻 |
| 📄 文件模式 | 回看、快速瀏覽、搜尋 |

### 投影片模式（D-029 路由）

- **主單位 = 一份 station 檔**：每份 `stations/s{NN}-slug.md` 對應一個投影片「站點」
- **URL 路由用 stable station id**：`/tutorial/{workspace_id}/{station_id}` —— 例如 `/tutorial/ws-abc/s02-mqtt-client`
  - station_id 是 D-029 §7.4 的 `s{NN}-slug` 穩定錨點，跨 session / 分享連結都可重現
  - **禁用 numeric index 當 URL key**：index 會因 stations 重排而漂移，stable id 不會
- **主翻頁（left/right 方向鍵 / 翻頁鈕）**：走 `route.json.stations[*]` 順序，跳到前/後一份 station 檔
- **檔內次級切頁**：單一 station 檔內若有多個 `###` 分頁符，`up/down` 或 PageUp/PageDown 在同檔內滑動；主翻頁按鍵仍跳整份檔
- **進度條**：顯示「第 N / M 站」與當前 station 檔內的次級頁碼（若有）
- **MOC `tutorial.md`** 作為「目錄首頁」：路由 `/tutorial/{workspace_id}/index`，點擊站名跳對應 station 檔

### 文件模式

- **載入當前 station 檔全文**（或可切「全站合併」模式載入 MOC + 所有檔案連 scroll）
- 站牌當 anchor；`###` 當 scroll 內錨
- 搜尋跨當前 workspace 所有 station 檔 + MOC

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
  "current_station_id": "s02-mqtt-client",
  "completed_station_ids": ["s01-repo-overview"],
  "checkpoints": {
    "station-1-check": { "done": true, "ts": "..." }
  },
  "quizzes": {
    "s1-q1": { "answer": "b", "correct": true, "attempts": 1 }
  }
}
```

- **progress key 用 stable station id**（D-029）：不用整數 index，因 stations 重排（未來 regenerate）時 index 會漂移、stable id 不會
- `current_station_id` / `completed_station_ids` 都對應 route.json 的 `stations[*].station_id`

### 解鎖邏輯（Phase 1）

一站「完成」 = Checkpoint 全勾 + 所有 `<Quiz>` 答對 → 解鎖 route.json 順序上的下一個 station_id

### 已完成站可回看
狀態顯示已通過，QA 保留答題紀錄；URL 直接跳對應 station_id 不會被解鎖邏輯擋（回看模式）。

---

## 六、Markdown Generator 端的責任

Module 5 產出多檔教材時必須（詳見 `module-5-generator.md`）：

1. 每份 station 檔下至少 1 個 `<Checkpoint>` 或 `<Quiz>`
2. 每個 `<Quiz>` 給 `id` 和 `correct`
3. `###` 當檔內次級分頁符，一頁內容量 ≤ 約 300 字或 1 個程式碼區塊
4. `route.json` 的 `stations[n].required_checks` 列出本站必過 id 清單
5. `route.json.stations[n].station_id` 用 D-029 §7.4 stable id 格式（`s{NN}-slug`），`file_path` 用相對 `route.json` 的相對路徑
6. **標準 markdown link**：跨站連結用 `[text](./s03-broker-selection.md)`（同 `stations/` 目錄下平級），**禁用** `[[wikilinks]]`

### route.json 契約（多檔結構）

詳見 `module-5-generator.md §八`：

```json
{
  "stations": [
    {
      "station_id": "s02-mqtt-client",
      "index": 2,
      "title": "MQTT Client",
      "duration": 15,
      "file_path": "stations/s02-mqtt-client.md",
      "required_checks": ["station-2-check", "s2-q1"],
      "related_stations": ["s01-repo-overview"],
      "degraded": false
    }
  ]
}
```

- `station_id` 是**跨 session stable** 的主 key；前端路由、progress.json、Q&A `add_to_kb` 引用都用它
- `file_path` 由前端 loader 相對 `route.json` 解析；只允許 `stations/` 下平級檔（禁跨目錄）
- 舊 `id`（整數）與 `markdown_anchor` 欄位已移除

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
| MOC 連結指向不存在的 station 檔（degraded 寫失敗 / slug 不一致） | Generator validator 階段驗 MOC 每條連結都能 resolve；前端載入時 404 顯示「本站產出失敗，請重跑」 |
| Slug 碰撞導致 frontmatter station_id 與檔名不一致 | Generator 負責 collision handling（`-2`, `-3` 後綴，見 module-5-generator §7.4）；前端單純信 frontmatter |

---

## 九、實作順序建議（對齊 Module 5 + Module 7）

### P0 — MVP 必做
1. Module 5 Generator 產出多檔教材（MOC + `stations/s{NN}-slug.md`）含 `<Checkpoint>` 與 `<Quiz>`
2. Module 7 前端 `@nuxtjs/mdc` 掛 `Checkpoint.vue` / `Quiz.vue` / `QAEntry.vue`
3. 投影片模式檔案級路由（`/tutorial/{workspace_id}/{station_id}`）+ prev/next 按鈕走 `route.json.stations[*]` 順序
4. 檔內 `###` 次級分頁翻頁（up/down）
5. frontmatter parser → `<StationLayout>` 外殼（標題、時長、degraded badge）
6. MOC 渲染（站列表 + 連結 + `<QAEntry>`）
7. `progress.json` 讀寫（key 用 `station_id`）+ 解鎖邏輯

### P1 — 時間夠加
8. 文件模式切換
9. `<CodeRef>` + side panel 檔案瀏覽
10. `<Reveal>` 漸進揭露

### P2+ — 見第七節
