---
title: uv-resolver
type: module
sources:
  - path: crates/uv-resolver/Cargo.toml
    sha256: 0bc1f8f004ee4e7c8e264f907b1129a8fa6c8cd67e9b5c2c10c2437e93258239
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-resolver/src/lib.rs
    sha256: 9c74793ce2b7688f16e3707a628fa0a0f0e3c583661259321edbdf1b70e89522
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-resolver 的核心 API 跟 entry point
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[resolver-manifest]]'
  - '[[resolver-options]]'
  - '[[resolver-output]]'
  - '[[resolver-provider]]'
  - '[[resolver-resolve]]'
stale: false
---

# uv-resolver

uv 內部 crate，封裝以 [pubgrub](https://github.com/pubgrub-rs/pubgrub) 為核心
的 Python 套件相依性解析器。給一組 requirements + constraints + overrides
（`Manifest`），產出一張穩定的相依圖（`ResolverOutput`），供下游的
`uv-installer` / `uv-build` / lockfile 序列化使用。

## 角色定位

- 不負責下載或編譯 wheel：那是 `uv-distribution` 的事。
- 不負責安裝到 venv：那是 `uv-installer` 的事。
- 只負責「給 N 條限制條件，找出一組相容的 (package, version, marker) 組合」。
- IO 透過 `ResolverProvider` trait 抽象出去，因此測試可以 mock，預設實作
  則靠 `DefaultResolverProvider` 串接 `DistributionDatabase`。

## 公開 API 總覽

`crates/uv-resolver/src/lib.rs` 把整個 crate 的對外介面集中在 `pub use`
列表，可分成幾類：

```rust
// from crates/uv-resolver/src/lib.rs
pub use resolver::{
    BuildId, DefaultResolverProvider, DerivationChainBuilder, InMemoryIndex,
    MetadataResponse, PackageVersionsResult, Reporter as ResolverReporter,
    Resolver, ResolverEnvironment, ResolverProvider, VersionsResponse,
    WheelMetadataResult,
};
pub use manifest::Manifest;
pub use options::{Flexibility, Options, OptionsBuilder};
pub use resolution::{
    AnnotationStyle, ConflictingDistributionError, DisplayResolutionGraph,
    ResolverOutput,
};
pub use lock::{
    Installable, Lock, LockError, LockVersion, Metadata, Package, PackageMap,
    PylockToml, PylockTomlErrorKind, RequirementsTxtExport, ResolverManifest,
    SatisfiesResult, TreeDisplay, VERSION, cyclonedx_json,
};
```

| 類別                       | 主要型別                                                        |
| -------------------------- | --------------------------------------------------------------- |
| Entry point                | [[resolver-resolve\|`Resolver`]] (含 `new` / `new_custom_io` / `resolve`) |
| 輸入（user 端）            | [[resolver-manifest\|`Manifest`]]、[[resolver-options\|`Options`]] |
| 設定 enum                  | `ResolutionMode`、`PrereleaseMode`、`DependencyMode`、`ForkStrategy`、`ExcludeNewer` |
| 環境                       | `ResolverEnvironment`、`PythonRequirement`、`UniversalMarker`   |
| IO 抽象                    | [[resolver-provider\|`ResolverProvider`]]、`DefaultResolverProvider`、`InMemoryIndex` |
| 進度回報                   | `ResolverReporter`、`BuildId`                                   |
| 輸出                       | [[resolver-output\|`ResolverOutput`]]、`DisplayResolutionGraph` |
| Lockfile（上層序列化）     | `Lock`、`Package`、`PylockToml`、`RequirementsTxtExport`        |
| 偏好 / 升級                | `Preferences`、`Preference`、`UpgradePackages`                  |
| 錯誤                       | `ResolveError`、`NoSolutionError`、`ErrorTree`                  |

## 內部模組地圖

```
src/
├── lib.rs                  # re-exports
├── manifest.rs             # Manifest 結構（user 輸入）
├── options.rs              # Options/OptionsBuilder/Flexibility
├── resolver/               # 核心 driver
│   ├── mod.rs              # Resolver / ResolverState / solve / fetch
│   ├── provider.rs         # ResolverProvider trait + DefaultResolverProvider
│   ├── reporter.rs         # 進度回呼介面
│   ├── environment.rs      # ResolverEnvironment（universal vs marker）
│   ├── index.rs            # InMemoryIndex（解析快取）
│   ├── batch_prefetch.rs   # metadata 預取器
│   ├── derivation.rs       # DerivationChainBuilder（解釋為何選了某版本）
│   └── ...
├── resolution/             # 結果圖
│   ├── mod.rs              # AnnotatedDist
│   ├── output.rs           # ResolverOutput
│   ├── display.rs          # DisplayResolutionGraph、AnnotationStyle
│   └── requirements_txt.rs
├── lock/                   # lockfile 序列化（uv.lock / pylock.toml / requirements.txt）
├── pubgrub/                # PubGrub adapter（自家 PubGrubPackage 等）
├── candidate_selector.rs   # 版本選擇策略
├── preferences.rs          # 既存 lockfile 的「偏好版本」
├── flat_index.rs           # --find-links
├── fork_*.rs               # universal resolution 的分支處理
├── version_map.rs          # 一個 package 的所有版本 → candidate
└── error.rs                # ResolveError + NoSolutionError 顯示
```

## 依賴拓樸

從 `Cargo.toml` 看，`uv-resolver` 同時 import 了 `uv-distribution`、
`uv-distribution-types`、`uv-client`、`uv-cache-key`、`uv-pypi-types`、
`uv-pep440`、`uv-pep508`、`uv-platform-tags`、`uv-types` 等核心 crate，
也直接吃 `pubgrub`、`petgraph`、`tokio`、`dashmap`、`hashbrown`、
`rustc-hash` 這些演算法 / 並行原語。

換句話說它位於 uv 軟體棧偏中段：上面被 `uv` CLI、`uv-cli`、`uv-build`
組合使用，下面把實際 IO 委派給 distribution / client / git 等 crate。

## 從哪裡開始讀

1. `lib.rs` —— 看 `pub use` 把握公開表面。
2. [[resolver-manifest]] —— 知道使用者要餵什麼進來。
3. [[resolver-options]] —— 知道有哪些可調 knob。
4. [[resolver-resolve]] —— 跟著 `Resolver::resolve()` 走完整條解析路徑。
5. [[resolver-provider]] —— 想 mock 或客製 IO 時看這裡。
6. [[resolver-output]] —— 知道結果長什麼樣，怎麼接給下游。
