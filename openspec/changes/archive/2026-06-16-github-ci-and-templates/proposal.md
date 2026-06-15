## Why

codebus 目前只有 tag 觸發的 Windows release workflow，缺少 push/PR 層級的品質閘門；Rust、Tauri backend、frontend test/typecheck 的 regressions 會延後到 release 才暴露。repo 也缺少 issue 與 PR templates，導致回報內容與 reviewer checklist 沒有一致入口。

## What Changes

- 新增 push/PR 觸發的 GitHub Actions CI workflow，覆蓋 Rust 測試、cargo clippy baseline 檢查、codebus-app 的 npm test 與 typecheck。
- 新增 .github/ISSUE_TEMPLATE/bug_report.md，收集可重現 bug report 所需資訊。
- 新增 .github/ISSUE_TEMPLATE/feature_request.md，收集功能需求、使用情境與 scope。
- 新增 .github/PULL_REQUEST_TEMPLATE.md，讓 PR 作者明確描述變更、驗證、spec/Spectra 狀態與風險。
- 不修改既有 release-windows.yml；release automation 維持由 release-automation spec 覆蓋。

## Capabilities

### New Capabilities

- `ci-automation`: 定義 codebus push/PR CI 的觸發條件、runner/toolchain、Rust 與 app 驗證命令、以及 clippy baseline 策略。

### Modified Capabilities

(none)

## Impact

- Affected specs: new `ci-automation`
- Affected code:
  - New:
    - .github/workflows/ci.yml
    - .github/ISSUE_TEMPLATE/bug_report.md
    - .github/ISSUE_TEMPLATE/feature_request.md
    - .github/PULL_REQUEST_TEMPLATE.md
  - Modified: (none)
  - Removed: (none)
- Affected systems: GitHub Actions checks for branch pushes and pull requests; GitHub issue/PR authoring UI through repository templates.
