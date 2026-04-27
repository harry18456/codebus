## 1. 前置驗證

- [x] 1.1 確認 baseline：`cd sidecar && uv run pytest tests/ -q` ~853 passed / 19 skipped；`cd web && npm run typecheck` 全綠
- [x] 1.2 重讀 `docs/authorization.md` §一-§十一 + `design/v1/03-grant.html` mockup + `phase6-shell` archive 中 `useSidecar.ts` / `useSseTask.ts` 兩 composable

## 2. docs/authorization.md spec drift 校正（先做，後續實作對齊它）

- [x] 2.1 改 §五 `scenario` 列舉：移除 `rules_version_bump` 與 `combined_version_and_kind`（P1 deferred），保留三個 P0 值
- [x] 2.2 改 §六 整段重寫：把「semver `vMAJOR.MINOR.PATCH`」+ MAJOR/MINOR/PATCH 觸發策略段全砍掉，改寫為「`rules_version` 為 opaque 字串、格式由 `sanitizer.RULES_VERSION` 決定（目前 `YYYY-MM-DD-N`）、版本比對與升級觸發邏輯整段 P1 deferred」
- [x] 2.3 改 §十一 P0 / P1 切分：`POST /auth/revoke` endpoint 屬 P0、Settings UI revoke 入口屬 P1；rules major bump trigger 整段 P1；scope 比對邏輯細節對齊本 change spec
- [x] 2.4 §八 endpoint placeholder 段標 ✅ 已落實，連結到 `docs/sidecar-api.md §三`（補完 endpoint schema 在 task 9.x）

## 3. App-level audit path leaf module

- [x] 3.1 [P] 新建 `sidecar/src/codebus_agent/auth/__init__.py` 空檔（確保 `auth` 為 Python package）
- [x] 3.2 新建 `sidecar/src/codebus_agent/auth/paths.py`：定義 `_APP_AUDIT_HOME_SUBDIR = ".codebus"` + `_AUTHORIZATION_AUDIT_FILENAME = "authorization_audit.jsonl"` + `authorization_audit_path() -> Path` helper（用 `Path.home()` 解析）
- [x] 3.3 [P] 新建 `sidecar/tests/auth/__init__.py` + `sidecar/tests/auth/test_paths.py`：兩條 defensive test — (a) `authorization_audit_path()` 回 `<home>/.codebus/authorization_audit.jsonl`；(b) 整 `sidecar/src/codebus_agent/` source-grep `authorization_audit\.jsonl` 字面量必須 0 命中（除 `auth/paths.py` 自身）
- [x] 3.4 跑 `uv run pytest sidecar/tests/auth/test_paths.py -q` 必綠

## 4. AuthorizationAuditLogger（第七層 audit log 唯一 writer）

- [x] 4.1 [P] 新建 `sidecar/src/codebus_agent/auth/audit_logger.py`：
    - `AuthorizationAuditLogger.__init__(path: Path)` 必須驗證 `path.is_absolute()` 否則 raise `ValueError`；`mkdir(parents=True, exist_ok=True)` parent
    - 三個 method `write_grant_issued(...)` / `write_grant_denied(...)` / `write_grant_revoked(...)` 各有 typed kwargs；每筆寫入恰一行 `\n` 結尾
    - 內部用 `with path.open("a", encoding="utf-8") as f:` append；不再 hold file handle
    - 模組頂註解寫「mirror `KBGrowthLogger` pattern；直接 `open()` against this path 屬 invariant violation」
- [x] 4.2 新 `sidecar/tests/auth/test_audit_logger.py`：
    - `test_constructor_rejects_relative_path`：傳 `Path("rel.jsonl")` → raise `ValueError`
    - `test_constructor_creates_parent_dir`：parent 不存在時 mkdir
    - `test_write_grant_issued_appends_one_line`：每次 write 增加一行 valid JSON
    - `test_write_grant_denied_minimal_required_fields`：reason="user_cancelled" 寫入後 line 含 ts/event/session_id/workspace_type/workspace_source/scenario/reason
    - `test_write_grant_revoked_required_fields`：trigger="settings_revoke" 寫入後 line 含 ts/event/session_id/workspace_id/grant_ts/trigger
    - `test_three_methods_write_distinct_event_kinds`：三 method 各 call 一次 → 檔內三行各自帶不同 `event` 值

## 5. workspace_id derivation + session_id generation

- [x] 5.1 新建 `sidecar/src/codebus_agent/auth/service.py`：
    - `def workspace_id_for_path(path: Path) -> str:` SHA-256(canonical lowercase POSIX path)[:12] prefix `"ws_"`
    - `def fresh_session_id() -> str:` `str(uuid.uuid4())`
    - 模組頂寫 docstring 連到 `authorization-audit` capability §workspace_id requirement
