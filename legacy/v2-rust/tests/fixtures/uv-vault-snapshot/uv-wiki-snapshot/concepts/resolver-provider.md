---
title: ResolverProvider trait
type: concept
sources:
  - path: crates/uv-resolver/src/resolver/provider.rs
    sha256: 3ed219fdd638c32fe64bce6469951a0741debcef067373ed3d37f5d1a086b0e9
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-resolver/src/resolver/reporter.rs
    sha256: a3c573b988f938dea57637f85844419d5cab4991be6ea4e13c8dbf57eab91770
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-resolver 的核心 API 跟 entry point
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[uv-resolver]]'
  - '[[resolver-resolve]]'
stale: false
---

# ResolverProvider：把 IO 抽出來的接縫

`uv-resolver` 不直接打 PyPI，也不直接讀檔。它把所有「拿版本列表」與
「拿 wheel metadata」的網路 / 檔案 IO 都收斂到 `ResolverProvider` trait
裡，由 `Resolver::new_custom_io` 接受一個任意實作。生產環境用
`DefaultResolverProvider`，測試或客製場景可以塞自己的 mock。

## Trait 定義

```rust
// from crates/uv-resolver/src/resolver/provider.rs
pub trait ResolverProvider {
    fn get_package_versions<'io>(
        &'io self,
        package_name: &'io PackageName,
        index: Option<&'io IndexMetadata>,
    ) -> impl Future<Output = PackageVersionsResult> + 'io;

    fn get_or_build_wheel_metadata<'io>(
        &'io self,
        dist: &'io Dist,
    ) -> impl Future<Output = WheelMetadataResult> + 'io;

    fn get_installed_metadata<'io>(
        &'io self,
        dist: &'io InstalledDist,
    ) -> impl Future<Output = WheelMetadataResult> + 'io;

    #[must_use]
    fn with_reporter(self, reporter: Arc<dyn Reporter>) -> Self;
}

pub type PackageVersionsResult = Result<VersionsResponse, uv_client::Error>;
pub type WheelMetadataResult = Result<MetadataResponse, uv_distribution::Error>;
```

三個 RPC 一目了然：

1. **`get_package_versions`** —— 給套件名（與選定的 index），回傳一份
   `VersionMap` 列表。
2. **`get_or_build_wheel_metadata`** —— 給一個 `Dist`，把 wheel
   metadata 拉回來；source distribution 的話就 build 它。
3. **`get_installed_metadata`** —— 從本機已安裝的 dist 直接讀取
   metadata（不下載）。

額外一個 `with_reporter` 用來掛進度回報（見下一節）。

## 回應型別

`VersionsResponse` 編碼了「找不到」的多種狀態，避免讓上層用 `Option`
之類的薄表示丟失原因：

```rust
// from crates/uv-resolver/src/resolver/provider.rs
pub enum VersionsResponse {
    Found(Vec<VersionMap>),
    NotFound,
    NoIndex,
    Offline,
}

pub enum MetadataResponse {
    Found(ArchiveMetadata),
    Unavailable(MetadataUnavailable),
    Error(Box<RequestedDist>, Arc<uv_distribution::Error>),
}

pub enum MetadataUnavailable {
    Offline,
    InvalidMetadata(Arc<uv_pypi_types::MetadataError>),
    InconsistentMetadata(Arc<uv_distribution::Error>),
    InvalidStructure(Arc<uv_metadata::Error>),
    RequiresPython(VersionSpecifiers, Version),
}
```

`MetadataUnavailable` 有意把 fatal 與 non-fatal 拆開：fatal 走
`MetadataResponse::Error`，PubGrub 那邊會把它映射成 incompatibility 而
不直接終止解析（因為其他版本可能可以）。

## DefaultResolverProvider

預設實作位於同一檔，內含：

- `fetcher: DistributionDatabase<'a, Context>` —— 下載 + 構建的真實
  IO 後端，由 `uv-distribution` 提供。
