## 1. 共用 allowlist helper 與名單（env_overrides.rs）

- [x] 1.1 依 design「共用 allowlist helper 置於 env_overrides.rs（claude / codex 單一名單）」「純函式 filter 分離，scrub 以 spawn-based 測試坐實」，在 `agent/env_overrides.rs` 實作純函式 `filter_passthrough(iter)` + 薄包裝 `passthrough_env()`：依「跨平台 allowlist 名單與逐項必要性論證」放行通用 + `cfg(windows)` + `cfg(unix)` 系統 env（含 `PATH`），依「Windows 大小寫不敏感比對與 LC_ 前綴家族」做比對與前綴家族放行，依「codebus 自身 CODEBUS_* 控制變數不入 allowlist」排除 `CODEBUS_AZURE_KEY` 等機密。Behavior：給定父 env 迭代器，回傳恰為 allowlist 成員的 `(name, value)`、保留原始大小寫、機密缺席。驗證：新增 `filter_passthrough` 純函式單元測試（合成輸入含 `GITHUB_TOKEN` / `CODEBUS_AZURE_KEY` / `PATH` / `ProgramFiles(x86)` / `LC_CTYPE` / 一個非名單變數），platform-aware cfg 斷言輸出恰為 allowlist 成員；`cargo test -p codebus-core` 綠。
- [x] 1.2 依 design「測試 mock 控制變數的遞送（CODEBUS_MOCK_ 前綴 passthrough）」，在 helper 放行 `CODEBUS_MOCK_` 前綴並以註解標示其為整合測試控制變數、不攜機密、production 不設定。Behavior：`CODEBUS_MOCK_LOG` 通過 passthrough，而 `CODEBUS_AZURE_KEY` 仍被 scrub（前綴不符）。驗證：`filter_passthrough` 測試新增「`CODEBUS_MOCK_LOG` 放行、`CODEBUS_AZURE_KEY` 排除」兩斷言；`cargo test -p codebus-core` 綠。
- [x] 1.3 更新 `agent/env_overrides.rs` module doc 與 `EnvOverrides::for_system` doc，移除「inherits the parent env unchanged / inherits the parent shell verbatim」表述，改述為「spawn 路徑 `env_clear` + allowlist passthrough，再疊加 profile 注入」。Behavior：doc 與新 spawn 行為一致、不再宣稱逐字繼承。驗證：人工 review doc 文字；既有 `for_system_returns_empty_map` 等單元測試仍綠（`EnvOverrides` 形狀未變），`cargo test -p codebus-core` 綠。

## 2. 兩 backend spawn 路徑加 env scrub

- [x] 2.1 [P] 依 design「注入順序：env_clear → allowlist passthrough → provider overrides」，在 `agent/claude_cli.rs` 的 `compose_claude_cmd` 於 `Command::new` 後立即 `cmd.env_clear()` + `cmd.envs(passthrough_env())`，既有 `EnvOverrides` 疊加保留於尾段（在 clear 之後存活）。Behavior：滿足 `claude-code-config` 的 **Scoped Environment Injection At Spawn** requirement——claude 子程序僅含 allowlist + provider env，父端非名單機密缺席。驗證：在 `claude_backend.rs` 新增 `build_command` 接線單元測試，以 `cmd.get_envs()` 斷言含 `PATH` 且 azure 變體含 `ANTHROPIC_API_KEY`；`cargo test -p codebus-core` 綠。
- [x] 2.2 [P] 依 design「注入順序：env_clear → allowlist passthrough → provider overrides」，在 `agent/codex_backend.rs` 的 `CodexBackend::build_command` 於 `Command::new` 後立即 `cmd.env_clear()` + `cmd.envs(passthrough_env())`，既有 Azure key 注入（`CODEBUS_CODEX_AZURE_KEY`）保留於 azure 分支（在 clear 之後存活）。Behavior：滿足 `codex-backend` 的 **Spawn Environment Scrub** requirement——codex 子程序僅含 allowlist + azure key（azure 時），父端非名單機密缺席。驗證：在 `codex_backend.rs` 新增 `build_command` 接線單元測試，以 `cmd.get_envs()` 斷言含 `PATH` 且 azure 變體含 `CODEBUS_CODEX_AZURE_KEY`；`cargo test -p codebus-core` 綠。

