---
title: Resolver Options
type: entity
sources:
  - path: crates/uv-resolver/src/options.rs
    sha256: ead83b853bc687aa99c4bc44854a83e79b62b85525b4b33349ed79bf9a16181c
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-resolver 的核心 API 跟 entry point
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[uv-resolver]]'
  - '[[resolver-manifest]]'
  - '[[resolver-resolve]]'
stale: false
---

# Options / OptionsBuilder

`Options` 把所有「策略開關」集中在一處，是 `Resolver::new` / `Resolver::new_custom_io`
的第二個必傳參數。可以視為 `Manifest`（內容）之外的「政策」。

## 結構

```rust
// from crates/uv-resolver/src/options.rs
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Options {
    pub resolution_mode: ResolutionMode,
    pub prerelease_mode: PrereleaseMode,
    pub dependency_mode: DependencyMode,
    pub fork_strategy: ForkStrategy,
    pub exclude_newer: ExcludeNewer,
    pub index_strategy: IndexStrategy,
    pub artifact_environments: SupportedEnvironments,
    pub flexibility: Flexibility,
    pub build_options: BuildOptions,
    pub torch_backend: Option<TorchStrategy>,
}
```

| 欄位                    | 控制的事 |
| ----------------------- | -------- |
| `resolution_mode`       | `Highest` / `Lowest` / `LowestDirect`：選最高、最低、或 direct deps 取最低。 |
| `prerelease_mode`       | 是否允許 alpha/beta/rc。 |
| `dependency_mode`       | `Transitive`（預設）或 `Direct`：是否展開 transitive。 |
| `fork_strategy`         | universal 模式下的 fork 策略（依 marker / requires-python 分支）。 |
| `exclude_newer`         | 排除某日期之後上傳的版本（reproducible builds）。 |
| `index_strategy`        | first-index / unsafe-best-match 等 PyPI mirror 政策。 |
| `artifact_environments` | 必須涵蓋的 marker 環境集合（locker 用）。 |
| `flexibility`           | 上述設定是否允許後續再被使用者覆蓋（見下節）。 |
| `build_options`         | source distribution 是否允許 build / 哪些可用 wheel。 |
| `torch_backend`         | torch 專屬的 CUDA/CPU/ROCm backend 自動選擇。 |

`Default::default()` 等同於 uv CLI 不帶任何旗標時的行為。

## OptionsBuilder

提供 fluent 介面建構 `Options`，每個 setter 都標 `#[must_use]`：

```rust
// from crates/uv-resolver/src/options.rs
let options = OptionsBuilder::new()
    .resolution_mode(ResolutionMode::Highest)
    .prerelease_mode(PrereleaseMode::IfNecessary)
    .dependency_mode(DependencyMode::Transitive)
    .fork_strategy(ForkStrategy::default())
    .exclude_newer(ExcludeNewer::default())
    .index_strategy(IndexStrategy::default())
    .artifact_environments(SupportedEnvironments::default())
    .flexibility(Flexibility::Configurable)
    .build_options(BuildOptions::default())
    .torch_backend(None)
    .build();
```

`build()` 把每個欄位移交到 `Options`，所以使用模式上 builder 屬於一次性。

## Flexibility

```rust
// from crates/uv-resolver/src/options.rs
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Flexibility {
    /// The setting is configurable.
    #[default]
    Configurable,
    /// The setting is fixed.
    Fixed,
}
```

`Configurable`（預設）代表 `resolution_mode` / `prerelease_mode` /
`dependency_mode` 仍可被使用者透過 CLI 覆寫；`Fixed` 則是「這次解析模式
已經拍板，不准動」，常見於 lockfile 重放（要嚴格依 lock 中記錄的策略
重做時）。

## 重新匯出的 enum

`Options` 把幾個重要 enum 從子模組 re-export 到 crate root：

- `ResolutionMode`（`resolution_mode.rs`）
- `PrereleaseMode`（`prerelease.rs`）
- `DependencyMode`（`dependency_mode.rs`）
- `ForkStrategy`（`fork_strategy.rs`）
- `ExcludeNewer` 等（`exclude_newer.rs`）

需要在外部組 `Options` 時，從 `uv_resolver::*` 直接匯入即可，不必走子
模組路徑。
