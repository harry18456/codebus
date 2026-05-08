---
title: 'Resolver::resolve flow'
type: process
sources:
  - path: crates/uv-resolver/src/resolver/mod.rs
    sha256: 92f907bddcc7c422452665e9c719351748904120785d25cc713ca2f470ff5beb
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-resolver 的核心 API 跟 entry point
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[uv-resolver]]'
  - '[[resolver-manifest]]'
  - '[[resolver-options]]'
  - '[[resolver-output]]'
  - '[[resolver-provider]]'
stale: false
---

# Resolver::resolve 解析流程

`Resolver` 是 uv-resolver 對外的 entry point。本頁追蹤從建構到產出
`ResolverOutput` 的整條路徑。

## 結構與兩段式建構

```rust
// from crates/uv-resolver/src/resolver/mod.rs
pub struct Resolver<Provider: ResolverProvider, InstalledPackages: InstalledPackagesProvider> {
    state: ResolverState<InstalledPackages>,
    provider: Provider,
}

struct ResolverState<InstalledPackages: InstalledPackagesProvider> {
    project: Option<PackageName>,
    requirements: Vec<Requirement>,
    constraints: Constraints,
    overrides: Overrides,
    excludes: Excludes,
    preferences: Preferences,
    git: GitResolver,
    capabilities: IndexCapabilities,
    locations: IndexLocations,
    exclusions: Exclusions,
    urls: Urls,
    indexes: Indexes,
    dependency_mode: DependencyMode,
    hasher: HashStrategy,
    env: ResolverEnvironment,
    current_environment: MarkerEnvironment,
    tags: Option<Tags>,
    python_requirement: PythonRequirement,
    conflicts: Conflicts,
    workspace_members: BTreeSet<PackageName>,
    selector: CandidateSelector,
    index: InMemoryIndex,
    installed_packages: InstalledPackages,
    unavailable_packages: DashMap<PackageName, UnavailablePackage>,
    incomplete_packages: DashMap<PackageName, DashMap<Version, MetadataUnavailable>>,
    options: Options,
    reporter: Option<Arc<dyn Reporter>>,
}
```

對外的建構 API 有兩個：

```rust
// from crates/uv-resolver/src/resolver/mod.rs
impl<'a, Context: BuildContext, InstalledPackages: InstalledPackagesProvider>
    Resolver<DefaultResolverProvider<'a, Context>, InstalledPackages>
{
    pub fn new(
        manifest: Manifest, options: Options,
        python_requirement: &'a PythonRequirement,
        env: ResolverEnvironment, current_environment: &MarkerEnvironment,
        conflicts: Conflicts, tags: Option<&'a Tags>,
        flat_index: &'a FlatIndex, index: &'a InMemoryIndex,
        hasher: &'a HashStrategy, build_context: &'a Context,
        installed_packages: InstalledPackages,
        database: DistributionDatabase<'a, Context>,
    ) -> Result<Self, ResolveError> { ... }
}

impl<Provider: ResolverProvider, InstalledPackages: InstalledPackagesProvider>
    Resolver<Provider, InstalledPackages>
{
    pub fn new_custom_io(
        manifest: Manifest, options: Options, hasher: &HashStrategy,
        env: ResolverEnvironment, current_environment: &MarkerEnvironment,
        tags: Option<Tags>, python_requirement: &PythonRequirement,
        conflicts: Conflicts, index: &InMemoryIndex, git: &GitResolver,
        capabilities: &IndexCapabilities, locations: &IndexLocations,
        provider: Provider, installed_packages: InstalledPackages,
    ) -> Result<Self, ResolveError> { ... }

    #[must_use]
    pub fn with_reporter(self, reporter: Arc<dyn Reporter>) -> Self { ... }

    pub async fn resolve(self) -> Result<ResolverOutput, ResolveError> { ... }
}
```

- **`Resolver::new`**：CLI 走這條，會自動組一個 `DefaultResolverProvider`
  包住 `database`、`flat_index`、`AllowedYanks::from_manifest(...)`、
  `build_context.locations()` / `build_options()` / `capabilities()`，
  然後呼叫 `new_custom_io`。
- **`Resolver::new_custom_io`**：底層建構式。把 `Manifest` 攤平後
  填入 `ResolverState`，再加上你自帶的 `provider`。測試 / 嵌入用。
- **`with_reporter`**：把 reporter 同時掛到 `state.reporter` 與
  `provider.with_reporter(...)`，後者會把事件折回給 distribution 層。

## resolve(): 雙線程分工

`Resolver::resolve` 的本體很短，但設計很關鍵：把 `ResolverState` 包成
`Arc`，分兩個跑路線同時並行：

```rust
// from crates/uv-resolver/src/resolver/mod.rs
pub async fn resolve(self) -> Result<ResolverOutput, ResolveError> {
    let state = Arc::new(self.state);
    let provider = Arc::new(self.provider);

    let (request_sink, request_stream) = mpsc::channel(300);

    // 1) tokio 任務：跑 IO（fetch 套件版本、wheel metadata）
    let requests_fut = state.clone().fetch(provider.clone(), request_stream).fuse();

    // 2) 專屬 OS 執行緒：跑 PubGrub solver
    let solver = state.clone();
    let (tx, rx) = oneshot::channel();
    thread::Builder::new()
        .name("uv-resolver".into())
        .spawn(move || {
            let result = solver.solve(&request_sink);
            let _ = tx.send(result);
        })
        .unwrap();

    let resolve_fut = async move { rx.await.map_err(|_| ResolveError::ChannelClosed) };

    let ((), resolution) = tokio::try_join!(requests_fut, resolve_fut)?;

    state.on_complete();
    resolution
}
```