## 3. spawn-based scrub 整合測試 + mock 擴充

- [x] 3.1 擴充 `codebus-cli/tests/bins/mock_claude.rs` 的 env dump 清單，additive 加入一個固定 sentinel 名（`CODEBUS_SCRUB_SENTINEL`）、`PATH`、`CODEBUS_CODEX_AZURE_KEY`，保留既有三個 anthropic var dump。Behavior：mock log 可觀察這些變數是否抵達 child，供 scrub 斷言使用。驗證：既有整合測試套件不破（純 additive dump），`cargo test -p codebus-cli` 綠。
- [x] 3.2 強化 `codebus-cli/tests/scoped_env_injection.rs` 既有兩個 claude 測試為 scrub 斷言（驗證 **Scoped Environment Injection At Spawn**）：`for_system_does_not_inject_env` 改為父端注入 `CODEBUS_SCRUB_SENTINEL` 機密→經 mock→斷言 child log 中 sentinel 缺席、`PATH` 在場，並刪除舊「Parent shell env may still legitimately leak them in ... expected inheritance behavior」誤導註解；`invoke_passes_env_overrides_to_command` 加上 sentinel 缺席 + `PATH` 在場斷言。Behavior：claude 子程序確實看不到父端機密、看得到系統必需 env。驗證：`cargo test -p codebus-cli --test scoped_env_injection` 綠。
- [x] 3.3 在 `codebus-cli/tests/scoped_env_injection.rs` 新增 codex spawn-based scrub 測試（驗證 **Spawn Environment Scrub**）：以 `CODEBUS_CODEX_BIN` 指向 mock 當 env dumper、父端設 `CODEBUS_SCRUB_SENTINEL`，呼叫 `CodexBackend`（azure）`build_command` 後直接 `.output()`（繞過 codex stream 解析），斷言 child log 中 sentinel 缺席、`PATH` 在場、`CODEBUS_CODEX_AZURE_KEY` 在場。Behavior：codex 子程序 scrub 機制端到端成立、azure key 存活。驗證：`cargo test -p codebus-cli --test scoped_env_injection` 新測試綠。

## 4. spec 對外文件同步 + 全套驗證

- [x] 4.1 [P] 依 design「spec delta：claude-code-config MODIFIED + codex-backend ADDED」同步 `docs/security.md`：將「其他已知 codebus-side 缺口」清單中「spawn agent 沒有 `env_clear`」一條，改述為「已補：spawn 路徑 `env_clear` + 跨平台 allowlist passthrough，父 shell 機密（`GITHUB_TOKEN`/`AWS_*`/`KUBECONFIG`）不再進 agent child env」。Behavior：security.md 不再把此列為未補缺口、與新行為一致。驗證：人工 review 該段；`grep` 確認舊「沒有 `env_clear`」表述已移除/改寫。
- [x] 4.2 全套測試 + clippy 綠：`cargo test -p codebus-core`、`cargo test -p codebus-cli`、`cargo clippy --workspace`（無新 warning）。Behavior：既有 flow 整合測試（goal/quiz/chat/fix/query/parse-error）因 `CODEBUS_MOCK_` 前綴 passthrough 維持綠、scrub 新測試綠。驗證：三命令皆綠、無新 clippy warning。

## 5. 實機驗證（apply 後，本變更最大風險）

- [ ] 5.1 [P] 依 design「跨平台 allowlist 名單與逐項必要性論證」的完整性風險，以真實 claude 跑一次 `codebus goal` 或 `codebus query`（system 或 azure profile），確認不因缺系統 env 而 spawn 失敗、run 正常完成。驗證：實機觀察 run 成功 + RunLog 正常寫入。
- [ ] 5.2 [P] 以真實 codex 跑一次 `codebus goal` 或 `codebus query`，確認 Windows `codex.cmd` → `node.exe` → `codex.exe` 鏈在 scrub 後仍以 `PATH` / `PATHEXT` / `SystemRoot` / `ComSpec` / `TEMP` 正常啟動、run 完成。驗證：實機觀察 run 成功。
