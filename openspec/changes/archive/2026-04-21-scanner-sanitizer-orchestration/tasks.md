## 1. ToolContext 擴充 Sanitizer 欄位

- [x] 1.1 檢查 `sidecar/src/codebus_agent/sandbox.py` 的 `ToolContext`：若 `sanitizer-safety-chain` archive 尚未加上 `sanitizer: SanitizerEngine` 欄位則補上；確認現有測試構造 `ToolContext` 時能以 default / fixture 注入 engine
- [x] 1.2 更新 `POST /scan` handler（`sidecar/src/codebus_agent/api/scan.py`）：構造 `ToolContext` 時必注入 `SanitizerEngine` 實例（從 app state 或每請求 new；以既有 sandbox 慣例為準）
- [x] 1.3 執行 `uv run pytest tests/sandbox/ tests/api/ -q`，確認 ToolContext 擴充未破壞既有授權 / 紅隊測試

## 2. Pass 1 Sanitizer orchestration 單元測試（TDD 紅燈）

- [x] 2.1 [P] `sidecar/tests/scanner/test_service.py` 新增案例：text 檔無 sanitizer 命中 → `FileEntry.content` 等於原 decode 字串、`FileEntry.sanitize_stats == {}`、該檔未產生 `sanitize_audit.jsonl` 新行（對應 **Pass 1 sanitizer orchestration for text FileEntries**）
- [x] 2.2 [P] `test_service.py` 新增案例：text 檔含一則 email → `content` 含 `<REDACTED:email#N>` placeholder、`sanitize_stats == {"email": 1}`、`sanitize_audit.jsonl` 多 1 行且 `source.pass == "scanner"` / `source.path` 為相對路徑（對應 **Pass 1 sanitizer orchestration for text FileEntries** + **Sanitize audit logging during scan**）
- [x] 2.3 [P] `test_service.py` 新增案例：binary / lockfile / generated / oversized kind 不觸發 sanitizer → `sanitize_stats == {}`、audit log 零新行（對應 **File classification by extension and content sniffing**）
- [x] 2.4 [P] `test_service.py` 新增案例（mock engine raise）：某檔 sanitize 拋例外 → 該檔**不**出現在 `ScanResult.files`、`ScanResult.warnings` 含該相對路徑、`ScanResult.stats.quarantined_count` 至少 +1、HTTP 仍 200（對應 **Sanitize audit logging during scan** fail-closed 條款）

## 3. scanner/service.py 串 Pass 1（TDD 綠燈）

- [x] 3.1 在 `scanner/service.py` decode 成功後、append `FileEntry` 到 `files` 前，對 `kind == "text"` 檔呼 `ctx.sanitizer.sanitize(content, FileSource(pass_="scanner", path=rel_path.as_posix()))`
- [x] 3.2 把 `SanitizedResult.audit_entries` 依 `AuditEntry.kind` 聚合成 `dict[str, int]` 寫入 `FileEntry.sanitize_stats`；無命中時保持 `{}`
- [x] 3.3 `FileEntry.content` 改存 `SanitizedResult.text`（Pass 1 sanitized 版）而非 raw decoded body
- [x] 3.4 透過既有 `SanitizeAuditLogger` 逐檔寫入 `sanitize_audit.jsonl`（每檔呼完 flush，不跨檔 batch），確認不影響其他 audit writer
- [x] 3.5 包 `try/except Exception`：engine 拋錯 → 跳過該檔（不進 `files`）+ `warnings.append(...)` + `stats.quarantined_count += 1`；不讓例外逸出 `POST /scan`
- [x] 3.6 移除 `service.py` 裡「quarantined_count 恆 0」註解與寫死常數；讓它由實際計數驅動

## 4. Fixture 與整合測試

- [x] 4.1 [P] 新增 `sidecar/tests/scanner/fixtures/with-secrets/`：包含含假 email（如 `test@example.com`）與假 API key pattern 的 `.txt` / `.py`、一個乾淨 `README.md`、一個空 `.gitignore`；必要時用 `git add -f` 繞過 repo 根 `.gitignore`
- [x] 4.2 [P] `sidecar/tests/scanner/test_scan_api.py` 加整合測：POST /scan with-secrets fixture → 回傳 `FileEntry.content` 含 placeholder、`sanitize_stats` 至少有一個非零 kind、實際 `sanitize_audit.jsonl` 落盤且含 `source.pass == "scanner"`

## 5. 回歸驗證與不變式守門

- [x] 5.1 執行 `uv run pytest tests/scanner/ -q`：scanner 全測綠，特別是 **Deferred subsystem schema preservation** 剩餘 stub（git / monorepo / oversized_preview）行為不變
- [x] 5.2 執行 `uv run pytest -q`：全測綠；LLM / provider 測試不得因 ToolContext 變更破綻
- [x] 5.3 執行 `pre-commit run --all-files` 綠

## 6. 文件 / impl plan 同步

- [x] 6.1 [P] `docs/module-1-scanner.md` §一 / §九 / §十一：刪「Sanitize orchestration deferred」條；`sanitize_stats` stub 從「永遠 `{}`」改成「真實 kind→count，無命中時 `{}`」
- [x] 6.2 [P] `docs/sidecar-api.md` §三 `/scan`：刪 Skeleton 註記中 sanitize-deferred 段；response stub 清單去掉 `sanitize_stats`
- [x] 6.3 [P] `docs/implementation-plan.md §二`：標第三階段第 13 步「scanner Pass 1 串通」收尾、第 14 步 KB Builder P0 解鎖