兩條線：

| 角色      | 跑在哪              | 做什麼 |
| --------- | ------------------- | ------ |
| `fetch`   | tokio runtime（async）| 從 `request_stream` 讀請求，呼叫 `provider.get_*`，把結果寫進 `state.index`（一份共享的 `InMemoryIndex`）。 |
| `solve`   | 名為 `uv-resolver` 的專屬 `std::thread` | 跑 PubGrub 演算法。同步邏輯，但需要外部資料時透過 `request_sink` 發請求並輪詢 `InMemoryIndex` 等 fetcher 端寫回。 |

之所以把 PubGrub 拉到獨立 OS thread，是因為它本身是 CPU 密集且 sync 設計，
塞進 tokio worker 會 block 其他 future。透過 channel 與共享 `Arc`，
solver 只 push 純粹的「我要這個 metadata」事件給 fetcher，由 fetcher
非同步去取。最後用 `tokio::try_join!` 等兩邊都結束。

## solve(): PubGrub 主迴圈

`ResolverState::solve` 是純 sync 函式，骨幹依序是：

1. **建立 root 與初始 fork state**

   ```rust
   // from crates/uv-resolver/src/resolver/mod.rs
   let root = PubGrubPackage::from(PubGrubPackageInner::Root(self.project.clone()));
   let pubgrub = State::init(root.clone(), MIN_VERSION.clone());
   let prefetcher = BatchPrefetcher::new(...);
   let state = ForkState::new(pubgrub, self.env.clone(), self.python_requirement.clone(), prefetcher);
   let mut forked_states = self.env.initial_forked_states(state)?;
   let mut resolutions = vec![];
   ```

   `ResolverEnvironment` 可能會基於 supported environments 一開始就分裂成
   多個 fork（universal mode 時最常見）。

2. **`'FORK` 迴圈**：每次 pop 一個 `ForkState`，內部跑 PubGrub 直到完
   成或失敗：
   - **unit propagation** 推導 incompatibility，無解則
     `convert_no_solution_err` 包成 `NoSolutionError`。
   - **pre-visit** transitive 套件，借用 fetcher 並行抓 metadata。
   - **reprioritize**：若衝突累積到 `CONFLICT_THRESHOLD`（5）就重新
     計算 priority。
   - **pick_highest_priority_pkg**：選出下一個要決策的 package；若
     `None` 表示 fork 解完，把 `state.into_resolution()` 收進
     `resolutions`，`continue 'FORK`。
   - **request_package** + **choose_version**：對選定 package 發 request
     並挑版本。失敗會記到 `unavailable_packages` 或新增 incompatibility。
   - **choose_version** 觸發二度 fork（依 marker / requires-python），新
     fork 推回 `forked_states` 待解。
   - **process dependencies**：把選中的版本展開成
     `PubGrubDependency` 加到 PubGrub state。

3. **完成所有 fork 後**：把 `resolutions: Vec<Resolution>` 用
   `ResolverOutput::from_state(...)` 之類的轉換組成最終圖
   （見 [[resolver-output]]）。

當解析過程中允許讀偏好版本時（`ResolutionMode::Lowest | Highest`），每
完成一個 fork 還會把選出的 (package, version) 加進 `preferences`，避免
不同 fork 之間挑出不一致的版本：

```rust
// from crates/uv-resolver/src/resolver/mod.rs
if matches!(self.options.resolution_mode, ResolutionMode::Lowest | ResolutionMode::Highest) {
    for (package, version) in &resolution.nodes {
        preferences.insert(
            package.name.clone(),
            package.index.clone(),
            resolution.env.try_universal_markers().unwrap_or(UniversalMarker::TRUE),
            version.clone(),
            PreferenceSource::Resolver,
        );
    }
}
```

## 流程總圖

1. caller 準備 [[resolver-manifest|`Manifest`]] + [[resolver-options|`Options`]] + 環境物件。
2. `Resolver::new` 內部建立 `DefaultResolverProvider`（或 caller 自帶
   provider，走 `new_custom_io`）。
3. 可選 `with_reporter(...)` 掛進度回呼。
4. `Resolver::resolve()`：
   - 開 mpsc channel 與一條 OS thread 跑 `solve`，主 task 跑 `fetch`。
   - solver 透過 channel 拉 metadata，fetcher 用 [[resolver-provider|`ResolverProvider`]] 打 IO。
   - solver 在 PubGrub 上一邊推導一邊分支處理 fork。
5. 兩線同時完成 → `state.on_complete()` → 回傳
   [[resolver-output|`ResolverOutput`]] 或 `ResolveError`。

這個架構同時兼顧：

- **CPU 密集邏輯不阻塞 runtime**：solver 在獨立 thread。
- **IO 並發**：fetcher 在 tokio 上 async。
- **可測試性**：IO 透過 trait 抽出去；solver 對 IO 細節無感知。
- **可重放性**：所有「策略」（resolution mode / fork strategy / exclude-newer / preferences）都成為 `Options` 的一部分，留在 `ResolverOutput` 內。