- [x] 5.2 加進 `sidecar/tests/auth/test_service.py`（先放 stable id 與 uuid 測；service 邏輯 task 7.x 才補）：
    - `test_workspace_id_stable_across_calls`：同 path 兩次 call → 結果相等
    - `test_workspace_id_case_insensitive_on_windows`：用 `Path("/foo")` vs `Path("/FOO")` 在 Windows 應該等價（用 monkeypatch + `os.path.normcase`-friendly 實作）
    - `test_workspace_id_format_15_chars_starts_with_ws`：結果為 `ws_` + 12 hex chars
    - `test_fresh_session_id_is_uuid4`：`uuid.UUID(value, version=4)` 不 raise
    - `test_two_session_ids_differ`：連續兩次 call 結果不同

## 6. Auth HTTP error code constants（disjoint from SSE ERROR_CODES）

- [x] 6.1 新建 `sidecar/src/codebus_agent/auth/errors.py`：定義 4 個 module-level string 常數 `AUTH_WORKSPACE_INVALID = "AUTH_WORKSPACE_INVALID"` / `AUTH_NO_ACTIVE_GRANT = "AUTH_NO_ACTIVE_GRANT"` / `AUTH_INVALID_REQUEST = "AUTH_INVALID_REQUEST"` / `AUTH_NOT_CONFIGURED = "AUTH_NOT_CONFIGURED"`；檔頂註解寫「sync HTTP only；分離自 `tasks.py::ERROR_CODES`（SSE wire-error 專用），詳見 design D-A11」
- [x] 6.2 加 `sidecar/tests/auth/test_error_codes_disjoint.py`：import 四 auth 常數 + `api.tasks.ERROR_CODES`，assert intersection 為 empty
- [x] 6.3 改 `sidecar/src/codebus_agent/api/tasks.py::ERROR_CODES` 上方 docstring 註解：補一行「Auth-specific HTTP error codes live in `codebus_agent/auth/errors.py` and are intentionally disjoint from this SSE-channel frozenset」

## 7. Pydantic schemas + auth service logic（無 endpoint 之前）

- [x] 7.1 在 `sidecar/src/codebus_agent/auth/service.py` 加 Pydantic models：
    - `WorkspaceSourceFolder(path: str)` / `WorkspaceSourceTopic(query: str, seed_urls: list[str], domain_allowlist: list[str])`（topic 為 Phase 2 schema slot）
    - `GrantRequest`（含 `workspace_type: Literal["folder", "topic"]` / `workspace_source` discriminated union / `scenario: Literal[...]` / `scope: Scope` / `sanitizer_rules_version: str` / `user_ack: list[str]`）
    - `Scope(llm_provider: str, llm_model: str, outbound_endpoint: str)`
    - `GrantResponse(session_id: str, workspace_id: str, granted_at: str)`
    - `DenyRequest`（含 `workspace_type` / `workspace_source` / `scenario` / `reason: Literal["user_cancelled", "app_closed"]`）
    - `RevokeRequest(session_id: str, trigger: Literal["settings_revoke"])`
    - `AuthStatusResponse(has_active_grant: bool, session_id: str | None, last_grant: dict | None, current_rules_version: str)`
- [x] 7.2 加 `find_last_grant_for_workspace(workspace_id: str, audit_path: Path) -> dict | None`：line-by-line `json.loads`，filter `event == "grant_issued"` + `workspace_id` 相等，回最後一筆（None if not found）
- [x] 7.3 加 `extract_acked_kinds(grant_entry: dict) -> set[str]`：從 `user_ack` 取 `new_kind:*` 後 strip prefix 回 set；非 grant entry 回空 set
- [x] 7.4 加 `validate_scenario_invariants(req: GrantRequest, last_grant: dict | None) -> None`：
    - `first_run` + `last_grant` 存在 → raise ValueError
    - `scope_upgrade_new_kind` + `last_grant` 不存在 → raise ValueError
    - `scope_upgrade_new_kind` + new_kinds_in_request `⊆` acked_kinds → raise ValueError（必須有新 kind diff）
    - `scope_reconfirm` + new_kinds_in_request `⊄` acked_kinds → raise ValueError（不可引入新 kind）
