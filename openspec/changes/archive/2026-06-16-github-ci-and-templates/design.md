## Context

複驗結果：

- `.github/` 目前只有 `workflows/` 與 `FUNDING.yml`；`.github/workflows/` 只有 `release-windows.yml`。
- `release-windows.yml` 是 tag `v*` 與 `workflow_dispatch` 觸發的 Windows release pipeline，runner 是 `windows-latest`，toolchain/cache 使用 `dtolnay/rust-toolchain@stable`、`Swatinem/rust-cache@v2`、`actions/setup-node@v4` 的 Node 20 + npm cache，並在 `codebus-app` 跑 `npm ci`。
- `CLAUDE.md` 記錄 clippy 標準是 no new warnings，不是 zero warnings。
- 2026-06-16 本機執行 `cargo clippy --workspace` 回傳 exit code 0，但仍有既有 baseline：`codebus-core` 8 warnings、`codebus-cli` 5 warnings、`codebus-app-tauri` 6 warnings。
- `codebus-app/package.json` scripts：`test` 是 `vitest run`，`typecheck` 是 `tsc --noEmit`，`build` 是 `tsc --noEmit && vite build`。

Windows 相依證據：

- `codebus-core/src/agent/process_kill.rs` 用 Windows Job Object 終止 `.cmd -> node.exe -> codex.exe` 進程樹，測試在 Windows 走 `cmd` / `powershell.exe` grandchild path，在 Unix 走 process group / `killpg`。
- `codebus-core/src/agent/codex_backend.rs` 在 Windows 預設 `codex.cmd`。
- `codebus-app/src-tauri/src/ipc/cli_status.rs` 在 Windows 用 `cmd /C` probe npm-installed CLI shim，並隱藏 GUI app 產生的 console window。
- `codebus-cli/src/commands/hook.rs` 有 Windows path 與 case 處理分支。
- `codebus-cli/tests/config_subcommand.rs` 與 `codebus-app/src-tauri/tests/keyring_ipc.rs` 會碰真實 OS keyring；headless Linux runner 對 keyring backend 的可用性風險比 Windows runner 高。

## Goals / Non-Goals

**Goals:**

- 建立 branch push 與 pull request 觸發的 CI workflow。
- CI 在 Windows runner 上跑 Rust workspace 測試、clippy baseline guard、app npm test/typecheck。
- 保持 toolchain/cache 與現有 release workflow 一致。
- 新增 bug report、feature request、pull request templates。
- 為 CI 行為建立 `ci-automation` capability spec。

**Non-Goals:**

- 不修改 `.github/workflows/release-windows.yml`。
- 不新增 macOS/Linux release、自動更新、簽章、branch protection、Dependabot。
- 不修改 core/cli/app runtime source。
- 不要求本 change 在 propose 階段觸發 GitHub Actions；實際雲端綠燈必須在 apply 後 push 觀察。
- 不讓 issue/PR templates 進入 capability spec；templates 是 repo hygiene artifact，不是產品或 automation 行為契約。

## Decisions

### Runner OS uses windows-latest

CI 採單一 `windows-latest` runner，不做 Ubuntu/Windows matrix。

理由：

- release workflow 已採 `windows-latest`；新的 CI 與 release path 使用相同主要 OS，降低「PR 綠但 release Windows 路徑破」的機率。
- repo 的高風險行為集中在 Windows：Job Object process-tree kill、`codex.cmd`/npm shim、Windows path/case、GUI app 隱藏 console、real OS keyring。
- Ubuntu matrix 會增加時間與維護成本，且 keyring tests 在 headless Linux 的 backend 可用性不穩定；此 change 的目標是建立第一層 push/PR gate，而不是全面跨平台支援。

取捨：單一 Windows runner 不會驗證 Unix `killpg` 分支。這是已知 coverage gap；若未來要承諾 Linux/macOS release 或 support，再用獨立 change 加 matrix 並處理 keyring 測試隔離。

### Clippy uses a baseline guard, not -D warnings

CI 不使用 `cargo clippy --workspace -- -D warnings`。

策略：

- 先跑 `cargo clippy --workspace` 並保留完整輸出。
- 解析 clippy warnings，依 package 檢查 warning count 是否高於既有 baseline：`codebus-core=8`、`codebus-cli=5`、`codebus-app-tauri=6`。
- warning count 等於或低於 baseline 時通過；任何 tracked package 超過 baseline 時 fail。
- 若 clippy 本身 exit code 非 0，CI 直接 fail。

