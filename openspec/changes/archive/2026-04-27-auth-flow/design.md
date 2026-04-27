## Context

Trust Layer Act 1 第一幕 O-01 Authorization Modal 的實作落地。`docs/authorization.md` 410 行 spec 在 2026-04-19 寫好後一直未實作，程式碼端只有 sanitizer / scanner / KB / explorer / generator / qa 通電，授權層**完全裸跑**。`phase6-shell` 已備好 `useSidecar` / `AuditPanel` 共用骨架，本 change 是第一個吃這套骨架的 Page 級 change。

第七層 audit log（`~/.codebus/authorization_audit.jsonl`）是 App-level（跨 workspace），與既有六層 workspace-level audit log（`<ws>/.codebus/*.jsonl`）路徑、語意、生命週期都不同，需要獨立 path constant 機制與獨立 logger 類別。

Discuss 階段（2026-04-27）發現 `docs/authorization.md §六` 寫的 `rules_version: vMAJOR.MINOR.PATCH` 與程式碼真值（`sanitizer/config.py:26 RULES_VERSION = "2026-04-20-1"` date format `YYYY-MM-DD-N`）矛盾——本 change 一併校正 docs/authorization.md。

## Goals / Non-Goals

**Goals:**

- 4 sidecar endpoints（grant / deny / revoke / status）全 sync、走既有 bearer middleware、回 OpenAPI-friendly Pydantic schema
- `AuthorizationAuditLogger` 是第七層 audit log 的唯一 writer（mirror `KBGrowthLogger` pattern）
- `workspace_type` 自 day-1 寫進 schema（D-002）；MVP 只支援 `"folder"`，`"topic"` Pydantic Literal 接受但 service handler 走 501
- 三情境 modal `first_run` / `scope_reconfirm` / `scope_upgrade_new_kind` 共用同一 Vue 組件（props-driven）
- 第七層 audit log 從 📐 design-only 翻 ✅ 實作完成，CLAUDE.md 七層 audit 段同步更新
- 校正 `docs/authorization.md §五 / §六 / §十一` 的 spec drift（rules_version 格式 / scenario 列舉 / P0-P1 切分）

**Non-Goals:**

- rules_version MAJOR/MINOR/PATCH 比對邏輯（P1，且需先決定版本格式是否從 date 改 semver）
- Settings 頁的 revoke UI 入口（P1，endpoint 在 P0）
- `combined_version_and_kind` 合併 modal 變體（P1）
- Provider sanitize allowlist 與 user_ack 動態綁定（避免雙閘門互相干擾）
- Topic mode 全鏈（schema 預留，handler 501）

## Decisions

### D-A1：App-level audit log 走獨立 leaf module

**選項：**
- (a) 把 App-level filename 加進既有 `_audit_paths.py`
- (b) 新建 `sidecar/src/codebus_agent/auth/paths.py` 平行 leaf module ✓
- (c) 不抽常數，直接寫死字面量

**選 (b)。** `_audit_paths.py` 第 37-39 行註解明寫「App-level `authorization_audit.jsonl` lives in a future capability and is intentionally NOT listed here」——預留位置就是給本 change 用獨立 leaf。語意上 workspace-level 與 App-level 兩組 path 生命週期不同（前者隨 workspace 切換，後者跨 workspace 持續），混進同一個 leaf 會讓「七層 audit JSONL」段的 invariants 難以表達。

`auth/paths.py` 暴露常數：

```python
_APP_AUDIT_HOME_SUBDIR = ".codebus"  # 注意：home 下的 .codebus，不是 workspace 下
_AUTHORIZATION_AUDIT_FILENAME = "authorization_audit.jsonl"

def authorization_audit_path() -> Path:
    return Path.home() / _APP_AUDIT_HOME_SUBDIR / _AUTHORIZATION_AUDIT_FILENAME
```

新 defensive test 鎖死：source-grep `authorization_audit\.jsonl` 字面量整 `sidecar/src/codebus_agent/` 只能命中 `auth/paths.py`。

### D-A2：`AuthorizationAuditLogger` 是唯一 writer，三事件 method

**選項：**
- (a) 單一 `write(event_type, payload)` 動態 dispatch
- (b) 三個 method `write_grant_issued(...)` / `write_grant_denied(...)` / `write_grant_revoked(...)` ✓
- (c) 直接讓 service.py 自己 append JSONL

**選 (b)。** 三事件必填欄位完全不同（`grant_issued` 帶 `scope` / `user_ack` / `sanitizer_rules_version`，`grant_denied` 帶 `reason`，`grant_revoked` 帶 `grant_ts` / `trigger`），用 typed method + Pydantic 驗證比動態 dispatch 安全。Mirror `KBGrowthLogger` 的單一 method（因為 kb_growth 只有一種事件 = `add`）pattern，但 auth 三事件三 method。

