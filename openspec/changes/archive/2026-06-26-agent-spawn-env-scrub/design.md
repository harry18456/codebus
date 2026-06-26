## Context

codebus spawn agent CLI（claude / codex）子程序時，子程序**完整繼承父 shell 的所有環境變數**——全 repo `env_clear` 零命中。父 env 內的機密（`GITHUB_TOKEN`、`AWS_*`、`KUBECONFIG`、各式 `*_TOKEN` / `*_KEY` / `*_SECRET`）連同 codebus 自己注入的 provider key 因此全對 agent 可見。`pii/` scanner 只掃**檔案內容**、不掃 env；在 codex `workspace-write` 沙箱下，agent 驅動的 shell 與 subagent 可直接讀出這些值。`docs/security.md` 目前把這條列為「其他已知 codebus-side 缺口」之一（spawn agent 沒有 env_clear）。本變更就是補上這個破口。

兩條 spawn 路徑與其 provider 注入方式**不對稱**，是設計的關鍵約束：

- **claude**：`agent/claude_backend.rs` 的 `build_command` 委派給 `agent/claude_cli.rs` 的 `compose_claude_cmd`；後者 `Command::new` 之後組 argv，最後以 `cmd.envs(...)` 疊加 `EnvOverrides`（system profile = 空 map；azure profile = 3 個 key）。`EnvOverrides` 是既有的乾淨抽象。
- **codex**：`agent/codex_backend.rs` 的 `build_command` 直接以 `cmd.env(CODEX_AZURE_KEY_ENV, key)` 注入 Azure key——**不走 `EnvOverrides`**。

provider-agnostic 的 `invoke` loop（`agent/claude_cli.rs`，檔名雖如此但 provider 中立）只透過 backend 三方法驅動，不碰 env，因此 env scrub 必須落在各 backend 的 `build_command` 組裝階段。

整合測試以 `codebus-cli/tests/bins/mock_claude.rs`（一支 mock binary，透過 `CODEBUS_CLAUDE_BIN` 取代真 claude）驅動，行為與 log 路徑由 `CODEBUS_MOCK_BEHAVIOR` / `CODEBUS_MOCK_LOG` 等控制變數選定。**這支 mock 是經由 codebus 真實 spawn 路徑啟動的**——也就是即將加上 env_clear 的同一條路徑。

## Goals / Non-Goals

**Goals:**

- 兩個 backend 在 spawn 子程序前 `Command::env_clear()`，僅以**跨平台 allowlist** 重新放行 agent CLI 執行所必需的系統 env；父 env 內的機密不再進入 child。
- claude 與 codex **共用單一 allowlist**（共用 helper），避免兩份名單漂移。
- provider key 仍到位：注入順序 `env_clear → allowlist passthrough → provider overrides`。
- spec 同步：`claude-code-config` 的「Scoped Environment Injection At Spawn」MODIFIED；`codex-backend` 新增「Spawn Environment Scrub」requirement。
- 既有整合測試套件維持綠燈；新增 scrub 斷言（sentinel 機密在 child 缺席、`PATH` / provider key 在 child 存在）涵蓋兩個 backend。

**Non-Goals:**

- SEC-2（codex hard read 隔離）、SEC-4（已完成）——不在本變更。
- **不做 provider key broker**：allowlist 已足以擋住父 env 機密，broker 是 over-engineer。
- **不改父 shell env**：`env_clear` 只作用於 child `Command`，父程序 env 本來就不動、也不該動。
- 不改 `pii/` scanner、不改 toolset / 沙箱 / hook gate。
- 不重構 mock 控制協定為 control-file（見 Decision 8 與 Open Questions——列為可選 follow-up）。

## Decisions

### 共用 allowlist helper 置於 env_overrides.rs（claude / codex 單一名單）

在 `agent/env_overrides.rs` 新增 `passthrough_env()`：讀取父程序 env，回傳通過 allowlist 的 `(name, value)` 配對（保留父端原始大小寫）。兩個 backend 都呼叫同一支，名單只有一份。helper 內部拆成兩層以利測試：純函式 `filter_passthrough(iter)`（吃一個 `(String, String)` 迭代器、回傳過濾後的 `Vec`，無副作用、可用合成輸入決定性單元測試）+ 薄包裝 `passthrough_env()`（把 `std::env::vars()` 餵給它）。allowlist 判定集中在一個述詞函式，避免散落。

**替代方案（不採）**：claude 與 codex 各自維護名單——必然漂移（一邊補了 env、另一邊忘了），正是本變更要避免的。

### 注入順序：env_clear → allowlist passthrough → provider overrides