- `flat_index: FlatIndex` —— `--find-links` 蒐集到的本地 wheel/sdist。
- `tags: Option<Tags>` —— 平台標籤（universal mode 為 `None`）。
- `requires_python: RequiresPython` —— Python 版本限制。
- `allowed_yanks: AllowedYanks` —— 從 manifest + env + dependency mode
  推出的「哪些 yanked 仍可選」。
- `hasher: HashStrategy` —— hash 驗證策略。
- `exclude_newer: ExcludeNewer` —— 時間切片。
- `available_version_cutoff` —— 測試環境用的 `UV_TEST_AVAILABLE_VERSION_CUTOFF`。
- `index_locations` / `build_options` / `capabilities` —— 行為開關。

`get_package_versions` 的核心是呼叫 `client.simple_detail(...)`，把每個
index 回傳的 `MetadataFormat::Simple` 或 `Flat` 透過 `VersionMap::from_*`
轉成 PubGrub 認得的版本表；如果出錯則用 `flat_index` 當 fallback。

`get_or_build_wheel_metadata` 把 `uv-distribution` 拋出來的各種錯誤一一
分類成 `MetadataResponse::Unavailable(...)` 或 `MetadataResponse::Error(...)`，
讓 resolver 主迴圈能用統一的方式處理。

```rust
// from crates/uv-resolver/src/resolver/provider.rs
async fn get_or_build_wheel_metadata<'io>(&'io self, dist: &'io Dist) -> WheelMetadataResult {
    match self.fetcher.get_or_build_wheel_metadata(dist, self.hasher.get(dist)).await {
        Ok(metadata) => Ok(MetadataResponse::Found(metadata)),
        Err(err) => match err {
            uv_distribution::Error::Client(client) => { /* 細分成 Offline / Invalid* / 真錯 */ }
            uv_distribution::Error::WheelMetadataVersionMismatch { .. } => { /* InconsistentMetadata */ }
            uv_distribution::Error::RequiresPython(...) => { /* RequiresPython */ }
            err => Ok(MetadataResponse::Error(...)),
        },
    }
}
```

## Reporter trait

`uv_resolver::ResolverReporter`（檔內叫 `Reporter`）是純粹的「進度回呼」
接口，跟 `ResolverProvider` 解耦：

```rust
// from crates/uv-resolver/src/resolver/reporter.rs
pub trait Reporter: Send + Sync {
    fn on_progress(&self, name: &PackageName, version: &VersionOrUrlRef);
    fn on_complete(&self);
    fn on_build_start(&self, source: &BuildableSource) -> usize;
    fn on_build_complete(&self, source: &BuildableSource, id: usize);
    fn on_download_start(&self, name: &PackageName, size: Option<u64>) -> usize;
    fn on_download_progress(&self, id: usize, bytes: u64);
    fn on_download_complete(&self, name: &PackageName, id: usize);
    fn on_checkout_start(&self, url: &DisplaySafeUrl, rev: &str) -> usize;
    fn on_checkout_complete(&self, url: &DisplaySafeUrl, rev: &str, id: usize);
}

pub type BuildId = usize;
```

`Resolver::with_reporter(Arc<dyn Reporter>)` 會把它一份留在 `ResolverState`，
另一份透過 `Reporter::into_distribution_reporter()` 轉成
`uv_distribution::Reporter` 的 facade，灌進
`DefaultResolverProvider::with_reporter`。所以 build / download / checkout
這幾個事件實際是 `uv-distribution` 觸發、再透過 facade 折回到使用者的
reporter 上。

## 自訂 IO 的場景

- 整合測試：用一個 in-memory `ResolverProvider` 假裝 PyPI，避免網路抖動
  影響測試。
- Snapshot regression：把 fetch 過的 `VersionMap` 序列化成 fixture，
  下次直接 replay。
- 嵌入第三方 registry：在 `get_package_versions` 中疊加自家 mirror 的
  優先序。

整個 trait 的設計目的，就是「主迴圈寫死 PubGrub，IO 細節留給接縫處」。