`AuthorizationAuditLogger.__init__(path: Path)` auto-mkdir parent；fail-loud（`path` 非絕對路徑直接 raise）。

### D-A3：`rules_version` P0 verbatim 記錄、版本比對邏輯整段 P1

`docs/authorization.md §六` 的 semver `vMAJOR.MINOR.PATCH` 格式與程式碼 `RULES_VERSION = "2026-04-20-1"` 矛盾。**選擇校正方向：保持程式碼端 date format，spec 端 wording 改為 opaque 字串。**

理由：
- 程式碼已經跑了 18+ 個 archive；改格式要動 sanitizer / audit 五處 callsite + RULES_VERSION constant identity test，scope 太大
- date format `YYYY-MM-DD-N` 自然遞增，不會發生「v1.2.0 < v1.10.0 但字串排序相反」這種 semver 字串比較雷
- P0 不需 MAJOR bump trigger（modal 三情境裡 (c) 變體屬 P1），所以「比對」這件事 P0 完全不做，rules_version 在 grant_issued 裡只是 verbatim 記錄

P0 行為：
- `grant_issued.sanitizer_rules_version` = `from codebus_agent.sanitizer import RULES_VERSION`
- 不做版本比對 / 不寫 meta.json / 不檢查 last_acked_version

P1 follow-up（後續 change）：
- 決定要不要從 date 改 semver
- 加 `last_acked_version` 機制（讀 audit log 取最後一筆 grant_issued 的 rules_version）
- 加 (c) modal 變體 + Settings revoke 入口

### D-A4：`workspace_id` 由 sidecar 從 path 雜湊產生

**選項：**
- (a) sidecar SHA-256 截 12 字（如 `ws_a3f2b1c8`）✓
- (b) UUID4
- (c) 前端傳

**選 (a)。** 同 path 雜湊一致 → 多次 grant 同 workspace 自然合併（lookup 容易）。前端不知 path 雜湊規則，傳遞容易出錯。UUID4 每次新建破壞「同 workspace 跨 session 連貫」的稽核敘事。

```python
def workspace_id_for_path(path: Path) -> str:
    canonical = path.resolve().as_posix().lower()  # case-insensitive on Windows
    digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
    return f"ws_{digest[:12]}"
```

`workspace_id` 是 audit log 的次主鍵（主鍵 = `ts`），用於 last_grant lookup 篩選。

### D-A5：`session_id` 由 sidecar 在 grant_issued 時生成

**選項：**
- (a) sidecar UUID4 ✓
- (b) sidecar 復用 bearer token 前 12 字
- (c) 前端傳

**選 (a)。** session_id 是 grant 後動作的歸屬標識，必須與 bearer 解耦（bearer 是 transport secret，不應流入 audit log；audit log 必須能在不洩露 bearer 的前提下追蹤 session）。每次成功 grant 產生新 session_id，寫進 audit + 回前端，前端後續所有 endpoint call 帶這個 id（query param 或 X-Session-Id header）。

P0 不做 server-side session 表（in-memory 即可），重啟 sidecar = 失去 session = 必須 grant 一次。重啟頻率低，這個 trade-off 可接受。

### D-A6：scope upgrade 比對讀 audit log，不做 in-memory cache

**選項：**
- (a) 每次 `GET /auth/status` 重讀 `authorization_audit.jsonl` 篩 workspace_id ✓
- (b) sidecar 啟動時 load 整個 audit log 進 dict
- (c) 前端傳「上次 acked_kinds」

**選 (a)。** Audit log 線性掃描 `~/.codebus/authorization_audit.jsonl` 對 1000+ 條也是亞秒級操作；in-memory cache 的同步成本（多進程？多 sidecar 實例？）反而引入新雷。前端傳容易被前端 bug 污染信任鏈。

實作：

```python
def find_last_grant_for_workspace(workspace_id: str, audit_path: Path) -> dict | None:
    if not audit_path.exists():
        return None
    last_grant = None
    with audit_path.open() as f:
        for line in f:
            entry = json.loads(line)
            if entry.get("event") == "grant_issued" and entry.get("workspace_id") == workspace_id:
                last_grant = entry
    return last_grant  # latest matching grant_issued, None 若 workspace 從未授權
```

`acked_kinds` 從 `last_grant.user_ack` 提取所有以 `new_kind:` 開頭的條目（去 prefix 後就是 kind 名）。

### D-A7：sanitizer dry-run 走既有 `POST /scan`，前端聚合 sanitize_stats

**選項：**
- (a) 前端用既有 `POST /scan` 拿 ScanResult 並聚合 ✓
- (b) 加 `?summary=true` query 回精簡 payload
- (c) 新 endpoint `POST /auth/scan-preview`

