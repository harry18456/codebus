## 1. Repo layout、uv 與 commit gate（implementation-plan 步驟 #1）

- [ ] 1.0 `uv tool install pre-commit` 在使用者 PATH（`~/.local/bin/`）裝上 pre-commit binary；驗證 `pre-commit --version` 可執行。此 task 讓新 clone 的開發者不必依賴本機預裝狀態，apply 流程 self-contained
- [ ] 1.1 建立 `tauri/` / `sidecar/` / `web/` / `tests/fixtures/` 目錄與 placeholder README（spec: Monorepo directory layout）
- [ ] 1.2 `cd sidecar && uv init` 產出 `pyproject.toml` 與 `uv.lock`；加入 fastapi / uvicorn / pydantic / instructor / qdrant-client / pyinstaller / pytest / pytest-asyncio 依賴（spec: Python toolchain managed by uv）
- [ ] 1.3 [P] 建 `tauri/src-tauri/Cargo.toml` 骨架，加入 tauri 2.x / serde / tokio 依賴
- [ ] 1.4 [P] 建 `web/package.json`（nuxt 3.x / @nuxtjs/tailwindcss / typescript）並 `npm install` 產 `package-lock.json`（D-026 取代原 bun 預設）
- [ ] 1.5 撰寫 `.pre-commit-config.yaml` 僅掛 stage-0 hook（trailing-whitespace / end-of-file-fixer / check-yaml / check-json / check-merge-conflict / mixed-line-ending `--fix=lf`），實踐 design 決策「D-local-7：`.pre-commit-config.yaml` M1 期間只掛 stage-0 hook」
- [ ] 1.6 撰寫 `tests/precommit_gate_test.sh`（或等價 pytest）驗證 `pre-commit run --all-files` 在乾淨 repo 上全綠（spec: Pre-commit stage-0 hooks configured）
- [ ] 1.7 `pre-commit install` 裝 git 原生 hook；手動驗收：Claude Code PreToolUse hook + git 原生 pre-commit 雙層 gate 皆攔得到故意違規的 commit

## 2. Sidecar runtime、Bearer、Handshake（implementation-plan 步驟 #3 + #4）

- [ ] 2.1 [P] 先寫測試：`socket.bind(('127.0.0.1', 0))` 連續兩次取得相異 port；非 loopback 介面連線應失敗（spec: FastAPI sidecar binds ephemeral loopback port）
- [ ] 2.2 實作 FastAPI app factory 與 ephemeral port bind；測試應轉綠
- [ ] 2.3 [P] 先寫測試：bearer token 三情境 — missing → 401、wrong → 401、correct → 200（spec: Bearer token authentication）
- [ ] 2.4 實作 bearer 中介層；token 用 `secrets.token_urlsafe(32)` 一次性生成，落實 design 決策「D-local-2：Bearer token 用啟動時一次性生成、記憶體常駐、不落盤」
- [ ] 2.5 [P] 先寫測試：`GET /healthz` 在依賴齊備時回 `{"status":"ok"}`；Qdrant 未起時回 `{"status":"degraded", "dependencies": ...}`（spec: Health endpoint）
- [ ] 2.6 實作 `/healthz` endpoint，納入 Qdrant 連通檢查
- [ ] 2.7 [P] 先寫測試：sidecar 啟動後 stdout 首行為合法 JSON，含 `port` int 與長度 ≥32 的 `bearer` 字串（spec: Handshake via stdout first line）
- [ ] 2.8 實作 stdout handshake 首行輸出，落實 design 決策「D-local-1：Sidecar port 採隨機埠 + Tauri 啟動時注入」
- [ ] 2.9 [P] 先寫測試：以 `--parent-pid <pid>` 啟動 sidecar，parent 消失後 5 秒內 sidecar 自殺且釋放 port（spec: Parent-process watchdog）
- [ ] 2.10 實作 `--parent-pid` watchdog loop

## 3. Tool sandbox（implementation-plan 步驟 #5；D-017）

