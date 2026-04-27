## Why

Phase 6 共用骨架（`phase6-shell`，2026-04-27 archive）已備好 `useSidecar` / `AuditPanel` / 七 tab 與 design tokens；但 sidecar 上目前**任何敏感操作都還沒有使用者明確同意**——KB build / Explorer / Generator / Q&A 全部裸跑。Trust Layer 敘事的 Act 1 第一幕（O-01 Grant Modal）必須先落地，後續所有 Page 級 change（R-01 / O-04 / O-05）才有「使用者已授權此 workspace 此 provider」可以掛。

`docs/authorization.md`（410 行 spec，2026-04-19 寫好）一直以 design-only 狀態擱在 docs/，沒落到 `openspec/specs/` 也沒有對應 sidecar endpoint。CLAUDE.md 七層 audit 段第七層 `~/.codebus/authorization_audit.jsonl`（**App-level**，跨 workspace）標 📐 design-only，等的就是這個 change。

關聯 ADR：D-002（雙模 discriminator day-1）/ D-008（First-run UX）/ D-011（資安）/ D-015（Sanitizer 三段防線）。對應 `docs/implementation-plan.md §六` 步驟 26.5、`design/v1/03-grant.html` mockup。Discuss 階段（2026-04-27）已釐清 `docs/authorization.md §六` rules_version semver 與程式碼真值（`sanitizer/config.py` 的 date 格式 `YYYY-MM-DD-N`）的 spec drift——本 change design.md 一併校正。

## What Changes

P0 範圍（~4d，對齊 `docs/authorization.md §十一` P0 條目）：

- 新建 `openspec/specs/authorization-audit/` capability（首次將 `docs/authorization.md` 規範化為 capability spec），4 個 ADDED Requirements：
  - `authorization_audit.jsonl` 是 App-level audit log，唯一 writer 是 `AuthorizationAuditLogger`
  - 4 個 sidecar endpoints（`POST /auth/grant` / `POST /auth/deny` / `POST /auth/revoke` / `GET /auth/status`）全 sync，bearer middleware 覆蓋
  - 三事件 schema（`grant_issued` / `grant_denied` / `grant_revoked`）必填欄位 + workspace_type discriminator day-1
  - O-01 Authorization Modal 三情境（`first_run` / `scope_reconfirm` / `scope_upgrade_new_kind`）共用同一 Vue 組件
- 新 sidecar module：`sidecar/src/codebus_agent/auth/`（`audit_logger.py` + `paths.py` + `service.py`）
- 新 sidecar API：`sidecar/src/codebus_agent/api/auth.py`
- 新 path constant leaf：App-level `~/.codebus/authorization_audit.jsonl`（與 workspace-level `_audit_paths.py` 平行的新 leaf module）
- 新 frontend 元件：`web/app/components/auth/AuthorizationModal.vue` + `web/app/composables/useAuthorization.ts` + `web/app/pages/workspace/grant.vue`
- 修改 `web/app/composables/useSidecar.ts`：補 4 個 typed wrapper（`grant` / `deny` / `revoke` / `status`），保留既有 `fetch`
- 修改 `sidecar/src/codebus_agent/api/__init__.py`：app factory 注入 `auth_audit_logger_factory` + 把 auth router include 進 app
- 新 module 級 `auth/errors.py` 定義 auth-specific HTTP error codes（`AUTH_WORKSPACE_INVALID` / `AUTH_NO_ACTIVE_GRANT` / `AUTH_INVALID_REQUEST`）；**不**擴 `tasks.py::ERROR_CODES` frozenset（那是 SSE background task 專用，auth 是 sync HTTP，兩條路徑語意分離）

校正既有 spec drift：

- 修改 `docs/authorization.md §六`：rules_version semver wording 全段重寫為「opaque date format `YYYY-MM-DD-N` + 版本比對邏輯 P1 deferred」
- 修改 `docs/authorization.md §十一`：P0 / P1 條目重切 — `POST /auth/revoke` endpoint 是 P0 但 Settings UI 入口 P1；rules major bump trigger 整段 P1
- 修改 `docs/authorization.md §五`：`scenario` 列舉移除 `rules_version_bump` / `combined_version_and_kind`（P1）保留 `first_run` / `scope_reconfirm` / `scope_upgrade_new_kind`

## Non-Goals

P1（留給後續 change）：

- Settings 頁的 revoke UI 入口（endpoint 是 P0 但 UI 入口非 P0）
- `rules_version_bump` modal 變體 + rules major bump 自動 trigger 邏輯
- `combined_version_and_kind` 合併情境 modal 變體
- `~/.codebus/sanitizer_rules_meta.json` 記錄 last_acked_version 的機制（P0 不需要 — 因為 P0 沒有 MAJOR bump trigger）
- `POST /scan?summary=true` query param（payload 優化 — P0 frontend 忍受完整 ScanResult 並 ignore content）

