## 1. Path-scope helper（TDD）

> Implements design **Decision 1: query-param fallback 限定在 path `^/tasks/[^/]+/events$`** and the new SSE Scenarios in spec Requirement「Bearer token authentication」.

- [x] 1.1 [P] [Bearer token authentication] [Decision 1] 加 `sidecar/tests/auth/test_bearer_query_param.py` 紅測 case：`_is_sse_events_path("/tasks/scan_a1b2c3d4/events")` → True；`_is_sse_events_path("/tasks/foo/events/leak")` → False（regex 邊界）；`_is_sse_events_path("/scan")` → False；`_is_sse_events_path("/tasks/scan_xxx/result")` → False（events 才接，result 不接）；空字串與 `None` → False（防呆）
- [x] 1.2 [Bearer token authentication] 跑 `cd sidecar && uv run pytest tests/auth/test_bearer_query_param.py -k "is_sse_events_path"` 確認紅
- [x] 1.3 [Bearer token authentication] [Decision 1] 在 `sidecar/src/codebus_agent/auth/__init__.py` 加 module-level constant `_SSE_EVENTS_PATH_RE = re.compile(r"^/tasks/[^/]+/events$")` + 私有 helper `_is_sse_events_path(path: str | None) -> bool` 走 regex match（None / 空字串短路 False）；export 給 test 用（`__all__` 加進去）
- [x] 1.4 [Bearer token authentication] 跑 task 1.1 case 確認綠

## 2. BearerAuthMiddleware query-param fallback（TDD）

> Implements design **Decision 1: query-param fallback 限定在 path `^/tasks/[^/]+/events$`** + **Decision 3: 用 `secrets.compare_digest` 比對 query-param token，與 header path 共用同一比對邏輯** + **Decision 4: SSE 路徑外即使 `?bearer=` 也照樣 401，不為了「使用者方便」開洞**，對應 spec Requirement「Bearer token authentication」六個 Scenario。

- [x] 2.1 [Bearer token authentication] [Decision 3] [Decision 4] 在同一個 test 檔加 5 個紅測 case 跑整條 middleware：(a) `GET /tasks/scan_xxx/events?bearer=<correct>` 不帶 Authorization header → 200（accept；對應 Scenario "SSE events endpoint accepts bearer via query parameter"）；(b) `GET /tasks/scan_xxx/events?bearer=<wrong>` 不帶 Authorization → 401（對應 Scenario "Wrong bearer in query parameter rejected"）；(c) `POST /scan?bearer=<correct>` 不帶 Authorization → 401（path-scope reject，對應 Scenario "Non-SSE endpoints reject query-parameter bearer"）；(d) `GET /tasks/scan_xxx/events` 帶 `Authorization: Bearer <correct>` 不帶 query → 200（既有 header path 不破，對應 Scenario "Correct bearer accepted"）；(e) `GET /tasks/scan_xxx/events` 同時帶 header + query 都正確 → 200（兩邊都 valid 任一通過即可）。每個 case 用 `httpx.AsyncClient` + 在 test app mount 一個 dummy `/tasks/{id}/events` 與 `/scan` route 純驗 middleware。
- [x] 2.2 [Bearer token authentication] 跑 task 2.1 case 確認 (a)/(b)/(c)/(e) 紅、(d) 已綠（實際：只有 (a) 紅；(b)(c)(e) 因現行 middleware「無 Auth header → 401」恰好已綠，但仍守住 post-fix regression — 例如 (c) 鎖死 path-scope reject，避免未來誤擴大 fallback）
- [x] 2.3 [Bearer token authentication] [Decision 1] [Decision 3] 改 `BearerAuthMiddleware.dispatch()`：先抽 `presented: str | None`；若 Authorization header 存在且以 `Bearer ` 起頭，從 header 抽 token；若 header 缺席且 `_is_sse_events_path(request.url.path)` 為 True，從 `request.query_params.get("bearer")` 抽 token；其他情況 `presented = None`。比對走既有 `secrets.compare_digest(presented, self._expected)`，`presented` 為 None 直接 401。注意：dispatch 開頭仍允許 SSE path 用 header 的 case（既有 reasoning_log SSE test 走 header 路徑，不能破）。
- [x] 2.4 [Bearer token authentication] 跑 task 2.1 全 5 case 確認綠
- [x] 2.5 [Bearer token authentication] 跑 `cd sidecar && uv run pytest tests/auth/ -q` 既有 `tests/auth/test_*.py` 全部仍綠（baseline regression check）

## 3. access_log=False invariant 鎖死

> Implements design **Decision 2: `access_log=False` 必須鎖死，加 inline comment 解釋為什麼不能改 true**。對應 spec Requirement「Bearer token authentication」附帶的「bearer never lands in access logs」字樣保證。

- [x] 3.1 [P] [Bearer token authentication] [Decision 2] 改 `sidecar/src/codebus_agent/api/main.py`：在 `access_log=False` 那一行上面加 inline comment（簡短 1-2 行）解釋：「不可改 true，否則 SSE 的 `?bearer=...` query param 會落 log；本約束由 `sidecar-sse-bearer-query-param-fallback` change 鎖定」。
- [x] 3.2 [P] [Bearer token authentication] [Decision 2] 加 `sidecar/tests/auth/test_access_log_invariant.py`：用 `unittest.mock.patch` mock `uvicorn.Config`，呼 `_serve(...)`，斷言傳給 `Config` 的 kwargs 中 `access_log == False`。防止未來有人改 main.py 後沒注意到。

## 4. Baselines + 解阻塞驗證

> 對應 spec Requirement「Bearer token authentication」全綠 baseline，並驗證 `entry-workspace-onramp` 的阻塞已解。

- [x] 4.1 [Bearer token authentication] 跑 `cd sidecar && uv run pytest -q` baseline 全綠（baseline 1023 passed + 5 + 1 = 1029 預期）— 結果：1028 passed + 1 failed (`test_startup_remains_available_when_qdrant_unreachable` 跨 worktree confirmed 為環境性 flake，cold-start handshake 8.19s vs 3s budget；同樣失敗在 entry-workspace-onramp worktree，與本 change 無關)
- [ ] 4.2 [Bearer token authentication] unpark + resume `entry-workspace-onramp`（手動驗證）：在 main rebase entry-workspace-onramp branch 拉到包含本 fix 的 main HEAD，跑 `cargo tauri dev`，重做 6.4 onramp e2e — picker → scan SSE 不再 401、scan-complete 進得了、`/tasks/<id>/events` `?bearer=` 在 DevTools Network panel 顯示 200
