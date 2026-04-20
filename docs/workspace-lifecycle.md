# Workspace Lifecycle Spec — 資料分級 / R-00 / 遺失恢復

> CodeBus workspace 的資料存放規則、Start Page（R-00）介面、以及檔案遺失時的恢復策略。
> 關聯決策：D-002（雙模 discriminator）、D-023（Topic 綁容器）、D-024（資料分級儲存）、D-025（整合性與遺失恢復）、D-029（`tutorials/{task_id}/stations/` 多檔結構）。
> 對應 UI：R-00 Start Page（Phase A 待 Design mockup）、O-01 修復子流程。

---

## 一、範圍

本文件定義三件事：

1. **資料分級** — App-level / Workspace-level / Pointer 三層各自住哪、搬家/備份單位為何
2. **R-00 Start Page** — App 啟動後的落地畫面，處理 workspace 選擇、建立、恢復入口
3. **遺失恢復** — 兩邊分離設計（Folder pointer + 實質資料）帶來的六種遺失情境與處理規則

**不在範圍內**
- 授權決策（`authorization.md` 的 O-01）
- Sanitizer 規則變動（`sanitizer.md §六`）
- Workspace 內部的 audit log schema（各層 audit spec）

---

## 二、資料分級（D-024）

### 三層切分

| 層級 | 位置 | 內容 | 搬家/備份單位 |
|---|---|---|---|
| **App-level** | `~/.codebus/` 根 | `authorization_audit.jsonl` / `sanitizer.local.yaml` 全域預設 / `workspaces.json` registry | 跟著 user home |
| **Workspace-level** | Topic: 容器資料夾內 / Folder: `~/.codebus/workspaces/{id}/` | KB (Qdrant) / 六層 audit JSONL / tutorials / per-workspace sanitizer 覆蓋 / `.codebus-workspace.json` metadata | 單一資料夾搬走即完 |
| **Pointer**（僅 Folder mode） | 使用者 repo 根 `.codebus/pointer.json` | `{ workspace_id, type, created_at }`（< 1KB） | 跟著 repo |

### 切分原則
- **Qdrant storage 不進使用者 repo**（可能數百 MB，git 會卡）
- **Workspace-level audit 跟著 workspace 搬家**（folder mode 也是搬 `~/.codebus/workspaces/{id}/` 這份）
- **App-level audit 跨 workspace 所以住 user home**
- **Pointer 是視覺錨點，不是資料本身** — 使用者在 repo 看得到「這個 repo 有 CodeBus workspace」

### App-level 完整結構
```
~/.codebus/
├── authorization_audit.jsonl      # 跨 workspace 授權紀錄
├── sanitizer.local.yaml           # 全域 sanitizer 預設
├── sanitizer_rules_meta.json      # 當前 rules 版本（authorization.md §六）
├── workspaces.json                # workspace registry
├── topics/                        # Topic mode 容器的家
│   └── {slug}/                    # 每個 topic workspace
└── workspaces/                    # Folder mode 實質資料的家
    └── {workspace_id}/
```

---

## 三、Topic Mode Layout（D-023）

容器資料夾 = workspace root = 所有 per-workspace 資源的家。

```
~/.codebus/topics/uv/
├── kb/                            # Qdrant collection storage
├── tutorials/
│   └── {task_id}/                 # 多檔教材（D-029）
│       ├── tutorial.md            # MOC 索引
│       ├── stations/              # 每站一檔
│       │   ├── s01-<slug>.md
│       │   ├── s02-<slug>.md
│       │   └── ...
│       ├── route.json
│       ├── progress.json
│       └── generator_log.jsonl
├── sanitize_audit.jsonl           ┐
├── tool_audit.jsonl               │
├── kb_growth.jsonl                │ 六層 workspace-level audit
├── reasoning_log.jsonl            │
├── token_usage.jsonl              │
├── llm_calls.jsonl                ┘
├── sanitizer.local.yaml           # (選) per-workspace 覆蓋
├── README.txt                     # 誤刪防線
└── .codebus-workspace.json        # metadata
```

### 隱式建立預設
首次建立 Topic workspace 時 App 自動在 `~/.codebus/topics/{slug}/` 建好（slug = topic 正規化後的短碼）。O-01 modal 顯示：

> 將在 `~/.codebus/topics/uv/` 建立 workspace — 包含知識庫、教材、稽核紀錄。
> [📁 開啟資料夾] [📍 變更位置] [繼續]

進階使用者點「變更位置」可選其他路徑（例如 `~/Documents/codebus/uv/` 方便跟 Dropbox / iCloud 同步）。

