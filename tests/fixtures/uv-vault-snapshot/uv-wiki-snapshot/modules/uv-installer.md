---
title: uv-installer
type: module
sources:
  - path: crates/uv-installer/Cargo.toml
    sha256: e38dc767b6820f562ce7354b6bcf97ee523576dd92999b32fa5f35381606e542
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-installer/src/lib.rs
    sha256: f0fbe3e08e8eb49a2b8dd0fec5fc498f22cb4845694eed757a65e13608323f19
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-installer/src/plan.rs
    sha256: a8caa2fe042270aac5b8fd3e5f1031520ac69732b8d6422abded50ff40d95b10
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-installer/src/preparer.rs
    sha256: 8daffc323f8d3b626abe2e0af3143761c7b0f55f5b0fbf76950e560ae1ddbf12
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-installer/src/installer.rs
    sha256: cf643256209a0b327cb146d369ab0fd0e60431f525e74f29d0198af6c3a85635
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-installer 怎麼用 cache 裝 wheel
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[wheel-install-pipeline]]'
  - '[[uv-cache-key]]'
  - '[[uv-cache-info]]'
  - '[[cache-info-vs-cache-key]]'
stale: false
---

# uv-installer

uv 的 **安裝端 crate**：拿 [[uv-resolver|resolver]] 解出來的 `Resolution`，比對 cache 與目前 venv 的 site-packages，最後把 wheel 連結到 venv 裡面。它的核心觀點是：

> **所有 wheel 最終都從本地 cache 裝**。網路下載只是「把還沒進 cache 的 wheel 帶進 cache」這個前置步驟。

也就是說，uv-installer 不直接「下載完就裝」，而是把流程切成 **Plan → Prepare → Install** 三段，每段對 cache 的角色不同。詳細端到端流程見 [[wheel-install-pipeline]]。

## 三段公開 API

```rust
// from crates/uv-installer/src/lib.rs
pub use compile::{CompileError, compile_tree};
pub use installer::{Installer, Reporter as InstallReporter};
pub use plan::{Plan, Planner};
pub use preparer::{Error as PrepareError, Preparer, Reporter as PrepareReporter};
pub use site_packages::{
    InstallationStrategy, SatisfiesResult, SitePackages, SitePackagesDiagnostic,
};
pub use uninstall::{UninstallError, uninstall};
```

對應的三段流程：

| 階段 | 主要型別 | 輸入 | 輸出 | 跟 cache 的關係 |
|------|---------|------|------|----------------|
| **Plan** | `Planner` → `Plan` | `Resolution` + `SitePackages` | 4 個 `Vec`：`cached` / `remote` / `reinstalls` / `extraneous` | **查 cache**：能命中就放 `cached`，否則放 `remote` |
| **Prepare** | `Preparer` | `Vec<Arc<Dist>>` (= plan.remote) | `Vec<CachedDist>` | **填 cache**：呼叫 `DistributionDatabase::get_or_build_wheel`，下載/建構結果寫入 cache 後變 `CachedDist` |
| **Install** | `Installer` | `Vec<CachedDist>` (cached + 剛 prepare 完的) | venv 裡裝好的套件 | **從 cache link**：用 `LinkMode` 連結 `wheel.path()`（在 cache 中）到 venv 的 site-packages |

外加兩個橫向工具：

- `SitePackages` (`site_packages.rs`) — 掃描 venv 現有套件，提供 `remove_packages()` 給 `Planner` 配對 reinstall。
- `compile_tree`、`uninstall` — 安裝後 `.pyc` 編譯與卸載，不在 cache install 主流程上。

## 為什麼要分三段，而不是一次做完？

把 cache 查找跟下載/建構分開，可以拿到三個明顯好處：

1. **批次規劃可以先決定哪些根本不必下載**。`Planner::build()` 走完一輪就能告訴呼叫端「這次安裝要碰網路嗎？要 rebuild 嗎？」(`Plan::is_empty()`) 進一步可以早退。
2. **`Preparer` 可以用 `FuturesUnordered` 並行下載**，且呼叫端在等下載時 venv 仍是乾淨的，沒有半安裝狀態。
3. **`Installer` 可以用 rayon 並行 link**：拿到的全是 `CachedDist`（指向 cache 的路徑），rayon 把 `install_wheel` 平行展開，不需要任何 IO 等待之外的同步點。

## `Plan` 的 4 個桶子

```rust
// from crates/uv-installer/src/plan.rs
pub struct Plan {
    /// The distributions that are not already installed in the current environment, but are
    /// available in the local cache.
    pub cached: Vec<CachedDist>,

    /// The distributions that are not already installed in the current environment, and are
    /// not available in the local cache.
    pub remote: Vec<Arc<Dist>>,

    /// Any distributions that are already installed in the current environment, but will be
    /// re-installed (including upgraded) to satisfy the requirements.
    pub reinstalls: Vec<InstalledDist>,

    /// Any distributions that are already installed in the current environment, and are
    /// _not_ necessary to satisfy the requirements.
    pub extraneous: Vec<InstalledDist>,
}
```

兩個維度組合：

- **是否已安裝**：`reinstalls` + `extraneous` 是當前 venv 已存在的；其他兩個不是。
- **是否在 cache 中**：`cached` 是；`remote` 不是。

`reinstalls` 跟 `extraneous` 的差異：前者要被換成新版本，後者直接不在 resolution 裡（unneeded）。`extraneous` 受到 _seed package_ 保護，預設保留 `pip`、`uv`、`setuptools`、`wheel` 不會被當成多餘移除。

## `Plan::partition()`：分批安裝

```rust
// from crates/uv-installer/src/plan.rs
pub fn partition<F>(self, mut f: F) -> (Self, Self)
where
    F: FnMut(&PackageName) -> bool,
```

實務情境：build 用的暫存依賴 vs 真正要裝的依賴想要分兩階段裝。`partition` 會：

- `cached` 全進 left。
- `remote` 用 predicate 切兩邊。
- `extraneous` 如果 right 不為空就推到 right（避免提早把可能是 build deps 的東西刪掉）。
- `reinstalls` 跟著 remote 走（避免提前刪除尚未替換完的舊版本）。

## 跟 cache 相關的依賴

```toml
# from crates/uv-installer/Cargo.toml
uv-cache = { workspace = true }
uv-cache-info = { workspace = true }
uv-cache-key = { workspace = true }
uv-distribution = { workspace = true }
uv-install-wheel = { workspace = true, default-features = false }
```

各自負責的事：

- [[uv-cache-key]] / [[uv-cache-info]] / `uv-cache` — 背景概念見 [[cache-info-vs-cache-key]]。`uv-installer` 主要透過 `uv_cache::{Cache, CacheBucket, WheelCache}` 拼出 cache shard 路徑、用 `uv_cache_info::Timestamp` 判斷 path 依賴是否新鮮。
- `uv-distribution` — 提供 `RegistryWheelIndex`、`BuiltWheelIndex`、`HttpArchivePointer`、`LocalArchivePointer`，封裝「在 cache 裡找已建構好的 wheel」的查找邏輯。`Planner::build` 大量呼叫它們。
- `uv-install-wheel` — 真正執行 wheel → venv site-packages 的 link/copy/symlink 操作（`install_wheel` 函式）。`uv-installer` 只是 orchestration 層。