兩個 backend 都在 `Command::new` 之後、緊接著 `env_clear()` + 套用 `passthrough_env()`；既有的 provider 注入**保留在原位、不前移**（claude 的 `EnvOverrides` 疊加、codex 的 Azure key 注入都在 argv 組裝尾段），因為它們發生在 `env_clear` 之後，自然存活並在任何同名碰撞時覆寫 allowlist 值。provider key 名稱（`ANTHROPIC_API_KEY` / `CODEBUS_CODEX_AZURE_KEY` 等）皆不在 allowlist 內，故實際上無碰撞；順序保證即使未來有碰撞，provider 仍最終勝出。

### 跨平台 allowlist 名單與逐項必要性論證

名單分三組：通用 + `cfg(windows)` + `cfg(unix)`。每一項都是 agent CLI 子程序（及它 shell out 的 git / rg / node）執行所必需或強需，缺漏會導致 spawn 或執行失敗。

**通用（所有平台）**

- `PATH`：解析 agent binary 本身與其 shell out 的工具。缺它 spawn 立即失敗。
- `HOME`：home 目錄解析（Unix 及部分跨平台工具 / node）。非機密。
- `LANG`：locale；缺它部分工具對 UTF-8 路徑 / 輸出產生 mojibake 或直接失敗。
- `LANGUAGE`：GNU gettext 多語 fallback 清單。
- `LC_` 前綴家族（如 `LC_ALL` / `LC_CTYPE`）：細分 locale 類別；全放行，皆 locale、非機密。
- `TZ`：時區，影響時間戳輸出。

**Windows（cfg(windows)）**——codex 在 Windows 是 `codex.cmd` → `node.exe` → 原生 `codex.exe` 鏈，特別依賴系統 env：

- `SystemRoot`：Windows 系統目錄；DLL 搜尋、`cmd.exe` / powershell 定位。node 與 `.cmd` shim 必需。
- `SystemDrive`：系統碟代號。
- `windir`：`SystemRoot` 的舊式別名；部分工具讀它。
- `USERPROFILE`：Windows 使用者家目錄；node / 工具家目錄解析。
- `HOMEDRIVE` / `HOMEPATH`：家目錄的碟與路徑拆分；部分工具組合它們。
- `APPDATA` / `LOCALAPPDATA`：node / npm runtime 路徑；即使帶 `--ignore-user-config`，node 自身仍解析這些。
- `PROGRAMDATA`：全機器共用程式資料。
- `ProgramFiles` / `ProgramFiles(x86)`：已安裝工具定位。（後者名稱含括號，仍是合法 Windows env 名，逐項迭代比對即可。）
- `PATHEXT`：**關鍵**——解析 `.cmd` / `.exe` 等可執行副檔名；缺它 `codex.cmd` 的解析直接失敗，整條 codex 鏈起不來。
- `ComSpec`：`cmd.exe` 路徑；`.cmd` shim 透過它執行。
- `TEMP` / `TMP`：暫存目錄；node / codex scratch，缺它多數工具失敗。
- `NUMBER_OF_PROCESSORS`：node libuv threadpool / 並行度讀取；benign。
- `OS`：值為 `Windows_NT`，部分工具分支判斷。
- `COMPUTERNAME`：偶被讀；低敏感、benign。

**Unix（cfg(unix)）**

- `USER` / `LOGNAME`：身分；部分工具用於 config / temp 路徑。
- `SHELL`：agent 可能透過 `$SHELL` shell out。
- `TMPDIR`：暫存目錄（macOS 尤其）；缺它部分工具失敗。

**刻意排除（且為何安全）**

- `GITHUB_TOKEN` / `GH_TOKEN` / `AWS_*` / `KUBECONFIG` / `DOCKER_*` / `OPENAI_API_KEY` / 父端 `ANTHROPIC_API_KEY` / `SSH_AUTH_SOCK` / 任何 `*_TOKEN` / `*_KEY` / `*_SECRET`：正是本變更要 scrub 的機密。codebus 需要的 provider key 由 provider-override 步驟**明確重新注入**，故排除父端副本安全。
- `NODE_OPTIONS`：可被用來注入 `--require` 任意腳本，是執行注入向量；node 無它照常運作，刻意排除。

### codebus 自身 CODEBUS_* 控制變數不入 allowlist

`CODEBUS_CODEX_BIN` / `CODEBUS_CLAUDE_BIN` / `CODEBUS_HOME` / `CODEBUS_AZURE_KEY` / codex 的 fallback key 來源等，都由**父程序**（codebus 本身）以 `std::env::var` 在 build / spawn 前讀取，`env_clear` 只作用於 child `Command`、不影響父程序的讀取。child agent 無需讀任何 `CODEBUS_*`；provider key 走專用 var（`ANTHROPIC_API_KEY` / `CODEBUS_CODEX_AZURE_KEY`）重新注入。因此 `CODEBUS_*` 不在 allowlist——其中 `CODEBUS_AZURE_KEY` / codex key fallback 本身就是機密，被 scrub 是正確的。（唯一例外見 Decision 8 的測試 seam。）

