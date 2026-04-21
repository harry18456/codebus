## 1. Setup & dependencies

- [x] 1.1 以 `uv add pathspec charset-normalizer` 於 `sidecar/pyproject.toml` 新增依賴，並確認 `uv sync` 乾淨通過
- [x] 1.2 建立 `sidecar/src/codebus_agent/scanner/` package 骨架（`__init__.py` + 空 `models.py` / `encoding.py` / `classify.py` / `language.py` / `summary.py` / `walk.py` / `service.py`），讓後續 TDD test 可完成 import

## 2. Data models — `ScanResult` schema 一次到位，Sanitizer / Git / Monorepo 欄位留 stub 預設值

- [x] 2.1 TDD red：寫 `tests/scanner/test_models.py` 覆蓋 Deferred subsystem schema preservation（`FileEntry.sanitize_stats=={}`、`ScanResult.git is None`、`is_monorepo is False`、`monorepo_type is None`、`sub_packages == []`），以及 `FileEntry` / `Symlink` / `ScanStats` / `ContentTypeSummary` 各欄位型別、round-trip `model_dump()`；對齊 design 決策「`ScanResult` schema 一次到位，Sanitizer / Git / Monorepo 欄位留 stub 預設值」
- [x] 2.2 實作 `scanner/models.py`（Pydantic v2 BaseModel，對齊 `docs/module-1-scanner.md §十一`），令 2.1 全綠

## 3. Leaf module TDD red — 各寫測試檔

- [x] 3.1 [P] TDD red：寫 `tests/scanner/test_encoding.py` 覆蓋 Encoding detection fallback chain（utf-8 / utf-16 BOM / big5 / gbk / shift_jis / charset-normalizer 保底 / 全失敗判 binary），對齊 design 決策「`charset-normalizer` 當 fallback chain 保底而非主判」
- [x] 3.2 [P] TDD red：寫 `tests/scanner/test_classify.py` 覆蓋 File classification by extension and content sniffing（binary 副檔名 / null-byte sniff / lockfile 如 `uv.lock` / generated 如 `*.min.js` / oversized / 純 text）
- [x] 3.3 [P] TDD red：寫 `tests/scanner/test_language.py` 覆蓋 Language identification（副檔名主路徑、shebang 副路徑、unknown 回 null + `language_confidence="unknown"`）
- [x] 3.4 [P] TDD red：寫 `tests/scanner/test_summary.py` 覆蓋 Content type summary generation（code dominant / mixed `dominant_category` / `has_tests` 偵測 `tests/` 目錄 / `has_docs` 偵測 README）

## 4. Leaf module implementations — 令 3.x 全綠

- [x] 4.1 [P] 實作 `scanner/encoding.py`（`detect_encoding(bytes) -> (encoding, str | None)`），fallback chain 用顯式 `try` 鏈，最後呼 `charset_normalizer.from_bytes` 保底
- [x] 4.2 [P] 實作 `scanner/classify.py`（`classify(path, size, head_bytes) -> Literal["text","binary","oversized","lockfile","generated"]`）
- [x] 4.3 [P] 實作 `scanner/language.py`（`identify(path, shebang: str | None) -> (language, confidence)`，副檔名表寫為 module-level dict）
- [x] 4.4 [P] 實作 `scanner/summary.py`（`build_summary(files: list[FileEntry]) -> ContentTypeSummary`，category 分類走 language + 路徑 heuristic）

## 5. Traversal with sandbox + gitignore

- [x] 5.1 TDD red：寫 `tests/scanner/test_walk.py` 覆蓋 File tree traversal with gitignore stacking（built-in always-ignore、nested `.gitignore` 疊加、negation `!important.log`）+ Symlink handling without following（in-workspace / out-of-workspace 兩組 case）+ Sandbox boundary enforcement（`..` 逃逸 fixture 掛出來會被 skip 並寫 warning），對齊 design 決策「`pathspec` vs 自寫 gitignore 解析」、「Symlink 預設不跟隨，resolve 後在 workspace 內才記錄」、「Scanner 不是 tool，直接用 sandbox helper」
- [x] 5.2 實作 `scanner/walk.py`：`walk(workspace_root, ctx) -> Iterator[FileEntry | Symlink]`，每個 entry 先 `ensure_in_workspace(path, ctx)`、gitignore 用 `pathspec.PathSpec.from_lines("gitwildmatch", ...)` 階層疊加、遇目錄命中 ignore 即不 rglob 進去、symlink 以 `Path.is_symlink()` 判別不跟隨

## 6. Service orchestrator

- [x] 6.1 TDD red：寫 `tests/scanner/test_service.py` 驅動 walk → classify → encode → language → summary 產出 `ScanResult`，並 assert deferred-stub defaults（`git is None`、`sub_packages == []`、每個 `sanitize_stats == {}`）
- [x] 6.2 實作 `scanner/service.py::scan(workspace_root: str, ctx: ToolContext) -> ScanResult`，組起整條 pipeline

## 7. API endpoint — POST /scan

- [x] 7.1 TDD red：寫 `tests/scanner/test_scan_api.py` 覆蓋 Workspace scan endpoint + Workspace type discriminator routing + Synchronous response without SSE progress events + Workspace scan endpoint registration（來自 `sidecar-runtime` delta spec）——各 case：200 folder 成功、501 topic（對齊 design 決策「`workspace_type="topic"` 回 `501 Not Implemented`（非 `400`）」）、422 未知 discriminator、401 缺 bearer、400 `SCANNER_WORKSPACE_INVALID` 當 `workspace_root` 不存在、`Content-Type: application/json` 單 body（對齊 design 決策「同步 `/scan` endpoint，SSE 留待後續 change」）
- [x] 7.2 實作 `sidecar/src/codebus_agent/api/scan.py`（FastAPI router）並於 `api/__init__.py::create_app` 註冊於 bearer 中介層下；注意 `workspace_type="topic"` 回 `HTTPException(status_code=501, detail="workspace_type='topic' not implemented in MVP")`

## 8. Fixtures & regression

- [x] 8.1 建立 `sidecar/tests/scanner/fixtures/` 下 `mini-py-repo/`（含 `__pycache__/` 該被 ignore、一支 `*_test.py` 驗 has_tests）、`mini-ts-repo/`（含根 `.gitignore` 內含 `node_modules/` + 實際 `node_modules/foo/index.js` 驗 built-in ignore）、`mixed-encoding/`（utf-8 / big5 / null-byte binary sample）、`symlink-cases/`（in-workspace / out-of-workspace 兩支 symlink，僅 POSIX 啟用、Windows 自動 skip）
- [x] 8.2 跑 `uv run pytest` 全量，確認 `test_healthz` / `test_e2e_handshake` / `test_create_app` / `test_main_run` 無 regression、新 `tests/scanner/` 全綠

## 9. Docs sync

- [x] 9.1 在 `docs/module-1-scanner.md` 的 §一 / §三 / §四 / §五 / §六 / §十一 / §十三 對應段落頭加 Skeleton 實作範圍註記（列明 Sanitizer / Git metadata / Monorepo / SSE 仍未實作、對應章節有 stub 欄位 / stub 行為）
- [x] 9.2 更新 `docs/sidecar-api.md` §三 `/scan` 章節（若既有）：補 bearer / 501 topic / 400 `SCANNER_WORKSPACE_INVALID` 行為；若 §三 不存在則新增
