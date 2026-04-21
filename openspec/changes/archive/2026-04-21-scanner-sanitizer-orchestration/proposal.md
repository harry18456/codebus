## Why

`scanner-skeleton`（2026-04-21 archive）把 Sanitizer Pass 1 明確列為 deferred，
`FileEntry.content` 是**直接解碼後的原文**、`FileEntry.sanitize_stats` 永遠 `{}`。
骨架 spec 同時寫死了一條約束：**該輸出不得被任何路徑餵進 LLM call chain**，直到
本 change 落地為止。

這個約束現在把 `docs/implementation-plan.md §二 第三階段` 第 14 步「Module 2 KB
Builder P0」整個擋住——KB Builder 的 embed pipeline 就是 LLM call chain 的起點
（embedding provider），沒有 Pass 1 串進 scanner 就沒辦法合法地把 `FileEntry.content`
拿去 chunk + embed。同時這也呼應「五條強制規則」裡「Sanitizer Pass 1 + 2 必須在
第一次 LLM call 之前可用」（D-015）。

`sanitizer-safety-chain`（2026-04-21 archive）已經把 `SanitizerEngine.sanitize(text,
source=FileSource(...))` 的純函式 API 做完、`sanitize_audit.jsonl` writer 也在位；
scanner 端現在缺的只是「在 decode 之後、build FileEntry 之前多打一個 sanitize」
這一小步。關聯決策：D-015（三段 Sanitizer）、D-011（Sidecar 資安）、D-017
（Sandbox ToolContext）。

## What Changes

- **`scanner/service.py` 解碼後新增 Pass 1 階段**：每個 `kind="text"` 的 FileEntry
  在 decode 成功後、append 到 `ScanResult.files` 之前，先呼
  `ctx.sanitizer.sanitize(content, FileSource(path=rel_path))`。
- **`FileEntry.content` 語意翻轉**：從「解碼後原文」改為「Pass 1 sanitized 版本」。
  `oversized_preview` 若落地（現仍為 `None`，延續 skeleton 行為）未來也須走同條
  Pass 1。
- **`FileEntry.sanitize_stats` 填真實計數**：key = `AuditEntry.kind`（例如 `email`
  / `secret` / `domain`），value = 該檔內該 kind 的 placeholder 數；沒命中任何規則
  時留 `{}`。
- **`sanitize_audit.jsonl` 寫入串通**：scanner 單次 scan 內每筆 Pass 1 命中都走
  既有的 `SanitizeAuditLogger` 落地；`source.pass == "scanner"`；檔名為相對 path。
- **ToolContext 必帶 SanitizerEngine**：`sidecar/src/codebus_agent/sandbox.py` 的
  `ToolContext` 增加 `sanitizer: SanitizerEngine` 欄位（若 `sanitizer-safety-chain`
  已塞過則視為 no-op；本 proposal 以此為前提）；`POST /scan` 端構造 ctx 時必帶。
- **Fail-closed 行為**：若 `SanitizerEngine.sanitize` 拋例外（非預期情況——Engine
  已宣告 fail-closed），該檔 **不得** 以任何形式進入 `ScanResult.files`；記入
  `warnings` 與 `stats.quarantined_count`。
- **解鎖 `folder-scanner` 既有約束**：`scanner-skeleton` 寫入的「Deferred
  subsystem schema preservation」requirement 其中 `FileEntry.sanitize_stats` 與
  `FileEntry.content` 的「未 scrub」stub 語意**移除**；其餘 stub（`git=null` /
  `is_monorepo=false` / `sub_packages=[]` / `oversized_preview=null`）不動，留給
  後續 change。
- **移除 LLM call chain 消費禁令**：`folder-scanner` spec 裡「skeleton 輸出不得被
  下游 LLM 路徑消費」這一段 requirement **刪除**（因為前提已消失）。

## Non-Goals

- **不實作** Git metadata 收集（pygit2）— 延後至獨立 change。
- **不實作** Monorepo 偵測 — 延後。
- **不實作** SSE Progress event — 延後至 `scanner-sse-progress` change。
- **不實作** `oversized_preview` 檔頭 200 行 summary — 留給單獨的 oversized 增強 change。
- **不新增 Sanitizer rule** — 本 change 只串 orchestration，不動 rules；若未來
  新增 rule 必須依 `docs/authorization.md §六` bump rules version。
- **不串 binary / lockfile / generated / oversized 類型的 scrub** — 這些 kind 的
  `content` 本就 `None`，沒有原文需要 sanitize。
- **不改 `POST /scan` request/response schema** — `sanitize_stats` 欄位早在
  skeleton 階段就 lock 住，只是從「永遠空」變「真實計數」。

## Capabilities

### New Capabilities

（none — 本 change 不新增 capability）

### Modified Capabilities

- `folder-scanner`: 改寫 `File classification by extension and content sniffing`、`Deferred subsystem schema preservation` 兩條 requirement 的 `sanitize_stats` 與 `content` 語意；新增 `Pass 1 sanitizer orchestration`、`Sanitize audit logging` 兩條 requirement；移除 skeleton 版的「LLM call chain 消費禁令」條款。

## Impact

- **Affected specs**:
  - `openspec/specs/folder-scanner/spec.md` — delta 改 / 加 / 刪（見 Modified Capabilities）
- **Affected code**:
  - `sidecar/src/codebus_agent/scanner/service.py` — 加 Pass 1 呼叫 + sanitize_stats 計算
  - `sidecar/src/codebus_agent/sandbox.py` — `ToolContext` 若尚未帶 `sanitizer` 則補上（視 sanitizer-safety-chain 實際狀態而定）
  - `sidecar/src/codebus_agent/api/scan.py` — 建 `ToolContext` 時注入 `SanitizerEngine` 實例
  - `sidecar/tests/scanner/test_service.py` — 加 Pass 1 串通單測（email / secret / 無命中）
  - `sidecar/tests/scanner/test_scan_api.py` — 加整合測：POST /scan 後 `sanitize_audit.jsonl` 有行、`FileEntry.content` 含 placeholder、`sanitize_stats` 非空
  - `sidecar/tests/scanner/fixtures/mixed-encoding/` 或新 `with-secrets/` fixture — 加含 email / API key 的文字檔驗 Pass 1 串通
- **Affected docs**:
  - `docs/module-1-scanner.md` §一 / §九 / §十一 的 Skeleton 範圍註記刪「Sanitize
    orchestration deferred」那條、把 `sanitize_stats` stub 從「永遠 `{}`」改成
    「真實計數，無命中時 `{}`」
  - `docs/sidecar-api.md` §三 `/scan` 的 Skeleton 註記同步刪去 sanitize-deferred
    描述，response stub 清單去掉 `sanitize_stats`
- **Impl plan 對應**: `docs/implementation-plan.md §二` 第三階段第 13 步的「scanner
  Pass 1 串通」收尾；解鎖第 14 步 KB Builder。
- **不變式不變**：
  - 雙模 discriminator（`workspace_type`）不動
  - LLM 看到的永遠是 sanitized 版本（D-015）
  - Sanitizer 單向替換、無 reverse mapping
  - bearer + loopback 綁定不鬆動