### 純函式 filter 分離，scrub 以 spawn-based 測試坐實

`Command::get_envs()` 只回傳「明確設定」的 env，無法直接證明 `env_clear` 旗標是否生效（未 clear 但同樣明確 set `PATH` 也會讓 `get_envs()` 含 `PATH`）。因此 scrub 的**端到端證據必須是 spawn-based**：在父端注入一個 sentinel 機密、spawn 一支會 dump 自身 env 的 child、斷言 child 看不到 sentinel、看得到 `PATH` 與 provider key。

三層測試：
1. **純函式單元測試** `filter_passthrough`：餵合成輸入（含 `GITHUB_TOKEN`、`CODEBUS_AZURE_KEY`、`PATH`、`ProgramFiles(x86)`、`LC_CTYPE`、一個非名單變數），斷言輸出**恰好**是 allowlist 成員、排除機密。決定性、無 global env 競態、跨平台以 cfg 斷言。
2. **build_command 接線單元測試**（兩 backend）：以 `get_envs()` 斷言 passthrough 已套用（`PATH` 明確在列）+ azure provider key 在列。
3. **spawn-based scrub 整合測試**（兩 backend，見 Implementation Contract）：真正坐實 child 看不到 sentinel。

### spec delta：claude-code-config MODIFIED + codex-backend ADDED

`claude-code-config` 的「Scoped Environment Injection At Spawn」是 claude 專屬 requirement（明文 `claude_cli::invoke` / `claude` child），MODIFIED 之；codex 的 env 行為目前**任何 spec 都未規範**，故在 `codex-backend`（已承載 codex 的 isolation recipe，同性質）**新增**「Spawn Environment Scrub」requirement。

**閉集陷阱處理**：allowlist 是一個閉集，但**完整名單只放在 design.md 與實作**，spec 以「契約」描述（drop 任意父端機密、放行一組 platform-aware 的系統必需 env、`PATH` 為其成員、provider 注入疊加其上），scenario 斷言**行為**（sentinel 缺席 / `PATH` 與 provider key 在場），不逐一列舉名單成員。如此未來增刪某個 passthrough 成員不需改 spec，也避免「多個 scenario 各列一份名單」的同步地獄。MODIFIED 時需 grep 該 spec 所有列舉「inherit / parent env / injects no env」的 scenario 全部同步——目前命中的是「System profile injects no env」一條。

### 測試 mock 控制變數的遞送（CODEBUS_MOCK_ 前綴 passthrough）

**衝擊**：mock binary 經 codebus 真實 spawn 路徑啟動，靠繼承父端設定的 `CODEBUS_MOCK_BEHAVIOR` / `CODEBUS_MOCK_LOG` / `CODEBUS_MOCK_SESSION_ID` 運作。env_clear 會把這些一起 scrub，導致約 10 個整合測試檔（goal / quiz / chat / fix / query / parse-error / ... flow）的 mock 退回 default 行為、不寫 log 而失敗。

**決定**：allowlist 額外放行 `CODEBUS_MOCK_` **前綴家族**，並在 helper 內明確註解其為「整合測試控制變數，由 `codebus-cli/tests/bins/mock_claude.rs` 讀取；該 mock 經本 scrub 路徑啟動，故須以此 seam 遞送控制；此前綴可證不攜機密、production 部署不會設定」。注意 seam **只**放行 `CODEBUS_MOCK_` 前綴——codebus 真正的機密控制變數（`CODEBUS_AZURE_KEY` / codex key fallback）名稱不符此前綴，**仍被 scrub**，安全屬性完整保留。

**替代方案（記錄、不在本變更採）**：把 mock 控制改由 child 工作目錄的 control-file 遞送（cwd 經 `Command::current_dir` 設定、與 env_clear 正交、必然存活），可讓 production allowlist 完全不含任何測試字樣。但需重構 mock 控制協定 + 約 10 個整合測試檔，對一個安全變更而言 churn / 風險不成比例。列為 Open Question 供 apply 時定奪。

## Implementation Contract

**Behavior（ship 後可觀察）**：codebus spawn 的 claude / codex 子程序，其環境**僅含** allowlist 放行的系統 env + codebus 注入的 provider env（claude azure = 3 個 key；codex azure = `CODEBUS_CODEX_AZURE_KEY`；system profile = 0 個 provider env）。父 shell 內任何不在 allowlist 的變數（含機密）對子程序**不可見**。父程序自身 env 不被修改。

