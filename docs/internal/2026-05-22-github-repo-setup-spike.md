# T4 Spike：GitHub 倉庫設定（CI / Release / templates）

**Date:** 2026-05-22
**Task:** loop T4（只讀探勘；草擬 YAML 寫在本 doc，不建 `.github/`）
**背景:** [github-repo-setup backlog](2026-05-14-github-repo-setup-backlog.md)（2026-05-14，parked）

---

## TL;DR — 三處過時 + 一個可立即做、一個仍卡

對現碼核對，2026-05-14 backlog 有 3 處 drift：

1. **套件管理器是 npm，不是 pnpm。** repo 有 `codebus-app/package-lock.json`，無 `pnpm-lock.yaml`。backlog YAML 的 `pnpm --filter` / `pnpm tauri build` 全部要改成 npm。
2. **Release workflow 仍卡。** 依賴的「F」(`v3-app-polish-ship`/穩定 `tauri build`)**未 archive**（archive 只有 `v3-app-foundation`、`v3-render-polish`），且 `tauri.conf.json:31` `bundle.active=false`——installers 根本還沒啟用，release workflow 無法有意義地寫。
3. **CI 的 Linux 陷阱**：workspace members 含 `codebus-app/src-tauri`（`Cargo.toml:2`），所以 `cargo test --workspace` 會在 Linux/macOS runner 編 tauri crate → **需先裝 webkit2gtk 等系統依賴**，否則 CI 紅。backlog 的 YAML 沒處理這點。

**結論**：**CI（test/typecheck）+ issue/PR templates 可立即做**（已草擬於下）；**release workflow 待 F + `bundle.active=true` 後再寫**。

---

## 對現碼核對

- 套件管理：npm（`codebus-app/package-lock.json`）。app 測試 = `npm test` → `vitest run`（`package.json:12`）；typecheck = `npm run build`（`tsc --noEmit && vite build`，`:8`）或 `npx tsc --noEmit`。
- Rust：workspace `version=3.0.0`、`rust-version=1.85`（`Cargo.toml:6,11`），members = core / cli / `codebus-app/src-tauri`。
- 打包：`tauri.conf.json:31` `bundle.active=false` → 尚無 installer 產出。
- `.github/` 目前**完全不存在**——全新建。

## 草擬 1：CI workflow（可立即落地，已修正 npm + tauri 系統依賴）

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  rust-core-cli:
    # core + cli 無 GUI 依賴，跑三平台最划算
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { toolchain: "1.85" }
      - uses: Swatinem/rust-cache@v2
      - run: cargo test -p codebus-core -p codebus-cli
      - run: cargo fmt --all --check
      - run: cargo clippy -p codebus-core -p codebus-cli -- -D warnings

  app:
    # 前端單元測試 + typecheck（不需編 tauri crate）
    runs-on: ubuntu-latest
    defaults: { run: { working-directory: codebus-app } }
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: "20", cache: "npm", cache-dependency-path: codebus-app/package-lock.json }
      - run: npm ci
      - run: npx tsc --noEmit
      - run: npm test

  tauri-build-check:
    # 確認 src-tauri 能編（含 Linux 系統依賴）；不產 installer
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev
      - uses: dtolnay/rust-toolchain@stable
        with: { toolchain: "1.85" }
      - uses: Swatinem/rust-cache@v2
      - run: cargo check -p codebus-app   # crate 名以 src-tauri/Cargo.toml 為準，落地時核對
```

> 落地注意：`cargo check -p codebus-app` 的 package 名要以 `codebus-app/src-tauri/Cargo.toml` 的 `[package].name` 為準（可能不叫 `codebus-app`）——本 spike 未開該檔，落地前核對。Windows/macOS 的 tauri 編譯系統依賴各異，故 tauri-build-check 先只掛 Linux；要不要擴三平台視成本決定。

## 草擬 2：Issue / PR templates（可立即落地）

```yaml
# .github/ISSUE_TEMPLATE/bug_report.yml
name: Bug report
description: 回報問題
body:
  - type: input
    attributes: { label: codebus 版本 (e.g. 3.0.0) }
    validations: { required: true }
  - type: dropdown
    attributes: { label: OS, options: [Windows, macOS, Linux] }
  - type: textarea
    attributes: { label: 重現步驟 }
    validations: { required: true }
  - type: textarea
    attributes: { label: 預期 vs 實際行為 }
```
```yaml
# .github/ISSUE_TEMPLATE/feature_request.yml
name: Feature request
description: 功能提案
body:
  - type: textarea
    attributes: { label: 問題 / 動機 }
    validations: { required: true }
  - type: textarea
    attributes: { label: 提案方案 }
  - type: textarea
    attributes: { label: 替代方案 }
```
```markdown
<!-- .github/pull_request_template.md -->
## Changes
## How to test
## Related issue
```

## 仍卡 / 待 F：Release workflow

`bundle.active=false` + F 未 archive → 暫不寫實質內容。落地前置：
1. F 把 `tauri build` 設定 + `bundle.active=true` + targets（msi/dmg/AppImage）穩定下來。
2. 之後 release.yml 用 `tauri-apps/tauri-action`（自動 build + attach installer 到 Release），三平台 matrix，`on: push: tags: ["v*"]`。

## Branch protection / badge（非 repo 檔案）
- branch protection：GitHub Settings 手動，視 solo/team 決定（無法進 repo 檔）。
- README badge：CI workflow 落地後加 status + latest-release badge。

## 工程量
- CI + templates：**輕（半天）**，現在就能做、低風險。
- Release workflow：**中**，卡 F；F 後約半天-1 個半天。

## 待 harry
1. 要先只上 **CI + templates**（不卡 F、立即有自動 test）嗎？建議 yes。
2. repo 要公開還私有？私有也值得 CI；release workflow 等公開前 + F 完成再做。
