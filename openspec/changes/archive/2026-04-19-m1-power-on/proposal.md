## Why

關聯 ADR：**D-001**（混合架構）、**D-013**（Monorepo + 目錄分層）、**D-014**（uv toolchain）、**D-017**（ToolSandbox）、**D-021**（UsageTracker）、**D-022**（LLMCallLogger）。

Repo 目前只有 spec（19 份 docs + 14 份 design mockup），`tauri/` / `sidecar/` / `web/` 尚未建立。繼續補 spec 的邊際效益遞減 — 真正能暴露 spec 矛盾與整合風險的是動手跑一次 end-to-end。本 change 對應 `docs/implementation-plan.md` §二「第一階段：基建與協議」步驟 1–8.5（里程碑 **M1 通電**，工期約 6.5 工作天），目的是把三層 retrofit 成本最高的橫切層（**Sandbox `ensure_in_workspace` helper**、**UsageTracker**、**LLMCallLogger**）連同骨架一次釘死，後續 Sanitizer / Agent / Scanner 任務才有地基可疊。

## What Changes

以下改動全部對齊 `implementation-plan.md` §二 步驟 1–8.5：

- **[#1] Monorepo 骨架**：建立 `tauri/`、`sidecar/`、`web/`、`tests/fixtures/`（D-013）；`sidecar/` 以 uv 初始化（D-014）；`.pre-commit-config.yaml` 骨架（先掛 EOF / trailing-whitespace 等 stage-0 hook，等各語言實作就緒再掛 ruff / pyright / eslint / cargo fmt / cargo clippy）
- **[#2] Tauri 2.0 殼 + Nuxt3 Hello World**：Cargo workspace + `cargo tauri dev` 可啟動；Nuxt3 + Tailwind + npm（D-026）首頁顯示 "CodeBus"；`fs.scope` 鎖 workspace 白名單（`docs/tool-sandbox.md §七`）
- **[#3] FastAPI sidecar + Bearer token + `/healthz`**：`uv run python -m codebus_agent.api --dev` 啟動 `localhost:<random-port>`；所有 endpoint 要求 `Authorization: Bearer <token>`，token 僅在啟動時由 Tauri 注入環境變數（`docs/sidecar-api.md §一`）
- **[#4] Tauri ↔ Sidecar HTTP ping**：Tauri 按鈕觸發 `invoke('sidecar_ping')` → 回 `{ status: "ok", pid, started_at }`；失敗要能區分「port 未監聽」vs「bearer 不對」vs「timeout」
- **[#5] ToolContext + Sandbox `ensure_in_workspace` helper + red team fixture**：`ToolContext` pydantic model 帶 `workspace_type: "folder" | "topic"`（**雙模 discriminator day 1** 不變式，D-023）；`ensure_in_workspace(path, ctx)` 阻擋 path escape（symlink / `..` / UNC path / `\\?\` prefix）；red team fixture 覆蓋 `docs/tool-sandbox.md §十五` 所有 attack vector
- **[#6] PyInstaller 打包驗證**：`pyinstaller codebus_agent.spec` 能在 Windows / macOS / Linux 產出 binary；binary `./codebus-sidecar --healthz` 自檢通過；Tauri 透過 `externalBin` 內嵌 sidecar binary（`docs/dev-setup.md`）
- **[#7] Qdrant 本地 + Python client 連通**：本地跑 Qdrant（docker 或 binary）；sidecar 透過 `qdrant-client` 能 `create_collection` / `upsert` / `search` 一個 dummy payload（`docs/module-2-kb-builder.md §三`）
- **[#8] LLM Provider Protocol + Mock provider + Instructor 接線**：`LLMProvider` Protocol 定義（`chat` / `embed`）；Mock provider 回固定 response；Instructor/Pydantic 對 mock output 做 structured output 解析（D-003, D-012）
- **[#8.5] UsageTracker + LLMCallLogger + TrackedProvider wrapper**：兩層稽核 JSONL 寫檔骨架（`token_usage.jsonl` / `llm_calls.jsonl`）；`TrackedProvider` 裝飾 `LLMProvider`，每次 call 前後都寫 JSONL（D-021, D-022）

**M1 守則（invariant）**：步驟 #8 LLM Provider 在本 change 期間**只實作 Protocol + Mock provider**，**禁止**發任何真實對外 LLM call。這守住 `implementation-plan.md` §一「Sanitizer Pass 1/2 必須在第一次 LLM call 前落地」— 真 provider call 延到後續 `m2-safety-layer` change 完成 Sanitizer 後才解鎖。

## Non-Goals

以下**明確不在本 change 範圍**，避免範圍蔓延：

- **Sanitizer 任一 Pass**（Pass 1 / Pass 2 / Pass 3） — 屬 M2「安全落地」，後續 `m2-safety-layer` change
- **Module 1 Scanner**（檔案遍歷 / gitignore / encoding / ScanResult schema） — 屬 M3，`implementation-plan.md` §三
- **Module 2 KB Builder 真實 pipeline**（chunk / embed / content-hash 去重） — 屬 M3；本 change 只驗證 Qdrant client 能連通，不跑實際 ingest
- **Explorer / Q&A Agent ReAct loop** — 屬 M4 / M5
- **Module 5 Generator、互動 Markdown 元件、Agent console 前端** — 屬 M5 / M6
- **真實對外 LLM call**（OpenAI / Anthropic / Gemini / Ollama 任一 provider） — 本 change 只實作 Mock；真 call 延到 Sanitizer Pass 1/2 落地後
- **Topic 模式（`workspace_type: "topic"`）之實作邏輯** — schema 欄位必須 day 1 就在（不變式），但 MVP 僅實作 `folder` 路徑（D-002）
- **七層 audit 中除 `token_usage.jsonl` / `llm_calls.jsonl` 之外的五層** — `sanitize_audit` 等 M2、`tool_audit` 等 M3、`kb_growth` / `reasoning_log` 等 M4–M5
- **跨平台 packaging CI matrix** — 本 change 只需在主開發平台（Windows + macOS 或 Linux 其一）驗證 PyInstaller 產 binary 可 smoke 測；跨平台完整矩陣延到打磨期
- **真實 linter（ruff / pyright / eslint / cargo fmt / cargo clippy）規則調校** — 本 change 只掛 pre-commit stage-0 hook；linter 規則待各語言實作進度陸續加入

## Capabilities

### New Capabilities

- `repo-layout`：Monorepo 目錄骨架（`tauri/` / `sidecar/` / `web/` / `tests/fixtures/`）、uv toolchain 初始化、`.pre-commit-config.yaml` 骨架、各語言 manifest（`Cargo.toml` / `pyproject.toml` / `package.json`）
- `tauri-shell`：Tauri 2.0 主殼 + Nuxt3 Hello World 前端 + `fs.scope` workspace 白名單
- `sidecar-runtime`：FastAPI sidecar + Bearer token 認證 + `/healthz` endpoint + Tauri↔Sidecar HTTP ping 握手（localhost 隨機 port + startup token 注入）
- `tool-sandbox`：`ToolContext` schema（含 `workspace_type` 雙模 discriminator）、`ensure_in_workspace` path-escape 阻擋、red team fixture 驗證攻擊面
- `app-packaging`：PyInstaller sidecar binary 打包驗證 + Tauri `externalBin` 內嵌鏈（`cargo tauri build` smoke 測）
- `qdrant-client`：本地 Qdrant 啟動程序 + sidecar 端 qdrant-client 連通驗證（dummy collection / upsert / search）
- `llm-provider`：`LLMProvider` Protocol（`chat` / `embed`）+ Mock provider 實作 + Instructor/Pydantic structured output 接線
- `usage-tracking`：`UsageTracker` + `LLMCallLogger` 兩層稽核 JSONL 寫檔 + `TrackedProvider` 裝飾器 wrapper（所有未來 provider call 強制過此 wrapper）

### Modified Capabilities

（無 — Spectra spec 目錄目前為空，本 change 不 modify 任何既有 capability）

## Impact

**新建目錄 / 檔案**：

- 實作目錄：`tauri/`、`sidecar/`、`web/`、`tests/fixtures/`
- 各語言 manifest：`tauri/src-tauri/Cargo.toml`、`sidecar/pyproject.toml`、`web/package.json`
- 打包 spec：`sidecar/codebus-sidecar.spec`（PyInstaller）
- Toolchain：`sidecar/uv.lock`、`web/package-lock.json`（D-026 起改用 npm）
- Commit gate：`.pre-commit-config.yaml`（stage-0 hook；真 linter 待後續補）

**新增 Spectra spec**（每項對應一個 capability 的 `spec.md`）：

- `openspec/specs/repo-layout/spec.md`
- `openspec/specs/tauri-shell/spec.md`
- `openspec/specs/sidecar-runtime/spec.md`
- `openspec/specs/tool-sandbox/spec.md`
- `openspec/specs/app-packaging/spec.md`
- `openspec/specs/qdrant-client/spec.md`
- `openspec/specs/llm-provider/spec.md`
- `openspec/specs/usage-tracking/spec.md`

**新增 Audit JSONL（骨架）**：`codebus-workspace/token_usage.jsonl`、`codebus-workspace/llm_calls.jsonl`（其餘五層延到 M2–M5）

**依賴**：

- Rust：tauri 2.x、serde、tokio
- Python（uv-managed）：fastapi、uvicorn、pydantic、instructor、qdrant-client、pyinstaller、pytest、pytest-asyncio
- TS / Node：nuxt 3.x、@nuxtjs/tailwindcss、typescript（package manager 用 npm，見 D-026）
- 外部 binary：Qdrant（docker image 或 standalone binary）、pre-commit（已安裝於 `~/.local/bin/pre-commit`）

**不觸碰**：既有 `docs/*.md`（spec 凍結）、既有 `design/*.html`（Phase A mockup）、既有 `tests/golden/*`（demo / adapter fixture）。若實作過程發現 spec 矛盾，須先回 `docs/decisions.md` 新增 D-XXX 再 bump spec，再回頭改本 change。
