## Context

M1 通電是 repo 從 spec-first 跨進 code 的第一步。Proposal 已說明 **Why** 與 **What Changes**；本 design 只處理 proposal 無法單獨涵蓋、且 `docs/` 既有 spec 未明確的技術取捨。其餘細節（endpoint schema、JSONL 欄位、Sandbox 規則表）均已定於 `docs/sidecar-api.md` / `docs/tool-sandbox.md` / `docs/agent-core.md §十三`，本 design 只引用不重述。

**當前狀態**：`tauri/` / `sidecar/` / `web/` 尚未建立；`.claude/settings.json` 已掛 commit-gate PreToolUse hook；`.spectra.yaml` 啟用 `tdd` / `audit` / `worktree` / `parallel_tasks`；pre-commit binary 已安裝於 `~/.local/bin/pre-commit`。

**約束**：
- 開發主機 Windows + Git Bash + Opus 4.7；macOS / Linux 跨平台驗證延至打磨期
- uv toolchain（D-014）、npm（D-026 取代原 Bun 預設）、cargo 工具鏈使用者本機已具備
- 本 change 結束時 **禁止任何真實對外 LLM call**（M1 守則）

## Goals / Non-Goals

**Goals：**

- 地基可通電：Tauri → Sidecar HTTP ping 來回；PyInstaller binary `--healthz` smoke 通過；Qdrant client 能 CRUD dummy payload
- 三層 retrofit 成本最高的橫切層一次釘死：**Sandbox `ensure_in_workspace` helper**、**UsageTracker**、**LLMCallLogger**
- 雙模 discriminator（`workspace_type`）day 1 就在 schema（即使 MVP 只實作 folder）
- Mock provider 能過 Instructor/Pydantic structured output 解析，讓未來 Agent loop 可直接 plug-in 真 provider

**Non-Goals**（引用 proposal §Non-Goals，不重述）。

## Decisions

### D-local-1：Sidecar port 採隨機埠 + Tauri 啟動時注入

**決定**：sidecar 啟動時在 `127.0.0.1` 綁 ephemeral port（`socket.bind((host, 0))`），將實際 port 寫入 stdout 首行 JSON（`{"port": 54321, "bearer": "<token>"}`）；Tauri 作為父行程讀取首行、保留握手資料、後續 IPC 全用此 port + bearer。

**為何**（vs 選項 A 固定 port、選項 B config 檔寫死）：
- 固定 port 會與使用者環境既有服務衝突（Qdrant 預設 6333、開發 server 3000/8000 等）
- config 檔寫死需要 install 時寫入使用者目錄，增加打包複雜度
- 隨機 port + stdout 握手符合 `docs/sidecar-api.md §一`「localhost + 隨機 port + Bearer token」原則，且 PyInstaller 產 binary 後仍能運作

**風險 → 緩解**：firewall 干擾 ephemeral port → 僅 bind `127.0.0.1`（loopback，一般 firewall 不攔）；握手失敗 → Tauri 要能區分 timeout / bind 失敗 / bearer 不對，回 UI 具體錯誤碼。

### D-local-2：Bearer token 用啟動時一次性生成、記憶體常駐、不落盤

**決定**：sidecar 啟動時 `secrets.token_urlsafe(32)` 生成 bearer，寫入 stdout 首行給 Tauri 讀，之後僅在記憶體內比對；**不寫入 config / 環境變數檔 / keyring**。sidecar 被 kill 即失效，下次啟動重新生成。

**為何**（vs 選項 A 固定 token、選項 B OS keyring）：
- 一次性 token 意外洩漏後自然過期（sidecar 重啟即失效），攻擊視窗短
- 不落盤 → 不必處理「舊 token 殘留」「keyring 權限」「多實例衝突」等問題
- 符合 `docs/security.md §3.x` 「最小 attack surface」原則

**風險 → 緩解**：Tauri 崩潰後 sidecar 仍在跑但 Tauri 重啟拿不到 token → sidecar 需實作 `--parent-pid` flag，父行程消失即自殺（避免孤兒 sidecar 佔 port）。

### D-local-3：`ensure_in_workspace` 先 resolve real path 再比對，阻擋所有 Windows 路徑變體