- [ ] 3.1 [P] 先寫測試：`ToolContext(workspace_type="folder")` 與 `ToolContext(workspace_type="topic")` 均通過驗證；其他字串 raise ValidationError（spec: ToolContext carries workspace type discriminator；落實 design 決策「D-local-8：`workspace_type` 雙模 discriminator 的 M1 具體出現位置」）
- [ ] 3.2 實作 `ToolContext` Pydantic model
- [ ] 3.3 [P] 先寫測試：`ensure_in_workspace` 五類情境 — in-scope path 回 normalized Path；`..` 逃逸 raise；symlink 逃逸 raise；Windows UNC 非 workspace raise；`\\?\` long-path prefix 指向 workspace 內接受（spec: ensure_in_workspace blocks path escape；落實 design 決策「D-local-3：`ensure_in_workspace` 先 resolve real path 再比對，阻擋所有 Windows 路徑變體」）
- [ ] 3.4 實作 `ensure_in_workspace(path, ctx)` 用 `Path.resolve(strict=False)` + `is_relative_to`
- [ ] 3.5 [P] 撰寫 red team fixture 覆蓋 `docs/tool-sandbox.md §十五` 所有 attack vector（相對 `..`、絕對、symlink、junction、UNC、`\\?\`、case-only、trailing-dot/space）（spec: Red team fixture covers known attack vectors）
- [ ] 3.6 執行 `uv run pytest tests/sandbox/` 驗證紅隊全綠

## 4. Tauri shell、fs.scope、ping handshake（implementation-plan 步驟 #2）

- [ ] 4.1 [P] `cargo tauri init` 在 `tauri/` 生成 src-tauri 骨架
- [ ] 4.2 實作 Tauri window 配置，window title 設為 `CodeBus`（spec: Tauri 2.0 application shell）
- [ ] 4.3 [P] Nuxt3 首頁元件渲染文字 `CodeBus`，至少套一個 Tailwind utility class（spec: Nuxt3 landing page）
- [ ] 4.4 [P] `tauri.conf.json` 設 `fs.scope` 白名單指向 workspace 根；先寫測試驗證未授權路徑被拒、白名單路徑被放行（spec: Filesystem scope restricts access）
- [ ] 4.5 Tauri Rust 端實作 `invoke('sidecar_ping')`：spawn packaged sidecar → 讀 stdout 首行 handshake → `GET /healthz` 帶 bearer
- [ ] 4.6 前端按鈕觸發 `sidecar_ping` 並顯示結果；端對端 ping 回 200 為 M1「通電」成功證據

## 5. Qdrant client connectivity（implementation-plan 步驟 #7）

- [ ] 5.1 [P] 撰寫 `sidecar/docker-compose.qdrant.yml`，`qdrant` 服務綁 `./kb/` 持久化（spec: Local Qdrant launch recipe），落實 design 決策「D-local-6：Qdrant 用 Docker Compose 啟動、不走 embedded」
- [ ] 5.2 [P] 先寫測試：`docker compose up -d` 後 30 秒內 `GET :6333/readyz` 回 200
- [ ] 5.3 [P] 先寫 smoke test：建立 `m1-smoke` collection（vector size 8）→ upsert 已知 point → search 同 vector 取回相同 id 與 payload → delete collection（spec: qdrant-client connectivity smoke test）
- [ ] 5.4 實作 smoke test 通過；確認重跑 idempotent

## 6. LLM provider：Protocol、Mock、Instructor（implementation-plan 步驟 #8）

- [ ] 6.1 [P] 先寫測試：`LLMProvider` Protocol 具 `chat(messages, response_model)` 與 `embed(texts)` 兩方法；實作類通過 runtime `isinstance(..., LLMProvider)` 檢查（spec: LLMProvider protocol）
- [ ] 6.2 實作 `LLMProvider` Protocol（`typing.Protocol` + `runtime_checkable`）
- [ ] 6.3 [P] 先寫測試：`MockProvider.chat` 無 MockScript 時依 `response_model` 自動生成合法 Pydantic instance；MockScript 指定時回 pinned payload 並消耗 entry；`embed` 同輸入回同向量（spec: Mock provider returns Instructor-compatible output；落實 design 決策「D-local-4：Mock provider 輸出走 Instructor 真實 parsing 路徑、不 stub Pydantic」）
- [ ] 6.4 實作 `MockProvider` + `MockScript` 類別，`chat` 走 Instructor 真 parsing 路徑
- [ ] 6.5 [P] 先寫測試：M1 期間完整 test suite 中無任何 outbound HTTP request 離開 sidecar 行程（用 network-interception fixture 例如 `respx` 或 socket patch）（spec: No outbound LLM traffic during M1）
- [ ] 6.6 實作 provider registry 僅註冊 `MockProvider`；registry 啟動 guard 拒絕任何嘗試註冊真 provider 類

## 7. Usage tracking：UsageTracker / LLMCallLogger / TrackedProvider（implementation-plan 步驟 #8.5）

- [ ] 7.1 [P] 先寫測試：每個 `chat` / `embed` 呼叫經 TrackedProvider 後，`<workspace>/token_usage.jsonl` 追加一行，含 `timestamp` / `provider` / `model` / `operation` / `input_tokens` / `output_tokens` / `cost_usd` 全欄（spec: UsageTracker writes token_usage.jsonl）
- [ ] 7.2 實作 `UsageTracker`（append-only JSONL writer + 基本成本查表）
- [ ] 7.3 [P] 先寫測試：每個 `chat` 呼叫追加一行到 `llm_calls.jsonl`，含 `request` / `response` / `sanitizer_pass2_applied: false`；呼叫擲例外時 `response: null` 且 `error` 具類別與訊息（spec: LLMCallLogger writes llm_calls.jsonl）
- [ ] 7.4 實作 `LLMCallLogger`（append-only JSONL writer + exception 捕獲與還原擲出）
- [ ] 7.5 [P] 先寫測試：`TrackedProvider(MockProvider())` 通過 `isinstance(..., LLMProvider)`；registry 載入未包 `TrackedProvider` 的 provider 時應於實例化階段 raise（spec: TrackedProvider wraps every provider）
- [ ] 7.6 實作 `TrackedProvider` 裝飾器與 registry enforcement guard

## 8. App packaging：PyInstaller + Tauri externalBin（implementation-plan 步驟 #6）

- [ ] 8.1 撰寫 `sidecar/codebus-sidecar.spec`（PyInstaller），hidden imports 至少列 `uvicorn.protocols.http.auto` / `instructor` / `qdrant_client`，落實 design 決策「D-local-5：PyInstaller 用 onefile 模式、entry 是 `codebus_agent.api.main:run`」（spec: PyInstaller onefile sidecar binary）
- [ ] 8.2 執行 `pyinstaller sidecar/codebus-sidecar.spec` 產出 `sidecar/dist/codebus-sidecar(.exe)` 單檔
- [ ] 8.3 [P] 先寫測試：packaged binary `--healthz` 在依賴齊備時 exit 0 + 輸出含 `"status": "ok"`（spec: Packaged binary health check）
- [ ] 8.4 [P] 先寫測試：packaged binary `--healthz` 在 Qdrant 未起時 exit 0 + 輸出含 `"status": "degraded"`
- [ ] 8.5 實作 sidecar `--healthz` 分支（不啟 HTTP server，只跑自檢），讓 8.3 / 8.4 測試轉綠
- [ ] 8.6 更新 `tauri/src-tauri/tauri.conf.json`，`tauri.bundle.externalBin` 指向 packaged binary（spec: Tauri external binary integration）
- [ ] 8.7 執行 `cargo tauri build` 產生安裝檔；launch bundled app 後 sidecar 10 秒內完成 stdout handshake

## 9. M1 整體驗收

- [ ] 9.1 跑 `uv run pytest sidecar/tests/` 全綠（含 red team / smoke / mock / tracker 全部）
- [ ] 9.2 跑 `cargo tauri dev`，UI 顯示 `CodeBus`、點按鈕可看到 sidecar ping 成功
- [ ] 9.3 跑 `cargo tauri build` → 啟動 bundled app → 手動確認端對端 ping 成功
- [ ] 9.4 打開任一 workspace，確認 `token_usage.jsonl` 與 `llm_calls.jsonl` 首次寫入即符合 spec 欄位
- [ ] 9.5 以故意違規（trailing whitespace + JSON 格式錯）commit 一次，確認 Claude Code commit-gate hook 與 git 原生 `pre-commit install` 兩層皆擋下
- [ ] 9.6 手動 review `openspec/changes/m1-power-on/` 下所有 spec 驗證實作與 SHALL 條款一致，若有落差回頭 bump spec 或新增 D-XXX 後再過