**Interface / data shape**：
- `agent/env_overrides.rs` 新增 `passthrough_env() -> Vec<(String, String)>`（公開於 crate 內供兩 backend）+ 內部純函式 `filter_passthrough`。
- claude：`compose_claude_cmd` 在 `Command::new` 後立即 `cmd.env_clear()` 並 `cmd.envs(passthrough_env())`；既有 `EnvOverrides` 疊加保留於原尾段。
- codex：`build_command` 在 `Command::new` 後立即 `cmd.env_clear()` 並 `cmd.envs(passthrough_env())`；既有 Azure key 注入保留。
- 注入順序固定 `env_clear → passthrough → provider`。

**Failure modes**：
- allowlist 漏列某 agent 必需系統 env → 子程序 spawn 或執行失敗（最高風險，見 Risks）。
- allowlist 成員在父端不存在 → 該成員不注入（best-effort per-member，不注入空值）。

**Acceptance criteria**：
- `filter_passthrough` 純函式測試：合成輸入 → 輸出恰為 allowlist 成員、排除 `GITHUB_TOKEN` / `CODEBUS_AZURE_KEY` 等機密（platform-aware，cfg 斷言）。
- 兩 backend `build_command` 接線測試：`get_envs()` 含 `PATH` + azure provider key。
- claude spawn-based 測試（強化 `codebus-cli/tests/scoped_env_injection.rs`）：父端注入 sentinel 機密 → 經 mock 啟動 → mock log 顯示 sentinel 缺席、`PATH` 在場；azure 變體 3 個 key 在場、system 變體注入的 api-key 值缺席。
- codex spawn-based 測試（新增）：`CodexBackend` azure `build_command` → 以 mock 當 env dumper 直接 spawn（繞過 codex stream 解析）→ 斷言 sentinel 缺席、`PATH` 在場、`CODEBUS_CODEX_AZURE_KEY` 在場。
- mock binary 擴充 env dump 清單以涵蓋 sentinel 名、`PATH`、`CODEBUS_CODEX_AZURE_KEY`（既有 3 個 anthropic var dump 保留，additive、不破壞既有斷言）。
- 既有整合測試套件（goal / quiz / chat / fix / query / parse-error flow）維持綠燈（`CODEBUS_MOCK_` 前綴 passthrough 保證）。
- **實機驗證（apply 後必做，本變更最大風險）**：真實 claude 與真實 codex 各跑一次 goal 或 query，確認不因缺系統 env 而 spawn 失敗。

**Scope boundaries**：
- In scope：兩 backend 的 env_clear + passthrough；共用 helper；`claude-code-config` MODIFIED + `codex-backend` ADDED spec；測試強化 + mock dump 擴充 + `CODEBUS_MOCK_` seam；`docs/security.md` 同步。
- Out of scope：SEC-2 / SEC-4；provider key broker；改父 shell env；改 PII scanner / toolset / 沙箱；mock 控制改 control-file。

## Risks / Trade-offs

- **[allowlist 漏列 agent 必需系統 env → 子程序 spawn 失敗]** → 逐項必要性論證（見 Decision 3）已盡量完整；apply 後**強制**以真實 claude + 真實 codex 實機驗證。unit test 不一定抓得到此類漏列（測試程序本身帶著這些 env），唯有實機（尤其 Windows codex `.cmd` → node → codex.exe 鏈，依賴 `PATH` / `PATHEXT` / `SystemRoot` / `ComSpec` / `TEMP`）會暴露。
- **[Windows env 名大小寫不敏感，父端可能是 `Path` / `Tmp` 等任意大小寫]** → allowlist 比對在 `cfg(windows)` 走大小寫不敏感（以大寫正規化比對）、Unix 走大小寫敏感；放行時以父端**原始**名稱+值重新注入。
- **[測試 seam 碰觸安全 allowlist]** → 限縮為 `CODEBUS_MOCK_` 單一前綴；codebus 真正機密控制變數（`CODEBUS_AZURE_KEY` 等）名稱不符，仍被 scrub；helper 內以註解誠實標示來由。production 不會設定此前綴，無實際機密外洩風險。
- **[未來新增工具需要新系統 env]** → 在 allowlist 加一行即可；spec 以契約描述、不列舉名單，故無需改 spec。

## Migration Plan

- 純 spawn-time 行為變更：**不 materialize 任何檔案**、不改 `~/.codebus/config.yaml`、不改 vault `.codebus/.claude/settings.json`。既有 vault 無需升級、不受影響。
- 部署：隨 codebus binary 一併生效。
- Rollback：revert 本變更 commit 即回到「子程序繼承父 env」舊行為，無資料遷移、無殘留狀態。

## Open Questions

- 測試 mock 控制變數的遞送採 `CODEBUS_MOCK_` 前綴 passthrough（本設計推薦，低 churn、安全屬性完整）抑或 control-file-in-cwd（production allowlist 完全 pristine，但 ~10 檔測試 churn）。apply 時可改採後者；若改採，scope 擴及 mock 控制協定 + 各 flow 測試檔。