**選 (a)。** 既有 ScanResult 已含 `files[*].sanitize_stats: dict[str, int]`，前端聚合 `Object.values(scanResult.files).reduce(...)` 得 `{secret: 12, email: 47, ...}`。Payload 浪費（含 `content` 欄）是 P1 優化；P0 1000 file workspace 約 10MB JSON，loopback 走 localhost 在毫秒內完成，可接受。

### D-A8：`POST /auth/grant` 在收到請求時做 workspace_root 驗證

mirror `SCANNER_WORKSPACE_INVALID` pattern：

- 路徑必須是絕對路徑、存在、是 directory（透過 `pathlib.Path.is_dir()`）
- 不通過 → `400 AUTH_WORKSPACE_INVALID`
- 通過 → 寫 audit + 建 in-memory session + 200 回 `{session_id, workspace_id, granted_at}`

不做的事：
- 不檢查 workspace_root 是否真的可讀（避免 race；之後 `POST /scan` 會碰）
- 不檢查 sanitizer dry-run 是否過（前端負責先 scan、再 grant；scan 過了才打 grant）

### D-A9：`POST /auth/deny` / `POST /auth/revoke` 純記錄、不影響 sidecar 狀態

deny 與 revoke 都只是 audit log 寫入。deny 在使用者點 cancel 時打，sidecar 不 spawn session。revoke 在 session 已存在時打，sidecar tear down session（in-memory dict 移除）。

revoke trigger MVP 只支援 `settings_revoke`（即使 Settings UI 入口 P1，endpoint trigger 欄位也只接受 `settings_revoke`，避免後續 P1 落地時 schema breaking）。`rules_version_bump` / `provider_change` / `workspace_deleted` 三 trigger 是 P1。

### D-A10：`useSidecar` typed wrapper 加在既有 composable，不新建

**選項：**
- (a) 既有 `useSidecar()` 回傳值加 4 個 method ✓
- (b) 新 composable `useAuth()`
- (c) 前端組件直接呼 `useSidecar().fetch('/auth/grant')`

**選 (a)。** 4 個 typed wrapper 用同一 bearer / baseUrl，沒有獨立狀態，不需新 composable 包裹。useSidecar 的 SidecarApi interface 從 4 欄擴 8 欄：

```typescript
interface SidecarApi {
  bearer: Ref<string>
  baseUrl: Ref<string>
  ready: Ref<boolean>
  fetch: typeof fetch
  // auth-flow additions:
  grant: (req: GrantRequest) => Promise<GrantResponse>
  deny: (req: DenyRequest) => Promise<void>
  revoke: (req: RevokeRequest) => Promise<void>
  status: () => Promise<AuthStatusResponse>
}
```

`web/app/composables/useAuthorization.ts` 是另一個 composable，責任範圍是 modal **flow state**（當前情境 `first_run` / `scope_reconfirm` / `scope_upgrade_new_kind`、ack checkbox 狀態、是否能 enable submit button），與 sidecar IPC 解耦——這是兩件事。

### D-A11：Auth HTTP error codes 走獨立 module，不污染 SSE `ERROR_CODES`

**選項：**
- (a) 把 4 個 auth code 加進 `tasks.py::ERROR_CODES` frozenset
- (b) 新 module `sidecar/src/codebus_agent/auth/errors.py` 定義 auth-specific code 常數 ✓
- (c) 直接寫字串字面量

**選 (b)。** `tasks.py::ERROR_CODES` 是 SSE background task wire-error 的封閉集合（`Background task error containment` Requirement 鎖死「ten codes, no more, no fewer」）；auth endpoints 是 **sync HTTP**，error 走 `HTTPException(status_code=4xx, detail={"code": "AUTH_*", "message": "..."})`，與 SSE 通道完全解耦。混進 ERROR_CODES 會（1）破壞 SSE drift guard test 既有契約 / （2）讓「frozenset 是 SSE 通道唯一錯誤碼集合」這條不變式失效 / （3）跨類型 enum 維護成本上升。

`auth/errors.py` 定義 3 個 sync HTTP error code 常數（P0）：

```python
AUTH_WORKSPACE_INVALID = "AUTH_WORKSPACE_INVALID"  # 400 — workspace_root 路徑不存在 / 非目錄
AUTH_NO_ACTIVE_GRANT = "AUTH_NO_ACTIVE_GRANT"      # 404 — revoke 時 session_id 對應 grant 不存在
AUTH_INVALID_REQUEST = "AUTH_INVALID_REQUEST"      # 400 — schema 驗證以外的 request 邏輯錯誤（例 user_ack flag 與 scenario 不對齊）
```

P0 不引入 `AUTH_RULES_VERSION_MISMATCH`：P0 沒任何程式碼路徑會 raise 它（rules version 比對邏輯整段 P1）。P1 落地時新增 `AUTH_RULES_VERSION_MISMATCH = "AUTH_RULES_VERSION_MISMATCH"`，新增當下對 P0 既有 callsite 零影響。

