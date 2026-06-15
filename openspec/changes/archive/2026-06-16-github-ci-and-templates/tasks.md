## 1. CI workflow

- [x] 1.1 建立 Branch and pull request CI workflow，並落實 Runner OS uses windows-latest：新增 `.github/workflows/ci.yml`，使 branch push 與 pull_request 觸發 Windows CI，且不觸發 release publish；驗證方式為檢查 workflow triggers、`runs-on: windows-latest`、無 release action/contents write publish path。
- [x] 1.2 落實 Toolchain and cache mirror release-windows.yml：CI 使用 `actions/checkout@v4`、`dtolnay/rust-toolchain@stable`、`Swatinem/rust-cache@v2`、Node 20、npm cache 與 `codebus-app/package-lock.json` cache key，並在 `codebus-app` 執行 `npm ci`；驗證方式為逐項對照 `.github/workflows/release-windows.yml` 的 toolchain/cache pattern。
- [x] 1.3 實作 Rust workspace validation with clippy baseline guard，並落實 Clippy uses a baseline guard, not -D warnings：CI 執行 `cargo test --workspace` 與 `cargo clippy --workspace`，clippy 不使用 `-D warnings`，且 warning count 高於 `codebus-core=8`、`codebus-cli=5`、`codebus-app-tauri=6` 時 fail；驗證方式為檢查 workflow script 的 baseline map、clippy command、fail condition 與完整 log 保留。
- [x] 1.4 實作 codebus-app npm validation，並落實 App CI runs npm test and typecheck：CI 在 `codebus-app` 執行 `npm run test` 與 `npm run typecheck`，任一命令非零即 fail；驗證方式為對照 `codebus-app/package.json` scripts 中 `test=vitest run` 與 `typecheck=tsc --noEmit`。

## 2. GitHub templates

- [x] [P] 2.1 新增 bug report issue template：`.github/ISSUE_TEMPLATE/bug_report.md` 必須收集摘要、重現步驟、預期/實際行為、環境、log 或截圖、Spectra/change 關聯；驗證方式為人工 content review 確認每個欄位存在且沒有 placeholder。
- [x] [P] 2.2 新增 feature request issue template：`.github/ISSUE_TEMPLATE/feature_request.md` 必須收集問題背景、目標行為、scope、替代方案、驗證或 acceptance signal；驗證方式為人工 content review 確認欄位能支撐 proposal 前置討論且沒有 placeholder。
- [x] [P] 2.3 新增 pull request template：`.github/PULL_REQUEST_TEMPLATE.md` 必須要求 PR 作者填寫 summary、Spectra change/spec 狀態、驗證命令、風險/rollback、release notes；驗證方式為人工 content review 確認 reviewer 可直接判斷 scope 與驗證狀態。

## 3. Validation

- [x] 3.1 驗證 change scope：確認 diff 只新增 `.github/workflows/ci.yml`、`.github/ISSUE_TEMPLATE/bug_report.md`、`.github/ISSUE_TEMPLATE/feature_request.md`、`.github/PULL_REQUEST_TEMPLATE.md`，且 `.github/workflows/release-windows.yml` 無修改；驗證方式為檢查 `git diff -- .github openspec/changes/github-ci-and-templates`。
- [x] 3.2 驗證 Spectra artifacts，並落實 Spec covers CI automation only：執行 `spectra validate github-ci-and-templates`，確認 proposal、design、tasks、`ci-automation` spec delta 一致，且 issue/PR templates 不新增 capability spec；驗證方式為命令 exit code 0。