### `README.txt` 內容（誤刪防線）
```
此資料夾由 CodeBus 管理（workspace_id: ws_xxx / type: topic）。

包含：
- 知識庫（kb/）
- 學習教材（tutorials/）
- 稽核紀錄（*.jsonl）

直接刪除會造成稽核紀錄永久遺失。
正確做法：從 CodeBus App → Settings → 「刪除此 workspace」。

如不慎刪除，請前往 App → Start Page 查看恢復選項。
```

---

## 四、Folder Mode Layout（D-024 混合策略）

Pointer 在 repo 根，實質資料在 `~/.codebus/workspaces/{id}/`。

### Repo 端（pointer）
```
~/projects/timeline/                       # 使用者的 repo
├── src/
├── .git/
├── .gitignore                             # 建議加入 .codebus/
└── .codebus/
    ├── pointer.json
    └── .gitignore                         # ignore 自己 + README 說明
```

`pointer.json`：
```json
{
  "workspace_id": "ws_abc123",
  "type": "folder",
  "created_at": "2026-04-19T10:30:00Z",
  "codebus_version": "0.1.0"
}
```

**`.codebus/.gitignore` 預設**：
```
*
!.gitignore
!README.txt
```
（預設 ignore 全部；進階使用者可手動調整 commit pointer 讓隊友共用 workspace 概念）

### 實質資料端
```
~/.codebus/workspaces/ws_abc123/
├── kb/
├── tutorials/                      # 多檔教材結構同 §三（D-029）
│   └── {task_id}/
│       ├── tutorial.md
│       ├── stations/
│       │   └── s0X-<slug>.md
│       ├── route.json
│       ├── progress.json
│       └── generator_log.jsonl
├── sanitize_audit.jsonl
├── tool_audit.jsonl
├── kb_growth.jsonl
├── reasoning_log.jsonl
├── token_usage.jsonl
├── llm_calls.jsonl
├── sanitizer.local.yaml
└── .codebus-workspace.json              # metadata（含 origin_path 反向指回 repo）
```

### `.codebus-workspace.json` schema（兩 mode 共用）
```json
{
  "workspace_id": "ws_abc123",
  "type": "folder",
  "created_at": "2026-04-19T10:30:00Z",
  "last_opened_at": "2026-04-19T15:22:00Z",
  "origin_path": "~/projects/timeline",        // folder only; topic 為 null
  "topic_seed": null,                          // topic only; folder 為 null
  "codebus_version": "0.1.0"
}
```

`topic_seed`（Topic mode）：
```json
{
  "query": "uv python package manager",
  "seed_urls": ["https://docs.astral.sh/uv/"],
  "domain_allowlist": ["docs.astral.sh", "github.com/astral-sh"]
}
```

---

## 五、核心不變式

1. **Workspace root 語意兩模式統一** — 有一個實體資料夾當錨點（topic = 容器、folder = repo）
2. **Qdrant storage 不進使用者 repo** — 只放輕量 pointer
3. **App-level audit 跨 workspace；workspace-level audit 跟著 workspace 搬家**
4. **Topic 搬家一 dir 即完；Folder 搬家需 pointer + 實質資料兩處**
5. **永遠不靜默修復** — 任何不一致在 R-00 讓使用者看到決策點
6. **永遠不重建 audit log** — audit 遺失 = 合規紀錄斷鏈，明確告知而非偷補

---

## 六、R-00 Start Page Spec

App 啟動後的落地畫面，不是 modal、是獨立 route。

### 6.1 狀態 a — 已有 workspace（returning）

```
┌────────────────────────────────────────────────────┐
│ CodeBus                         [⚙ Settings] [?]   │
├────────────────────────────────────────────────────┤
│                                                    │
│  Recent workspaces                                 │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐            │
│  │timeline │  │uv-learn │  │ssd-kb   │ [+ 建立]   │
│  │📁 folder│  │🌐 topic │  │🌐 topic │            │
│  │2d ago   │  │5h ago   │  │3d ago   │            │
│  └─────────┘  └─────────┘  └─────────┘            │
│                                                    │
│  ⚠️ 1 個 workspace 需要修復 →                      │
│                                                    │
│  🎯 試試 Demo workspace（合成 fixture）            │
│                                                    │
└────────────────────────────────────────────────────┘
```

### 6.2 狀態 b — 第一次啟動（fresh install）

```
┌────────────────────────────────────────────────────┐
│               🚌 CodeBus                           │
│          給它目的地，它帶你上車                    │
│                                                    │
│  ┌──────────────────────────────────────────────┐  │
│  │  📁 選個專案資料夾開始            [primary]  │  │
│  └──────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────┐  │
│  │  🎯 試試 Demo workspace（30 秒走一輪）       │  │
│  └──────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────┐  │
│  │  🌐 Topic 模式 · Phase 2 敬請期待 [disabled] │  │
│  └──────────────────────────────────────────────┘  │
│                                                    │
│  🛡️ 原值留在本機、不進 LLM、不寫進 KB             │
│                                                    │
└────────────────────────────────────────────────────┘
```