- [x] 7.5 在 `sidecar/tests/auth/test_service.py` 補：
    - `test_find_last_grant_returns_none_for_empty_log`
    - `test_find_last_grant_returns_latest_match`：寫三筆 grant_issued（兩筆 ws_a、一筆 ws_b），對 ws_a 查回最新一筆
    - `test_extract_acked_kinds_strips_prefix`：`user_ack=["raw_stays_local", "new_kind:secret", "new_kind:email"]` → `{"secret", "email"}`
    - 4 個 `validate_scenario_invariants` 各情境（first_run with prior / scope_upgrade without prior / scope_upgrade no diff / scope_reconfirm with new kind）→ 各 raise

## 8. POST /auth/* endpoints + app factory wiring

- [x] 8.1 新建 `sidecar/src/codebus_agent/api/auth.py`：
    - `router = APIRouter(prefix="/auth")`
    - 4 個 handler（`grant` / `deny` / `revoke` / `status`），dependency `request: Request` 取 `request.app.state.auth_audit_logger_factory`；`None` → 503 `AUTH_NOT_CONFIGURED`
    - in-memory `_session_dict: dict[str, GrantSession]` 模組私有
    - `POST /auth/grant`: 驗 workspace_root（`Path.is_dir()`）→ 不通過 raise `HTTPException(400, detail={"code": AUTH_WORKSPACE_INVALID, ...})`；call `validate_scenario_invariants`；不通過 raise 400 `AUTH_INVALID_REQUEST`；通過則 `fresh_session_id()` + `workspace_id_for_path()` + audit logger `write_grant_issued`；存 session 進 `_session_dict`；回 200 GrantResponse
    - `POST /auth/deny`: 直接 `write_grant_denied`；回 204
    - `POST /auth/revoke`: 從 `_session_dict` lookup session_id；找不到 raise 404 `AUTH_NO_ACTIVE_GRANT`；找到則 audit log scan 取對應 grant_ts；`write_grant_revoked`；`del _session_dict[session_id]`；回 204
    - `GET /auth/status`: query param `workspace_id`；scan audit log 取最新 `grant_issued`；回 AuthStatusResponse
    - `workspace_type="topic"` 在三個 POST handler 內走 `HTTPException(501, detail={"code": AUTH_INVALID_REQUEST, "message": "topic mode reserved for Phase 2"})`
- [x] 8.2 改 `sidecar/src/codebus_agent/api/__init__.py::create_app`：
    - 加 keyword-only 參數 `auth_audit_logger_factory: Callable[[], AuthorizationAuditLogger] | None = None`
    - `app.state.auth_audit_logger_factory = auth_audit_logger_factory`
    - `app.include_router(auth.router)`（在 bearer middleware 已掛之後）
- [x] 8.3 改 `sidecar/src/codebus_agent/api/main.py`：startup path 建 default factory `lambda: AuthorizationAuditLogger(authorization_audit_path())` 傳給 `create_app`
- [x] 8.4 新 `sidecar/tests/api/test_auth_endpoints.py`：
    - `test_endpoints_reject_missing_bearer`：4 endpoint 不帶 bearer → 401
    - `test_grant_factory_none_returns_503`：create_app(auth_audit_logger_factory=None) → POST /auth/grant 回 503 AUTH_NOT_CONFIGURED
    - `test_grant_workspace_invalid_returns_400`：path 是 file 而非 dir → 400 AUTH_WORKSPACE_INVALID + 0 audit lines written
    - `test_grant_first_run_success_writes_audit_returns_200`
    - `test_grant_first_run_with_prior_returns_400`：先 grant 一次，再用 first_run → 400
    - `test_deny_writes_audit_returns_204`：1 audit line + 0 sessions
    - `test_revoke_unknown_session_returns_404`：直接 revoke 一個不存在 session_id → 404 AUTH_NO_ACTIVE_GRANT
    - `test_revoke_active_session_writes_grant_revoked_and_clears_session`：grant → revoke 同 session_id → 兩 audit lines + session 移除
    - `test_status_no_grants_returns_has_active_grant_false`
    - `test_status_after_grant_returns_last_grant_payload`
    - `test_topic_workspace_type_returns_501`：`workspace_type="topic"` → 501 AUTH_INVALID_REQUEST

## 9. docs/sidecar-api.md endpoint schema 補完

- [x] 9.1 在 `docs/sidecar-api.md §三` 補 `POST /auth/grant` schema 區塊：request body Pydantic model + response 200/400/503 範例 + ERROR_CODES 列舉指向 `auth/errors.py`
- [x] 9.2 補 `POST /auth/deny` schema：request + 204 response
- [x] 9.3 補 `POST /auth/revoke` schema：request + 204 + 404 / 503
- [x] 9.4 補 `GET /auth/status` schema：query param + 200 AuthStatusResponse + 503
- [x] 9.5 §三-bis ERROR_CODES 表格：在現有 10 codes 表下加一個 sub-section「Auth HTTP error codes (NOT in ERROR_CODES frozenset)」列 4 codes + 描述