**決定**：`ensure_in_workspace(path, ctx)` 實作：
1. `Path(path).resolve(strict=False)` → 展開 `..` / symlink / `\\?\` long-path prefix
2. 與 `ctx.workspace_root.resolve()` 比對，要求 `is_relative_to`
3. 對 UNC path（`\\server\share`）、drive-letter 變體（`C:` vs `c:`）、junction / reparse point 全部 normalize 後比對
4. Red team fixture 必須覆蓋：相對 `..`、絕對路徑、symlink 逃逸、junction 逃逸、UNC、`\\?\` prefix、大小寫差異、trailing dot / space（Windows 檔名奇葩）

**為何**（vs 選項 A 純字串 `startswith` 比對、選項 B chroot-style OS 層 sandbox）：
- 字串比對被 symlink / `..` 輕易繞過，`docs/tool-sandbox.md §十五` 已列明
- OS 層 sandbox（seccomp / sandbox-exec）Windows 不支援，跨平台成本高
- Path.resolve + is_relative_to 是 Python 3.11+ 原生、語義清楚、紅隊可窮舉

**風險 → 緩解**：`resolve()` 對不存在路徑行為 OS-specific → 統一用 `strict=False` 且在 helper 內處理 `FileNotFoundError`；效能（每次 call 都 resolve）→ tool 呼叫頻率不高（Agent 每 step 1-3 次），可接受。

### D-local-4：Mock provider 輸出走 Instructor 真實 parsing 路徑、不 stub Pydantic

**決定**：Mock provider 的 `chat(messages, response_model)` 實作 = 依 `response_model` 欄位生成合法 dummy 值（例：`BaseModel` 用 Pydantic `model_construct` + field default / factory），**不**繞過 Instructor 直接回 dict。

**為何**（vs 選項 A mock 直接 return 預設 dict）：
- M1 的價值在**驗證整條 chain 通暢**：Instructor 的 retry / validation / coercion 都能被真實 exercise
- 若 mock 繞過 Instructor，真 provider 接上後才會踩到 schema 不符等問題，違背「M1 通電」本意
- Instructor 本身已是 D-012 決策，mock 應沿用同一 code path

**風險 → 緩解**：Mock 生成 dummy 值時若 schema 複雜（nested / Union）可能失敗 → 提供 `MockScript` 機制，單測可指定精確輸出（fixture-driven mock），生產 Mock 只在沒指定時走自動生成。

### D-local-5：PyInstaller 用 onefile 模式、entry 是 `codebus_agent.api.main:run`

**決定**：
- `codebus-sidecar.spec` 使用 `--onefile` 打單一 binary（體積約 30-50 MB 可接受）
- Entry point：`codebus_agent.api.main:run`，內部啟動 uvicorn、綁 ephemeral port、stdout 握手
- 隱藏 import 清單明列：`uvicorn.protocols.http.auto`、`instructor`、`qdrant_client`（PyInstaller 靜態分析漏抓）
- Tauri `tauri.conf.json` 的 `externalBin` 指向產出 binary 路徑

**為何**（vs 選項 A onedir、選項 B Nuitka）：
- onefile → Tauri `externalBin` 只需一個檔，打包鏈最簡；啟動慢 ~1s 可用 UI loading 遮蔽
- Nuitka 編譯期長（分鐘級）、AOT 對 FastAPI / Instructor 等 runtime-heavy 套件收益有限

**風險 → 緩解**：隱藏 import 漏抓 → M1 smoke 測要 binary 級驗證（`./codebus-sidecar --healthz` 回 OK 才算過），不能只測 source 版；onefile 解壓到 temp 目錄會觸發防毒警報 → 文件註記，不阻止 M1。

### D-local-6：Qdrant 走 standalone binary（主路徑）+ Docker Compose（fallback）

> **更新（2026-04-19，D-027）**：原本主路徑為 Docker Compose，實作期發現 Docker Desktop 在 Windows / macOS 上門檻過高（~700 MB + WSL2 + 商用授權），違背「降低上手門檻」的專案定位。重新盤點後發現原反對 embedded binary 的理由（打包進 PyInstaller ~100 MB 爆 + 跨平台多份）並不適用於「**由使用者本機放 standalone binary**」的第三條路。故翻轉 D-local-6 主路徑為 standalone binary，Docker Compose 降為 fallback。見 `docs/decisions.md` D-027。

**決定**：
- **主路徑**：使用者從 Qdrant 官方 release 下載對應平台 binary（Win `.zip` / mac `.tar.gz` / Linux `.tar.gz`），解壓至 `~/.codebus/bin/qdrant(.exe)` 或 `$CODEBUS_QDRANT_BIN` 指向之路徑
- **啟動腳本**：`sidecar/scripts/start-qdrant.{ps1,sh}` 在 foreground 跑 binary 指向 `~/.codebus/kb/` 為 storage
- **Fallback**：`sidecar/docker-compose.qdrant.yml` 保留，`docs/dev-setup.md` 列為 CI / advanced 選項
- **sidecar 側**：透過 `CODEBUS_QDRANT_URL`（預設 `http://127.0.0.1:6333`）連線，**與啟動方式完全解耦** — binary 或 Docker 皆可；M1 smoke 測使用 `qdrant-client` 建 dummy collection 做 upsert / search

**為何**（vs 選項 A embedded rust binary、選項 B Qdrant cloud、選項 C Docker Compose）：
- Embedded binary 打包進 PyInstaller 體積會爆（~100 MB × 平台份數），且 Qdrant 升版綁 app 升版
- Qdrant cloud 違背「本地優先」D-009 claim
- Docker Compose 門檻過高（~700 MB Docker Desktop + WSL2 + 商用授權），M1 評審環境不可用
- Standalone binary 無 Docker 門檻、體積不進 PyInstaller、升版獨立 — 三個問題同時解