明確不做（避免 scope 膨脹）：

- Topic mode 授權（D-002 雙模 discriminator schema 預留，但 handler 走 501 同 `POST /scan` 既有 pattern）
- Multi-user / Role-based 授權（單機 local app）
- 授權過期自動 revoke（與 App session lifetime 綁）
- 動態權限升降（Agent 臨時要求更高權限）
- Biometric / hardware token 二次確認
- Provider sanitize allowlist 與 `user_ack` 綁定（`TrackedProvider.ALLOWED_INNER_TYPES` 維持 hardcoded，不依 grant 動態變化 — 避免兩個閘門互相干擾）

範圍判決（拒絕）：

- 拒絕 `auth-flow` 與 R-01 workspace 主畫面合一個 change（O-01 是 modal + 4 endpoint + audit log，R-01 是站牌 + Agent console，跨層級無對稱）
- 拒絕跳過 audit log 直寫 modal（沒第七層 audit 就違反 CLAUDE.md「Audit JSONL 七層」不變式）

## Capabilities

### New Capabilities

- `authorization-audit`: App-level 授權 audit log 與 O-01 Modal flow 不變式集合 — `AuthorizationAuditLogger` 是唯一 writer / 三事件 schema 必填欄位 / 4 sidecar endpoints 全 sync 走 bearer / workspace_type discriminator day-1 / O-01 Modal 三情境共用組件 / scope upgrade 比對讀上一筆 grant_issued。**只規範不變式與 schema，不規範視覺細節**（mockup 以 `design/v1/03-grant.html` 為 source of truth）。

### Modified Capabilities

- `sidecar-runtime`：新 ADDED Requirement `Authorization endpoints registration` — 規範 4 個 sync endpoint 都掛在 bearer middleware 之下、route prefix `/auth/`、HTTP error 用獨立 `auth/errors.py` 三常數而非擴張 SSE 專用 `ERROR_CODES` frozenset。
- `frontend-shell`：`Sidecar bearer and base URL come from Tauri IPC` Requirement 加 Scenario `useSidecar exposes typed auth wrappers`，明文列出 4 個 auth method（`grant` / `deny` / `revoke` / `status`）型別簽名。

## Impact

- Affected specs: 1 NEW（`authorization-audit`，4 ADDED Requirements）+ 2 MODIFIED（`sidecar-runtime` 1 條 / `frontend-shell` 1 條）
- Affected code:
  - New:
    - sidecar/src/codebus_agent/auth/__init__.py
    - sidecar/src/codebus_agent/auth/paths.py
    - sidecar/src/codebus_agent/auth/audit_logger.py
    - sidecar/src/codebus_agent/auth/service.py
    - sidecar/src/codebus_agent/auth/errors.py
    - sidecar/src/codebus_agent/api/auth.py
    - sidecar/tests/auth/test_paths.py
    - sidecar/tests/auth/test_audit_logger.py
    - sidecar/tests/auth/test_service.py
    - sidecar/tests/api/test_auth_endpoints.py
    - web/app/components/auth/AuthorizationModal.vue
    - web/app/composables/useAuthorization.ts
    - web/app/pages/workspace/grant.vue
  - Modified:
    - sidecar/src/codebus_agent/api/__init__.py
    - web/app/composables/useSidecar.ts
    - openspec/specs/sidecar-runtime/spec.md（透過 spectra archive 自動套）
    - openspec/specs/frontend-shell/spec.md（透過 spectra archive 自動套）
    - docs/authorization.md（§五 / §六 / §十一 校正）
    - docs/sidecar-api.md（§三補 4 個 auth endpoint schema）
    - CLAUDE.md（七層 audit 段第七層從 📐 改 ✅；archive 時間軸 + Phase 6 動工順序新增 row）
    - docs/implementation-plan.md（§六 步驟 26.5 改完成）
  - Removed: 無
- Affected docs:
  - CLAUDE.md（子系統段 sidecar / web 各補 auth module 描述、archive 時間軸新行、第七層 audit 從 design-only 改實作完成）
  - docs/authorization.md（§五 schema 列舉 / §六 rules_version wording / §十一 P0-P1 切分 三處校正）
  - docs/sidecar-api.md（§三新增 POST /auth/grant / POST /auth/deny / POST /auth/revoke / GET /auth/status schema）
  - docs/implementation-plan.md（§六步驟 26.5 從待動工改完成）
- Test suite delta：sidecar baseline 853 / 19 → 預期 ~885 passed / 19 skipped（新增約 30 條 unit + 4 條 integration）；前端維持 typecheck + dev HTTP 200 驗收（test framework 仍 Phase B 才裝）