新 defensive test：`test_auth_error_codes_disjoint_from_sse_error_codes` 鎖死 `auth/errors.py` 三常數與 `tasks.py::ERROR_CODES` 交集為空——避免將來不小心混進去。

## Risks / Trade-offs

### R-1：rules_version 格式現狀有遺技債

**風險：** `docs/authorization.md §六` 整段 wording 改為 opaque 是「迴避」而非「解決」——將來 rules major bump 真的發生時，date format 沒辦法表達 MAJOR/MINOR/PATCH 區分，必須切成 semver 並 migrate 既有 audit log。

**緩解：** P0 階段不影響任何行為（rules_version 只是 verbatim 字串）。P1 follow-up change 開始時再決定：(i) 切 semver 並寫 migration / (ii) 維持 date 但加「is_breaking_change」manual flag。Spec 內容不會 lock 死格式，未來 P1 change 可校正。

### R-2：P0 三情境模糊地帶——`scope_reconfirm` vs `scope_upgrade_new_kind` 邊界

`scope_reconfirm` = 同 workspace 重開、scope 完全沒變；`scope_upgrade_new_kind` = 同 workspace 重開、scan 偵測到新 kind 類別。但「scan 偵測」依賴於 sanitizer rules 集合——若 rules_version 沒變但 workspace 內容改了（新檔含新類型），這也算 `scope_upgrade_new_kind`。

**緩解：** 比對邏輯純粹基於「current scan kinds vs last acked kinds」，不關心 rules_version 變化。文檔上釐清這個語意。

### R-3：session_id in-memory 重啟即失，可能造成使用者困惑

**風險：** 使用者重開 App、sidecar 重啟、原本 grant 過的 session 失效——但 audit log 還寫著「ws_xxx 已授權」。前端打 `GET /auth/status` 會回 `has_active_grant: false`，但 Settings 看 audit log 又顯示「上次 grant 時間」，語意有歧義。

**緩解：** P0 接受這個 trade-off。`GET /auth/status` 回 `has_active_grant: false` + `last_grant_for_workspace: {...}` 兩個欄位都帶，前端 UI 文案區分「曾經授權過、但需重新確認」與「從未授權」。Settings 頁（P1）拿 audit log 顯示歷史，不依賴 `has_active_grant`。

### R-4：workspace_id 雜湊在 path rename 後失效

**風險：** 使用者把 workspace 從 `~/projects/timeline` 重新命名為 `~/projects/timeline-old`，sha256 雜湊改變，audit log 看起來像兩個 workspace。

**緩解：** P0 不解決這個案例（屬 workspace lifecycle，與 audit 解耦）。文檔註明 workspace_id 是「path-derived stable id」，不是 workspace 本體 id。Phase 2 做 workspace registry 時統一處理。

### R-5：auth-specific error code 散落兩處的維護負擔

`auth/errors.py` 三常數 vs `tasks.py::ERROR_CODES` 十元素 frozenset，將來閱讀者可能不清楚為何 auth code 不在 `tasks.py` 裡。

**緩解：** 在兩個檔頭加交叉引用註解（`auth/errors.py` 註明「sync HTTP 專用，與 SSE `ERROR_CODES` 互斥；分離理由見 design D-A11」；`tasks.py::ERROR_CODES` 註解註明「SSE background task wire-error only；auth 走 `auth/errors.py`」）。新 defensive test `test_auth_error_codes_disjoint_from_sse_error_codes` 鎖死交集為空，防止後人不小心混進。

## Open Questions

以下問題在 propose 階段已決，列出避免後人重新質疑：

1. **Q：是否該為 auth endpoints 新建 router file？** A：是，`api/auth.py`，mirror scan/kb/explore/generate/qa 的命名規律。
2. **Q：是否該將 `authorization_audit.jsonl` 加進 `_audit_paths.py`？** A：否，新 leaf `auth/paths.py`（語意分離 + `_audit_paths.py:37-39` 已預留註解）。
3. **Q：是否該包 `useAuth()` composable 包裝所有 auth IPC？** A：否，typed wrapper 加進既有 `useSidecar()`；`useAuthorization.ts` 是 modal flow state composable，不是 IPC layer。
4. **Q：rules_version 矛盾在本 change 是「校正 docs」還是「校正 code」？** A：校正 docs（code 已 stable 18+ archive，scope 太大）。
5. **Q：是否該把 last_grant lookup 結果 cache 進 sidecar in-memory？** A：否，每次 status 請求重讀；audit log 線性掃描成本可接受。
6. **Q：是否需要 `POST /auth/grant` 同時做一次 sanitizer dry-run？** A：否，前端先 scan、再 grant；分離職責。