### 6.3 Workspace 卡片元素
- **名稱** — topic 用 `topic_seed.query` 或 slug；folder 用 repo 目錄名
- **Type badge** — 📁 folder / 🌐 topic
- **Last opened** — 相對時間（`2d ago`）
- **健全性** — 正常無標記；異常顯示 🔴 / 🟡 / 🟠 / ⚫（見 §七）
- **右鍵 menu** — 開啟 / 在檔案瀏覽器顯示 / 重新命名 / 刪除

### 6.4 「試試 Demo workspace」行為
- 點擊 → 自動建 folder workspace 指向 `tests/golden/demo-synthetic/repo/`
- 或 copy fixture 到 `~/.codebus/workspaces/demo_synthetic/`（避免寫入 test fixture）
- 走完整 O-01 → scan → explore → tutorial 流程
- 當作新手導覽替代（不做獨立互動 tour，跳過率高 + 費工）
- 同一份素材也是比賽 demo / regression 用（一石二鳥）

### 6.5 路由
```
App 啟動 → R-00 Start → (選/建/續) → O-01 授權 → R-01 主畫面
                 ↑                                    ↓
                 └────── cancel / workspace 切換 ─────┘
```

R-00 永遠可達（R-01 右上角「切換 workspace」→ 回 R-00）。

---

## 七、遺失情境與恢復（D-025）

### 7.1 六種情境

| 代號 | 情境 | 觸發 | 偵測時機 |
|---|---|---|---|
| **A** Pointer 孤 | folder: repo `.codebus/pointer.json` 在、`~/.codebus/workspaces/{id}/` 不在 | 使用者清 `~/.codebus/` / 換機器沒帶 | 開啟該 workspace 時 |
| **B** 實質孤 | folder: `~/.codebus/workspaces/{id}/` 在、repo pointer 不在 | 使用者刪 repo / git clean 帶走 `.codebus/` | 啟動時孤兒掃描 |
| **C** Path 不一致 | `.codebus-workspace.json.origin_path` ≠ pointer 所在位置 | repo 搬家 | 開啟時比對 |
| **D** Topic 容器遺失 | `~/.codebus/topics/{slug}/` 被刪 | 使用者誤刪 | 啟動時 registry 驗證 |
| **E** Registry 遺失 | `~/.codebus/workspaces.json` 不存在或解析失敗 | 磁碟錯 / 誤刪 | 啟動時 |
| **F** App-level audit 遺失 | `~/.codebus/authorization_audit.jsonl` 不存在 | 磁碟錯 / 誤刪 | 啟動時 |

### 7.2 修復選項矩陣

| 情境 | 修復選項 | UI 位置 |
|---|---|---|
| **A** Pointer 孤 | (1) 重新 scan 建庫（audit 斷層）(2) 視為新 workspace 重授權 (3) 移除 pointer | R-00 卡片點開後 → 修復頁 |
| **B** 實質孤 | (1) 指定新 repo 位置重建 pointer (2) 保留為 "detached"（可讀 audit 不可 scan）(3) 一起刪 | R-00 孤兒通知 → 修復頁 |
| **C** Path 不一致 | 彈 modal「偵測到 repo 移動：A → B，更新記錄？」+ 寫 `workspace_path_updated` audit | 開啟時 modal |
| **D** Topic 容器遺失 | (1) 從 `.codebus-workspace.json.topic_seed` 重爬（如有備份）(2) 移除 | R-00 卡片點開後 |
| **E** Registry 遺失 | Walk `~/.codebus/workspaces/*/` + `~/.codebus/topics/*/` 讀 `.codebus-workspace.json` 重建 + 寫 `registry_rebuilt` audit | 啟動時自動 + 通知 |
| **F** App-level audit 遺失 | **不補寫**，新檔開頭記 `audit_log_initialized{prior_log_lost:true}` + R-00 全域 warning banner | 啟動時 |

### 7.3 五條鐵律

1. **永遠不靜默修復** — 任何不一致在 R-00 讓使用者看到決策點（情境 E 例外：registry 可安全重建，但要寫 audit 讓使用者看得到）
2. **永遠不重建 audit log 內容** — audit 遺失就是合規紀錄斷鏈，寧可明確告知也不偷偷補一個騙人
3. **永遠不自動刪** — 只標 broken / detached，刪除決策一定走使用者介入 + 二次確認
4. **啟動時 integrity check** — walk `workspaces.json` 驗兩邊健在，broken 標記而不 crash
5. **孤兒掃描納入啟動流程** — `~/.codebus/workspaces/*/` + `~/.codebus/topics/*/` 沒在 registry 的提示使用者收編 / 刪除

