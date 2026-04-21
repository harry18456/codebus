## Context

Module 1 Scanner 的完整 spec（`docs/module-1-scanner.md`）涵蓋 11 個 P0/P1 項，合計 ~4.75 人日；若一次推進會同時觸到 Sanitizer（D-015）/ pygit2（D-016）/ monorepo 偵測 / SSE progress 四個主題，單一 change 過大難收、也拖 sanitizer rules version 的獨立節奏。

M1 power-on 已提供：FastAPI app factory + bearer middleware（`sidecar-runtime`）、`ensure_in_workspace` helper（`tool-sandbox`）、`AsyncQdrantClient` lifecycle（`qdrant-client` / D-027）、Sanitizer Pass 1 `scrub()` API（`sanitizer`）。Scanner 是這四個能力的第一個實際 consumer。

下游消費者：
- **Module 2 KB Builder**（後續 change）：`ScanResult.files[]` → chunk → embedding → Qdrant
- **Module 4 Explorer Agent**（後續 change）：`ScanResult.content_summary` 啟動時讀取決定策略（`docs/agent-explorer-spec.md`）

## Goals / Non-Goals

**Goals:**

- 讓 Tauri / 後續 change 可透過 `POST /scan` 拿到一個 **完整、可序列化、可重複產出** 的 `ScanResult`，涵蓋遍歷 / 分類 / 編碼 / 語言識別 / content-type summary 五段。
- 把 `ScanResult` 的 schema 一次定到位（含 Sanitizer / Git / Monorepo 欄位的 stub 預設值），讓後續 change 疊加時**不破壞**既有 JSON consumer。
- 鎖定 `workspace_type` discriminator 在 API 層的行為（`"folder"` 處理、`"topic"` → `501 Not Implemented`），滿足 D-002 day-1 契約。

**Non-Goals:**

- 不做 Sanitizer orchestration 串接（D-015）：`FileEntry.content` 留解碼後原文、`sanitize_stats` 留空 dict。本 change 的 `/scan` 輸出**不得被 LLM 呼叫鏈消費**；下游 consumer（Module 2）在 Sanitizer 串接 change 完成前不得 import scanner output。
- 不做 Git metadata（pygit2）：`ScanResult.git` 留 `None`。
- 不做 Monorepo 偵測：`is_monorepo: false` / `sub_packages: []` 預設。
- 不做 SSE progress event：本 change 回同步 JSON；大 repo（> 5000 檔）可能阻塞 request `> 30s`，由後續 SSE change 修。
- 不做前端整合（`web/` / `tauri/`）。

## Decisions

### `ScanResult` schema 一次到位，Sanitizer / Git / Monorepo 欄位留 stub 預設值

為避免後續 change 造成 JSON breaking change，本 change 直接定下完整 `ScanResult` schema（對齊 `docs/module-1-scanner.md §十一`），含：
- `FileEntry.sanitize_stats: dict[str, int]`（本 change 固定回 `{}`）
- `ScanResult.git: GitMeta | None`（本 change 固定回 `None`）
- `ScanResult.is_monorepo: bool` / `sub_packages: list[dict]`（本 change 固定回 `false` / `[]`）

替代方案考慮：
- **A. schema 分段推進**（本 change 只定 text / binary 欄位，Sanitizer change 再加 `sanitize_stats`）——拒絕：每個後續 change 都是 JSON additive change，累積成多次 consumer 適配成本。
- **B. 本方案 schema 一次定到位，內容分段填**——採用：schema 穩定，後續 change 只改「欄位是否為預設值」而非「欄位是否存在」。

### `pathspec` vs 自寫 gitignore 解析

採用 `pathspec`（`uv add pathspec`）。

替代方案：
- **自寫**：gitignore 的 negation / nested rule / glob semantics 邊界 case 多，自寫易漏。
- **`pygitignore`**：上次更新久、社群小。
- **`pathspec`**（採用）：標準 gitignore 語法、GitHub 組織維護、`pathspec.PathSpec.from_lines("gitwildmatch", ...)` 支援階層疊加。

### `charset-normalizer` 當 fallback chain 保底而非主判

