# Backlog: GitHub 倉庫設定（Actions CI + Release workflow + Issue templates）

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** release readiness
**Owner:** harry
**Status:** parked

---

## 觀察

codebus 目前是私有 / 個人開發倉庫，缺乏：

1. **CI 自動化**：PR / push 無自動 build + test
2. **Release workflow**：沒有 tag-triggered multi-platform binary build
3. **協作基礎設施**：無 issue template、PR template、branch protection

F `v3-app-polish-ship` 會處理 Tauri 的 release build 與 installer，但那是 app 打包機制（`tauri build`）。
本條是 **GitHub 倉庫層的設定**——兩者需要配合但概念不同。

## Proposed fix

新提一條 change：`v3-github-repo-setup`（after F）

### 1. GitHub Actions CI

```yaml
# .github/workflows/ci.yml
on: [push, pull_request]
jobs:
  test:
    strategy:
      matrix:
        os: [windows-latest, macos-latest, ubuntu-latest]
    steps:
      - cargo test --workspace
      - pnpm --filter codebus-app test
      - pnpm --filter codebus-app exec tsc --noEmit
```

- 三平台 matrix build（與 F 的跨平台驗收一致）
- Rust + Node.js 雙 toolchain cache

### 2. Release workflow

```yaml
# .github/workflows/release.yml
on:
  push:
    tags: ["v*"]
jobs:
  build:
    strategy:
      matrix:
        os: [windows-latest, macos-latest, ubuntu-latest]
    steps:
      - pnpm tauri build
      - upload installer to GitHub Release
```

- 對應 F 的 Tauri build 設定
- 自動 attach `.msi` / `.dmg` / `.AppImage` 到 Release
- Release notes 從 git tag message 取（或 CHANGELOG.md）

### 3. Issue templates

```
.github/ISSUE_TEMPLATE/
  bug_report.yml
  feature_request.yml
```

- Bug report：版本、OS、重現步驟、預期 / 實際行為
- Feature request：問題描述、提案方案、替代方案

### 4. PR template

```markdown
## Changes
## How to test
## Related issue
```

### 5. Branch protection

- `main`：require PR + 1 review（或 CI pass）
- 視 solo / team 開發決定是否啟用

### Tasks（粗估）

1. `.github/workflows/ci.yml`（三平台 test）
2. `.github/workflows/release.yml`（tag-triggered build）
3. Issue templates（bug / feature）
4. PR template
5. Branch protection rules（GitHub Settings 手動設定）
6. README badge（CI status、latest release）

工程量：輕-中（1-2 個半天；release workflow 的 Tauri cross-compile 設定是最複雜的部分）。

## Out of scope

- Dependabot / Renovate 自動更新依賴（可後續加）
- Code scanning / security alerts（GitHub 預設即可）
- Wiki / GitHub Pages（不用 GitHub Wiki，codebus 有自己的）

## 依賴

- **after F**：Tauri release build 指令穩定後才能寫 release workflow
- Release workflow 直接呼叫 F 建立的 `tauri build` 設定

## 何時動

F archive 之後，作為 open-source / public release 前的最後準備。
若計畫保持 private repo，CI workflow 仍值得做（自動 test）；release workflow 可延後。