## 10. 前端 useSidecar 加 4 個 typed wrapper

- [x] 10.1 改 `web/app/composables/useSidecar.ts`：新加 TypeScript types `GrantRequest` / `GrantResponse` / `DenyRequest` / `RevokeRequest` / `AuthStatusResponse`（與 sidecar Pydantic 對齊）
- [x] 10.2 加 4 個 method 到 `useSidecar()` 回傳：`grant(req)` → `POST /auth/grant`；`deny(req)` → `POST /auth/deny`；`revoke(req)` → `POST /auth/revoke`；`status(workspaceId)` → `GET /auth/status?workspace_id=...`；都走既有 `sidecarFetch`（自動帶 Authorization）
- [x] 10.3 update `SidecarApi` interface 從 4 欄擴 8 欄
- [x] 10.4 跑 `cd web && npm run typecheck` 全綠

## 11. AuthorizationModal.vue + useAuthorization composable

- [x] 11.1 [P] 新 `web/app/composables/useAuthorization.ts`：暴露 `useAuthorization()` 回 `{ scenario, ackFlags, submitEnabled, setAck, reset }`，纯 modal flow state，不 IPC
- [x] 11.2 [P] 新 `web/app/components/auth/AuthorizationModal.vue`：
    - props 用 `defineProps<AuthorizationModalProps>()` 嚴格 typed（`activeScenario` 三 literal）
    - 三段 grid 版面對齊 `design/v1/03-grant.html`：scope 摘要 / sanitizer 類別預告（4 色 chip）/ hero line / provider 行 / 三條 ack checkbox（+ scope_upgrade_new_kind 時 per-kind 多 checkbox）/ footer（cancel / submit）
    - 全部用 design token utility class（design tokens originate from a single source 不變式）
    - submit 按鈕 disabled 條件：所有 base ack + new_kind ack 全 ticked
    - cancel button click → call `useSidecar().deny(...)` 一次 → emit `denied` Vue event；不自己 navigate
    - submit button click → call `useSidecar().grant(...)` → resolve 後 emit `granted` Vue event
- [x] 11.3 grep enforce：`web/app/components/auth/` 內無 `bg-slate-/bg-indigo-/bg-zinc-/text-slate-/text-indigo-/text-zinc-`、無 hex 字面量、無 `localStorage`/`sessionStorage`/`document.cookie`

## 12. pages/workspace/grant.vue route

- [x] 12.1 新 `web/app/pages/workspace/grant.vue`：以 default layout 包；route mounted 時先 `POST /scan` 拿 ScanResult、聚合 `files[*].sanitize_stats` 為 `sanitizeKindCounts`；再 call `useSidecar().status(workspaceId)` 拿 last_grant；依 last_grant 決定 `activeScenario`（無 last_grant → first_run / 有 last_grant 但無新 kind → scope_reconfirm / 有 new kind → scope_upgrade_new_kind）；render `<AuthorizationModal />` 帶上述 props
- [x] 12.2 listen modal 的 `denied` event → router.push 回 home（`/`）；listen `granted` event → router.push 到後續 R-01 路徑（暫用 placeholder `/workspace/scan`）
- [x] 12.3 跑 `cd web && npm run dev` + `curl localhost:3000/workspace/grant` → 200（route 註冊成功；UI 是否正確 manual visual 對照）

## 13. 整合驗收

- [x] 13.1 `cd sidecar && uv run pytest tests/ -q` ~885 passed / 19 skipped（baseline 853 + 約 32 條新測）
- [x] 13.2 `cd web && npm run typecheck` 全綠
- [x] 13.3 `cd web && npm run dev` HTTP 200，`/workspace/grant` 路由可達
- [x] 13.4 grep enforce 三件：
    - `rg "authorization_audit\.jsonl" sidecar/src/codebus_agent/` → 1 命中（`auth/paths.py`）
    - `rg "useSidecar\(\)\.fetch\(\s*['\"]/auth/" web/app/` → 0 命中
    - `rg "AUTH_GRANT_FAILED|AUTH_RULES_VERSION_MISMATCH" sidecar/src/codebus_agent/` → 0 命中（P0 不引入這兩 code）
- [x] 13.5 manual smoke：開瀏覽器 → 走 first_run flow → 驗 `~/.codebus/authorization_audit.jsonl` 真有一行 `grant_issued`；點 cancel → 驗有 `grant_denied`；用 settings 介面或直接打 endpoint revoke → 驗有 `grant_revoked`