理由：`-D warnings` 會因既有 baseline 直接失敗，違反 repo 現行 bar。baseline guard 才能表達「不新增 warnings」。

### App CI runs npm test and typecheck

CI 在 `codebus-app` 執行 `npm ci`、`npm run test`、`npm run typecheck`。

理由：

- `package.json` 已定義 `test=vitest run`、`typecheck=tsc --noEmit`。
- `npm run build` 內含 typecheck + Vite build；release workflow 已透過 Tauri build path 覆蓋 build/package，push/PR CI 先聚焦 test/typecheck，避免把 release installer build 成本放進每個 PR。

### Toolchain and cache mirror release-windows.yml

CI 沿用 release workflow 的 toolchain/cache pattern：

- `actions/checkout@v4`
- `dtolnay/rust-toolchain@stable`
- `Swatinem/rust-cache@v2` with workspace root
- `actions/setup-node@v4` with Node 20、npm cache、`cache-dependency-path: codebus-app/package-lock.json`
- `npm ci` working-directory `codebus-app`

理由：CI 與 release path 不應使用不同 toolchain 假設；cache 設定也應與既有 release workflow 對齊。

### Spec covers CI automation only

新增 `ci-automation` capability spec；不修改 `release-automation`，也不為 issue/PR templates 建 spec。

理由：

- CI workflow 會改變 repo 的自動化品質閘門，是可測、可回歸的 automation 行為。
- `release-automation` 已描述 tag release installer path，本 change 不改其 requirement。
- issue/PR templates 是作者輸入格式與 reviewer checklist，不是產品功能、runtime contract 或 automation behavior；用 proposal/design/tasks 追蹤即可。

## Implementation Contract

**Behavior:**

- Branch push 與 pull request 會觸發新的 CI workflow。
- CI 在 `windows-latest` 上執行 Rust 與 app 驗證。
- CI 不建立 GitHub Release、不產生 installer、不取代 `release-windows.yml`。
- issue author 會看到 bug report 與 feature request templates。
- PR author 會看到 pull request template。

**Interface / data shape:**

- 新增 `.github/workflows/ci.yml`。
- workflow triggers：branch `push` 與 `pull_request`。
- workflow runner：`windows-latest`。
- Rust commands：`cargo test --workspace`；`cargo clippy --workspace` 加 baseline guard。
- clippy accepted baselines：`codebus-core=8`、`codebus-cli=5`、`codebus-app-tauri=6`。
- App commands in `codebus-app`：`npm ci`、`npm run test`、`npm run typecheck`。
- 新增 `.github/ISSUE_TEMPLATE/bug_report.md`、`.github/ISSUE_TEMPLATE/feature_request.md`、`.github/PULL_REQUEST_TEMPLATE.md`。

**Failure modes:**

- Rust test failure -> CI fail。
- clippy command failure -> CI fail。
- clippy warning count above baseline -> CI fail and identify package。
- npm install/test/typecheck failure -> CI fail。
- GitHub Actions schema invalid -> workflow file rejected by GitHub after apply/push；apply phase must at least review YAML syntax locally.

**Acceptance criteria:**

- `spectra validate github-ci-and-templates` passes before apply.
- apply 後 repo diff 只新增 `.github/workflows/ci.yml`、issue templates、PR template，且不修改 `release-windows.yml`。
- apply 後 local content review confirms workflow uses `windows-latest`, Rust stable/cache, Node 20/npm cache, `cargo test --workspace`, clippy baseline guard, `npm run test`, and `npm run typecheck`.
- apply 後 push/PR on GitHub Actions confirms actual cloud execution.

**Scope boundaries:**

- In scope：GitHub workflow/config/template files under `.github/` and Spectra artifacts under this change。
- Out of scope：product source changes, release workflow changes, signing, auto-update, macOS/Linux release, branch protection, Dependabot。

## Risks / Trade-offs

- [Single Windows runner misses Unix-specific process group behavior] -> Record as an explicit coverage gap; future Linux/macOS support gets a separate matrix change.
- [Clippy count parsing could drift if cargo output changes] -> Prefer structured JSON parsing in workflow rather than only matching human summary lines.
- [Baseline guard allows warning replacement with same count] -> Accepted for this change because repo standard is no new warnings and current baseline is count-based; stricter exact-warning snapshots require a separate baseline file or code cleanup.
- [Real OS keyring tests can be environment-sensitive] -> Use Windows runner first because it matches the product/release target and has Credential Manager available on hosted runners.
