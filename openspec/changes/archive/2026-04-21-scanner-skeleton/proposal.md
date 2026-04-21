## Why

M1 power-on 階段已完成 sidecar / Tauri / Qdrant client / Sanitizer 等基礎；下一步是把 repo 的原始檔案轉成結構化 `ScanResult` 餵給 Module 2 KB Builder。Scanner 是 Trust Layer 敘事的起點——沒有 Scanner，Explorer Agent 連第一個檔都看不到。

本 change 先交付 **Scanner 骨架**：能走完 repo、分類每個檔、輸出 `ScanResult` JSON，但暫不含 **Sanitizer Pass 1 orchestration 串接**、**Git metadata（pygit2）**、**Monorepo 子模組識別** 三段——各自另開 change 疊上來，避免單個 change 過大難收、也讓 Sanitizer orchestration 可跟隨 sanitizer rules version 獨立節奏。

關聯決策：D-002（雙模 discriminator day 1）、D-015（Sanitizer 單向替換）、D-017（Sandbox 整合）、D-018（Module 1 Scanner 細節定案）、D-027（Qdrant client wrapper 已就位供下游 Module 2 消費）。

## What Changes

- **新增 `folder-scanner` capability**：定義 `ScanResult` schema（`FileEntry` / `Symlink` / `ScanStats` / `ContentTypeSummary`）、檔案遍歷 / 分類 / 編碼 / 語言識別行為契約。
- **新增 `POST /scan` endpoint**（`sidecar-runtime` capability 擴充）：接 `{workspace_type, workspace_root}`，回同步 `ScanResult` JSON；bearer + loopback 紅線不鬆綁。SSE progress event 本 change **不做**。
- **Scanner 每個 entry 必過 `ensure_in_workspace(path, ctx)`**（D-017）；symlink 預設不跟隨，resolve 後仍在 workspace 內才記錄，否則 skip。
- **`workspace_type` discriminator day 1**（D-002）：MVP 只處理 `"folder"`，`"topic"` 回 `501 Not Implemented`（schema 不缺，未來加實作不 breaking）。
- **新依賴**：`pathspec`（gitignore 階層疊加）、`charset-normalizer`（encoding fallback 保底）。`pygit2` **不加**（留給 git metadata 後續 change）。

## Non-Goals

- ❌ **Sanitizer Pass 1 orchestration 串接**：`FileEntry.content` 是解碼後原文，`sanitize_stats` 留空 dict；另開 change。
- ❌ **Git metadata 收集**：`ScanResult.git` 留 `None`；另開 change（scanner-git-metadata 之類）。
- ❌ **Monorepo 子模組識別**：`is_monorepo` / `sub_packages` 留預設值（`false` / `[]`）；另開 change。
- ❌ **SSE progress event**（每 50 檔 emit 一次）：本 change 回同步 JSON；SSE 另開 change 疊加。
- ❌ **`.dockerignore` / `.codebusignore` 疊加**、**per-file blame**、**file watch / incremental scan**——全依 `docs/module-1-scanner.md §十五` 列的 MVP 不做項。
- ❌ **前端 UI 整合**：本 change 不動 `web/` / `tauri/`；`/scan` endpoint 只由後續 change / 手測呼叫。

## Capabilities

### New Capabilities

- `folder-scanner`: 檔案遍歷、分類、編碼 / 語言識別、`ScanResult` schema 與 `POST /scan` contract。**不含** Sanitizer / Git metadata / Monorepo / SSE，各留明確 stub 欄位讓後續 change 無 breaking 疊加。

### Modified Capabilities

- `sidecar-runtime`: 新增 `POST /scan` endpoint 契約（bearer + loopback 不變式繼承自既有 `/healthz` 規格）。

## Impact

- **Affected specs**:
  - 新增 `openspec/specs/folder-scanner/spec.md`
  - 修改 `openspec/specs/sidecar-runtime/spec.md`（新 `POST /scan` requirement）
- **Affected code**:
  - 新增 `sidecar/src/codebus_agent/scanner/` package（walk / classify / encoding / language / summary / models / service）
  - 新增 `sidecar/src/codebus_agent/api/scan.py` + 註冊於 `api/__init__.py`
  - 新增 `sidecar/tests/scanner/` 單測 + 極簡 fixture（`mini-ts-repo/` / `mini-py-repo/` / `mixed-encoding/` 的子集）
- **Dependencies**: `uv add pathspec charset-normalizer`
- **Audit / JSONL**: 本 change **不觸及**任何稽核層（Sanitizer / ToolSandbox audit 均由後續 change 串入）
