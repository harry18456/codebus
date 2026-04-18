# Golden Sample: Timeline + Google Drive Adapter

> **Task**: 在 Timeline 專案新增 Google Drive Adapter 同步功能
> **Repo**: `~/projects/timeline`（Nuxt 4 + TS + PWA + Pinia）
> **用途**: 評估 Explorer Agent 產出路線的 benchmark（D-004 / D-006）
> **建立日期**: 2026-04-17
> **狀態**: 初稿，待 Harry review

---

## 一、人工分析 — 這個任務要懂什麼

新加一個 Storage Adapter 的前置知識：

1. **Storage 介面契約**（`IStorageService`）—— 新 Adapter 要實作哪些 method
2. **現有 Adapter 兩種實作模式**—— `MockStorageAdapter`（純記憶體）vs `LocalFileAdapter`（File System Access API + IndexedDB）
3. **Adapter 如何被載入使用**—— `useStorage` composable、什麼時候 init
4. **資料模型**—— `TimelineConfig` / `EventNode` / `AppSettings` 的形狀
5. **前端呼叫位置**—— 哪些 store / page 會打 storage，測試時要跑到這些路徑

---

## 二、理想路線（4-5 站）

### 🚏 站 1：Storage 介面契約

**目標**：理解要實作什麼介面

**核心檔案**
- `app/types/index.ts` — `IStorageService` 介面定義（line 109-122）與相關資料型別（`TimelineConfig`, `EventNode`, `AppSettings`）

**該教什麼**
- `IStorageService` 的 12 個 method 及語意（timeline / node / settings / media 四組）
- 每個 method 的輸入輸出型別
- Async 回傳形狀（Promise<void> vs Promise<T | null>）

**檢核**
- 能列出新 Adapter 必須實作的 method 數量
- 能說出 `getTimeline` 回 null 的語意（找不到）

---

### 🚏 站 2：Mock 實作 — 最小可行版

**目標**：從純記憶體版本理解「要寫什麼」

**核心檔案**
- `app/services/MockStorageAdapter.ts` — in-memory 實作 + 預設 demo 資料

**該教什麼**
- 每個 method 怎麼把 interface 落成實作
- 狀態存在 instance field 的策略
- Mock 資料結構（可當新 Adapter 初次寫入的參考）

**檢核**
- 能指出 `saveTimeline` / `saveNode` 的相依關係（node 必須屬於已存在的 timeline）

---

### 🚏 站 3：LocalFile 實作 — 真正持久化

**目標**：理解「真的寫檔」要處理什麼——這是 Google Drive Adapter 最接近的參照

**核心檔案**
- `app/services/LocalFileAdapter.ts` — File System Access API + IndexedDB handle cache + FileSystemObserver

**該教什麼**
- File handle 授權流程（`showDirectoryPicker` → 存 IndexedDB）
- 檔案 I/O 錯誤處理模式（NotFoundError / permission denied）
- 遠端同步相關概念：**change detection**（這裡用 FileSystemObserver，GDrive 版要用 changes API 或 polling）
- 序列化策略（JSON.stringify + 檔案分片：timeline.json + nodes/{id}.md）
- Media file 處理（binary，走不同 method）

**檢核**
- 能描述 handle 遺失時的重新授權流程
- 能說出 timeline / node 檔案在磁碟上的對應結構

---

### 🚏 站 4：Adapter 如何被掛載使用

**目標**：看到新 Adapter 寫完後，要怎麼插進 App

**核心檔案**
- `app/composables/useStorage.ts` — composable 暴露 `$storage`, `$storageReady`, `$initStorage`, `$changeFolder`
- `app/stores/timeline.ts` — 典型 consumer（看 `useStorage()` 在 store 內怎麼用）
- （若存在）Adapter 選擇 / 註冊的入口點 plugin

**該教什麼**
- 何時初始化 Adapter（App startup vs 使用者選資料夾後）
- `$storageReady` 狀態在 UI 的角色（未 ready 要擋住寫入）
- 要加 Google Drive 選項的話，要動到哪幾個入口

**檢核**
- 能說出 `useTimelineStore.fetchTimelines()` 背後發生的事
- 能指出 Adapter 切換邏輯位於何處

---

### 🚏 站 5：設計練習 — 草擬 GoogleDriveAdapter（Quiz 站）