**風險 → 緩解**：首次啟動多一步「下載解壓」 → 啟動腳本偵測缺 binary 即印 `--help` 樣式訊息給下載連結；使用者若有既有 Docker 偏好 → compose 檔仍在，切換只需 `CODEBUS_QDRANT_URL` 環境變數。

### D-local-7：`.pre-commit-config.yaml` M1 期間只掛 stage-0 hook

**決定**：M1 `.pre-commit-config.yaml` 僅掛 `pre-commit-hooks` repo 的：
- `trailing-whitespace`
- `end-of-file-fixer`
- `check-yaml`
- `check-json`
- `check-merge-conflict`
- `mixed-line-ending`（`--fix=lf`）

各語言 linter（ruff / pyright / eslint / cargo fmt / cargo clippy）**不**在本 change 啟用，理由：
- 各語言實作剛起步，lint 規則還在流動，強制跑會卡 commit
- 骨架掛好先讓 commit gate 跑得起來，linter 規則與 fix-on-save 延至各語言 capability 實作到位後逐一開啟

**為何**（vs 選項 A 一次掛齊所有 linter、選項 B 完全不掛）：
- 一次掛齊 → 沒 code 時 linter 空跑、加上 bootstrap 階段常調整設定，體驗差
- 完全不掛 → 失去 `.claude/settings.json` commit-gate hook 的 enforce 對象（gate 永遠走 else 分支）
- 折衷掛 stage-0 → commit gate 立即有意義，linter 陸續 opt-in

### D-local-8：`workspace_type` 雙模 discriminator 的 M1 具體出現位置

**決定**：M1 期間 `workspace_type: Literal["folder", "topic"]` 必須出現在：
- `ToolContext` pydantic model（`tool-sandbox` capability）
- Sidecar 任何接受 workspace 參數的 request schema（即使 M1 只有 `/healthz`，預留 `POST /scan` request 模型不實作 endpoint）

**為何**：這是不變式；後續 M3 Scanner、M4 Agent 才補 `topic` 分支邏輯，但 schema **必須一開始就有兩個值**。若 M1 只放 `"folder"` 單值、之後改 union，會造成 JSON schema breaking change（`docs/decisions.md` D-002）。

**風險 → 緩解**：M1 tests 必須包含「建立 `workspace_type="topic"` 的 `ToolContext` 不拋例外」單測（即使 tool 行為 stub）→ 防止未來移除欄位。

## Risks / Trade-offs

| Risk | Mitigation |
|---|---|
| Sidecar onefile binary 啟動慢（~1s）影響 first-run UX | Tauri 顯示「啟動中…」loading，後續 ping 快速回應；UX 數字列入 D-008 監控 |
| Mock provider 自動生成 dummy 值無法覆蓋複雜 Pydantic schema | 提供 `MockScript` 讓特定測試指定精確 output；複雜 schema 延到真 provider 接入再驗證 |
| Qdrant Docker 依賴提高新手入門門檻 | `docs/dev-setup.md` 補 standalone binary fallback 說明；未來考慮嵌入式方案另開 change |
| PyInstaller 隱藏 import 漏抓導致打包後 runtime ImportError | M1 驗收條件 = binary 級 smoke（`--healthz`），不接受 source 版過就結案 |
| `ensure_in_workspace` 在罕見 Windows 路徑（junction、reparse point）仍被繞過 | Red team fixture 覆蓋所有 `docs/tool-sandbox.md §十五` 列明的 attack vector；發現漏網回頭 bump spec + 新增 D-XXX |
| Bearer token 在 Tauri ↔ Sidecar 握手間隙外洩（父子 pipe race） | bearer 只在 stdout 首行傳遞、之後即從 process memory 可見性最小化；`--parent-pid` 自殺機制限制 token 生命週期 |
| Worktree 模式下 `.spectra/worktrees/m1-power-on` 分岔 branch 造成 uncommitted 狀態混亂 | 開工前先確認 `spectra list` 狀態；跨 branch 修改須先 stash / commit 再切換 |
| pre-commit stage-0 hook 不夠強，bootstrap 期仍會 commit 到格式不一致的 code | 本 change 只管 gate 通電；真 linter 由各語言 capability 實作時加入、非本 change 責任 |

## Open Questions

- **Q1**：`codebus-sidecar --healthz` 驗收條件是否要包含 Qdrant 連通？若 Qdrant 未起，sidecar 應 `healthz=degraded` 還是 `healthz=error`？
  **暫定**：`degraded`（sidecar 自身健康、外部依賴未就緒），待實作時 confirm。
- **Q2**：`MockScript` fixture 格式 — YAML 還是 Python module？
  **暫定**：Python module（`tests/fixtures/mock_scripts/*.py`），便於 type check；待 task 層再定案。
- **Q3**：PyInstaller onefile 的簽名（Windows SmartScreen / macOS notarization）是否 M1 要處理？
  **暫定**：不處理，屬打磨期；M1 只要 binary 能在本機跑。
