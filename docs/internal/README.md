# docs/internal — 內部工作史封存

這裡放 codebus 開發過程的**內部工作文件**：規劃、調查、spike 紀錄、決策史。對「使用 codebus」沒有必要，但保留作為**為什麼這樣做**的可追溯紀錄。

對外文件在上一層 `docs/`：

- [`../security.md`](../security.md) — 安全模型、各 provider 隔離強度
- [`../codebus-ai-architecture.md`](../codebus-ai-architecture.md) — AI 行為與架構解析（含流程圖）

## 檔案分類

| 樣式 | 是什麼 | 性質 |
|---|---|---|
| `BACKLOG.md` | 開放工作項目索引，每列連到對應 `*-backlog.md` | **活文件**（持續更新） |
| `<date>-<slug>-backlog.md` | 單一工作項目的細節與決策 | 半活：項目 ship 後即定格 |
| `<date>-<slug>-discussion.md` | 設計討論的結論 | 不可變歷史紀錄 |
| `<date>-<slug>-spike.md` / `spike-artifacts/` | 技術探針與其原始輸出（JSONL） | 不可變歷史紀錄 |
| `<date>-<slug>-diagnosis.md` | 問題根因調查 | 不可變歷史紀錄 |
| `v3-roadmap.md` / `v3-app-roadmap.md` | 早期規劃藍圖 | **部分過時**，當前狀態以 `openspec/specs/` + `BACKLOG.md` 為準 |

## 保留政策

- `BACKLOG.md` 是唯一的活索引；它只連結 `*-backlog.md` 子集。spike / discussion / diagnosis 等是**不可變歷史**，不從 BACKLOG 索引、也不刪除。
- 規格的當前真實狀態看 `openspec/specs/`（capability 規格）與 `openspec/changes/archive/`（已歸檔的變更）。
- dated 檔一旦寫定就**不回頭改內容**；後續若有推翻，在新的 dated 檔記錄，而非改舊檔。