**目標**：前四站的整合測驗

**該做什麼**（純 Markdown + Checkpoint + Quiz，不寫 code）
- 列出 Google Drive API 對應 `IStorageService` 每個 method 的實作策略
- 指認需處理但前幾站沒出現的新挑戰：OAuth token refresh、配額、離線衝突
- 判斷題：「可以直接抄 LocalFileAdapter 的 change detection 嗎？」（答：不行，FileSystemObserver 是本機 fs-only）

**Quiz 範例**
- Q1：新 Adapter 必須實作幾個 method？（a) 8 b) 10 c) 12 d) 14）
- Q2：Media file 該怎麼存？
- Q3：authorization token 過期時要處理哪幾個 method？

---

## 三、評分預期（Explorer Agent 跑完後對照）

### 必達（核心檔案召回率）
- [x] `app/types/index.ts`
- [x] `app/services/MockStorageAdapter.ts`
- [x] `app/services/LocalFileAdapter.ts`
- [x] `app/composables/useStorage.ts`
- [x] `app/stores/timeline.ts`（至少一個 consumer）

**滿分**：5/5 檔案都命中。**合格**：4/5。

### 可加分（非必要但加分）
- `app/stores/node.ts` / `app/stores/settings.ts`（另外的 consumer，讓 Module 5 能舉更多呼叫例子）
- Adapter 選擇 / DI 入口檔案（如果有的話）

### 雜訊扣分（不該進路線的）
- UI component 細節（`components/`）— 這個任務不需要改 UI
- 事件卡片樣式、Tailwind config
- i18n、README、package.json
- `.nuxt/` 生成檔

### 順序合理性（人工 rubric 1-5）
- ✅ 介面 → Mock → Real → 掛載 → 練習，依賴層次清楚
- ❌ 跳過介面直接讀 LocalFileAdapter → 扣分
- ❌ 把 UI 組件排進來 → 扣分

### 粒度 rubric
- 每站 1 個主角檔案 + 最多 1-2 個輔助 ✅
- 一站塞 5 個檔案 ❌

### 新人可讀性 rubric
- 每站都有「為什麼排這站」的連結（例如「因為新 Adapter 要同樣實作這些介面」）
- 程式碼片段短（≤ 30 行），聚焦要點

---

## 四、自動評分公式（草稿）

```
recall = |hits ∩ must_have| / |must_have|           # 目標 ≥ 0.8
noise  = |extras - nice_to_have| / |extras|         # 目標 ≤ 0.3
depth  = |resolved_dependencies| / |dep_chain|      # 目標 ≥ 0.7

score = 0.5 * recall + 0.3 * (1 - noise) + 0.2 * depth
```

**regression gate**：改 prompt 後 score 下降 > 5% 必 review。

---

## 五、Demo 劇本（連動 D-008 Agent console）

跑 Explorer Agent 時，觀眾應該看到類似：

```
[Step 1] Thought: 任務是加 Storage Adapter，先找 Storage 介面定義
[Step 1] Action: grep("IStorageService", type="ts")
[Step 1] Observation: types/index.ts:109 找到 interface

[Step 2] Thought: 找到介面，看 method 簽名決定探索深度
[Step 2] Action: read_file("app/types/index.ts", lines=[109, 122])
[Step 2] Judge: relevance=0.98, 必進路線

[Step 3] Thought: 介面有 12 method，找現有實作當參考
[Step 3] Action: find_callers("IStorageService")
[Step 3] Observation: MockStorageAdapter, LocalFileAdapter 兩個實作

[Step 4] Thought: Mock 是最小版本，先讀懂最簡單的
[Step 4] Action: read_file("MockStorageAdapter.ts")
...

[Step N] Coverage Check: 路線涵蓋介面/實作/使用點，無 gap
[Step N+1] Done: 產出 5 站路線
```

這段即時 stream 到前端 Agent console = Demo 金句素材。

---

## 六、待 Harry review 的點

- [ ] 五站數量是否恰當，或合併成 4 站？
- [ ] 站 5（Quiz 練習）是否保留，或全部 Checkpoint 無 Quiz？
- [ ] 資料模型（EventNode / TimelineConfig）是否該單獨成站，還是併進站 1？
- [ ] OAuth / 配額屬於 MVP 教材要教，還是站 5 提一下即可？