## 14. 文件連動

- [x] 14.1 改 `CLAUDE.md` 七層 audit 段第七層：從 📐 改 ✅ 已實作；補一行「`auth-flow` change（2026-MM-DD archive）落地，writer 是 `AuthorizationAuditLogger`，path constant 在 `auth/paths.py`」
- [x] 14.2 改 `CLAUDE.md` 子系統段 sidecar：補 `codebus_agent/auth/` 子套件描述（`paths.py` / `audit_logger.py` / `errors.py` / `service.py` / `api/auth.py`）
- [x] 14.3 改 `CLAUDE.md` 子系統段 web：補 `app/components/auth/` + `app/composables/useAuthorization.ts` + `app/pages/workspace/grant.vue`
- [x] 14.4 改 `CLAUDE.md` archive 時間軸：新增 row（同 2026-04-27 phase6-shell pattern）
- [x] 14.5 改 `CLAUDE.md` Phase 6 動工順序：步驟 26.5 row 從待動工改完成
- [x] 14.6 改 `docs/implementation-plan.md §六` 步驟 26.5：標已完成 + 對應 spec `authorization-audit`

## 15. 規格覆蓋錨點（apply 階段純驗證 checkbox）

- [x] 15.1 Spec coverage：`AuthorizationAuditLogger is the sole writer for the App-level audit log` 由 task 3.x + 4.x + 13.4 grep 滿足
- [x] 15.2 Spec coverage：`Three-event audit schema with workspace_type discriminator` 由 task 4.2 + 5.2 + 7.5 + 8.4 滿足
- [x] 15.3 Spec coverage：`Four sync sidecar endpoints under bearer middleware` 由 task 8.x + 9.x 滿足
- [x] 15.4 Spec coverage：`O-01 Authorization Modal supports three P0 scenarios with a shared Vue component` 由 task 11.x + 12.x 滿足
- [x] 15.5 Spec coverage：`scope upgrade detection reads the latest grant from audit log` 由 task 7.2-7.4 + 8.4 滿足
- [x] 15.6 Spec coverage：`Authorization endpoints registration` (sidecar-runtime) 由 task 8.x 滿足
- [x] 15.7 Spec coverage：`useSidecar exposes typed auth wrappers` (frontend-shell `Sidecar bearer and base URL come from Tauri IPC` Requirement 內 Scenario) 由 task 10.x 滿足

## 16. Design / Risks 交叉索引（apply 期不執行；對齊 design.md）

| design.md 條目 | 對應 task |
|---|---|
| D-A1：App-level audit log 走獨立 leaf module | 3.1, 3.2, 3.3 |
| D-A2：`AuthorizationAuditLogger` 是唯一 writer，三事件 method | 4.1, 4.2 |
| D-A3：`rules_version` P0 verbatim 記錄、版本比對邏輯整段 P1 | 2.2, 7.1, 7.4 |
| D-A4：`workspace_id` 由 sidecar 從 path 雜湊產生 | 5.1, 5.2 |
| D-A5：`session_id` 由 sidecar 在 grant_issued 時生成 | 5.1, 5.2 |
| D-A6：scope upgrade 比對讀 audit log，不做 in-memory cache | 7.2, 7.4, 8.1 |
| D-A7：sanitizer dry-run 走既有 `POST /scan`，前端聚合 sanitize_stats | 12.1 |
| D-A8：`POST /auth/grant` 在收到請求時做 workspace_root 驗證 | 8.1, 8.4 |
| D-A9：`POST /auth/deny` / `POST /auth/revoke` 純記錄、不影響 sidecar 狀態 | 8.1 |
| D-A10：`useSidecar` typed wrapper 加在既有 composable，不新建 | 10.1, 10.2, 10.3 |
| D-A11：Auth HTTP error codes 走獨立 module，不污染 SSE `ERROR_CODES` | 6.1, 6.2, 6.3 |
| R-1：rules_version 格式現狀有遺技債 | 2.2（緩解 = 校正 docs §六 wording） |
| R-2：P0 三情境模糊地帶——`scope_reconfirm` vs `scope_upgrade_new_kind` 邊界 | 7.4, 11.2 |
| R-3：session_id in-memory 重啟即失，可能造成使用者困惑 | 8.1（GET /auth/status 回 has_active_grant + last_grant 兩欄） |
| R-4：workspace_id 雜湊在 path rename 後失效 | 5.1（不解決，文檔註明 path-derived stable id） |
| R-5：auth-specific error code 散落兩處的維護負擔 | 6.2, 6.3（檔頂註解 + defensive test） |