### 7.4 R-00 卡片健全性狀態

| Badge | 語意 | 對應情境 | 點擊後行為 |
|---|---|---|---|
| （無） | 正常 | — | 進 R-01 |
| 🔴 `pointer_orphan` | pointer 在、實質資料無 | A | 進修復頁（三選一） |
| 🟡 `data_orphan` | 實質資料在、pointer 無 | B | 進修復頁（三選一） |
| 🟠 `path_moved` | path 不一致 | C | 進確認 modal |
| 🔵 `detached` | 使用者選擇保留實質資料 + 刪 pointer 後 | B 後續 | 只能讀 audit、不能 scan |
| ⚫ `tombstone` | 使用者確認刪除後墓碑 N 天（預設 14）供後悔 | — | 復原 / 永久刪除 |

---

## 八、新增 Audit 事件

以下事件寫入 `~/.codebus/authorization_audit.jsonl`（App-level，跟 workspace 生命週期有關所有跨 workspace 事件都在這）。

### `workspace_path_updated`
```json
{
  "ts": "...",
  "event": "workspace_path_updated",
  "workspace_id": "ws_abc123",
  "old_path": "~/old-location/timeline",
  "new_path": "~/projects/timeline",
  "trigger": "path_mismatch_detected_at_open"
}
```

### `registry_rebuilt`
```json
{
  "ts": "...",
  "event": "registry_rebuilt",
  "reason": "registry_missing" | "registry_corrupt",
  "recovered_workspaces": 5,
  "orphans_found": 2
}
```

### `audit_log_initialized`
```json
{
  "ts": "...",
  "event": "audit_log_initialized",
  "prior_log_lost": true,
  "codebus_version": "0.1.0"
}
```
（正常啟動不寫此事件；只在偵測到 `authorization_audit.jsonl` 先前不存在時寫）

### `workspace_tombstoned`
```json
{
  "ts": "...",
  "event": "workspace_tombstoned",
  "workspace_id": "ws_abc123",
  "workspace_type": "folder",
  "tombstone_expires_at": "2026-05-03T..."  // +14 天
}
```

### `workspace_deleted`（墓碑期滿或使用者選擇永久刪）
```json
{
  "ts": "...",
  "event": "workspace_deleted",
  "workspace_id": "ws_abc123",
  "trigger": "tombstone_expired" | "user_confirmed"
}
```

---

## 九、MVP 範圍

### 必做（Phase A + B）
- 資料分級（App / Workspace / Pointer 三層）完全落實
- R-00 Start Page（狀態 a + b）
- 啟動 integrity check + 情境 A / C / E / F 偵測與處理
- Demo workspace 入口（使用合成 fixture）
- 墓碑機制（情境 tombstone）

### MVP 不做（Phase 2+）
- Topic mode 全套（D-002）— 但 workspace_type discriminator + `.codebus-workspace.json.topic_seed` 欄位 day 1 寫入 schema
- 孤兒掃描修復頁（情境 B 只做偵測 + 通知，修復 UX Phase 2 補）
- 跨機器備份/匯入/匯出（Settings 加一鍵打包 zip）
- Workspace rename 後自動重建 pointer（情境 C 自動偵測）
- Workspace 合併 / 分拆（用不到）

### 實作檢核點
- [ ] `~/.codebus/workspaces.json` schema + migration 機制
- [ ] `.codebus-workspace.json` schema + Pydantic model
- [ ] `pointer.json` schema
- [ ] R-00 route 實作（Nuxt3 page）
- [ ] Integrity check 函式（walk registry 驗 path 存在）
- [ ] 孤兒掃描函式（walk `~/.codebus/workspaces/*/` + `topics/*/`）
- [ ] 六種情境的 R-00 UI 狀態
- [ ] 墓碑清理排程（啟動時檢查墓碑過期 → 永久刪）

---

## 十、Cross-reference

| 主題 | 相關文件 |
|---|---|
| `workspace_type` discriminator | D-002 / `authorization.md §一` / `sidecar-api.md §三` / `tool-sandbox.md §三` |
| O-01 cancel 返回 | `authorization.md §七` → R-00（本文件 §六） |
| Sanitizer per-workspace config 位置 | `sanitizer.md §五` + 本文件 §三 / §四 |
| Audit log 七層 | `security.md §二` + 本文件 §八（新增事件） |
| 隱式容器誤刪防線 | 本文件 §三 README.txt + §七 情境 D |