採用 `charset-normalizer`（`uv add charset-normalizer`）當最後保底，**不當主路徑**。對齊 `docs/module-1-scanner.md §五` 的 fallback 順序：UTF-8 → UTF-16 BOM → Big5 → GBK → Shift_JIS → charset-normalizer 猜 → 全失敗判 binary。

替代：`chardet`（拒絕，維護慢 + 中文偏誤）；純 `charset-normalizer`（拒絕，猜測開銷大，對明顯 UTF-8 檔也要跑統計模型）。

### 同步 `/scan` endpoint，SSE 留待後續 change

本 change 的 `POST /scan` 一次回完整 `ScanResult` JSON。大 repo 會阻塞 HTTP client，但：
- MVP 的 demo repo（Timeline，~500 檔）預期 < 5s
- `sidecar-api.md §四` 的 progress event schema 已鎖定，SSE change 疊加時只需換 endpoint 實作，契約不變

替代：本 change 就做 SSE——拒絕，scope 膨脹，且 bearer + SSE 的 connection lifecycle 需獨立驗證。

### `workspace_type="topic"` 回 `501 Not Implemented`（非 `400`）

D-002 要求 discriminator day 1 進 schema。`topic` 不是**錯誤輸入**（schema 有效），是**尚未實作的分支**——語意上是 `501 Not Implemented`，非 `400 Bad Request`。FastAPI 端用 `raise HTTPException(status_code=501, detail="workspace_type='topic' not implemented in MVP")`。

### Symlink 預設不跟隨，resolve 後在 workspace 內才記錄

對齊 `docs/module-1-scanner.md §三` + `tool-sandbox` 不變式。
- `Path.rglob` 預設不跟隨 symlink：保留此行為。
- 遇 symlink entry：`Path.resolve(strict=False)` → 若解析結果仍在 `workspace_root` 內，記入 `Symlink(resolved_in_workspace=True)`；否則 `resolved_in_workspace=False`，**不 follow**、**不讀內容**。
- `follow_symlinks: true` config 選項**本 change 不做**（非 MVP）。

### Scanner 不是 tool，直接用 sandbox helper

Scanner 不呼叫 `ToolSandbox.register(...)` 也不出現在 tool registry——它是 tool 的上游，不是 tool 本身。但每個 path 操作必過 `ensure_in_workspace(path, ctx)`，共用紅隊 fixture（`sidecar/tests/sandbox/` 已就位的逃逸 case）。

## Risks / Trade-offs

- **[Risk] `/scan` 輸出含未 sanitize 內容** → **Mitigation**：在 `folder-scanner` spec 寫明「skeleton 階段 `/scan` 輸出僅供 scanner-sanitize-wiring change 之前的內部測試使用；任何跨 Sanitizer 進 LLM 呼叫鏈的 consumer 必須等 sanitize orchestration change」。此限制進 spec requirement 而非僅 docs note，違反即 analyzer flag。
- **[Risk] 大 repo 同步 `/scan` 阻塞** → **Mitigation**：在 spec 註明「> 5000 檔 repo 延遲可能 > 30s，由後續 SSE change 解」；測試 fixture 故意用小 repo（< 50 檔）驗骨架而非性能。
- **[Risk] `pathspec` 對極端 gitignore pattern 行為與 git 原生不 100% 一致** → **Mitigation**：覆蓋常見 pattern fixture（`node_modules/` / `*.log` / negation `!important.log`），不保證 gitignore 100% 語意等價，spec 明記「以 `pathspec` gitwildmatch 為準」。
- **[Risk] Schema 定太早，後續 Sanitizer / Git / Monorepo change 發現缺欄位** → **Mitigation**：schema 直接以 `docs/module-1-scanner.md §十一` 既有完整版為準（已經過設計討論），而非憑本 change 需要臆造；真有漏欄位走 `proposal` 流程 bump schema，避免 silent change。
- **[Trade-off] `workspace_type="topic"` 501 vs 400**：選 501 讓 client 能用 HTTP status 判「功能未實作 vs 請求無效」，代價是未來 topic 實作時 client 需更新預期 status code——可接受，因為 topic 實作本身就是大 change、client 必配合。

## Open Questions

無（本 change scope 明確；若實作期發現缺欄位，以 proposal 流程追加 change 而非擅自擴 schema）。
