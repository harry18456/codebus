---
title: Wheel install pipeline (Plan → Prepare → Install)
type: process
sources:
  - path: crates/uv-installer/src/plan.rs
    sha256: a8caa2fe042270aac5b8fd3e5f1031520ac69732b8d6422abded50ff40d95b10
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-installer/src/preparer.rs
    sha256: 8daffc323f8d3b626abe2e0af3143761c7b0f55f5b0fbf76950e560ae1ddbf12
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-installer/src/installer.rs
    sha256: cf643256209a0b327cb146d369ab0fd0e60431f525e74f29d0198af6c3a85635
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
  - path: crates/uv-installer/src/lib.rs
    sha256: f0fbe3e08e8eb49a2b8dd0fec5fc498f22cb4845694eed757a65e13608323f19
    at_commit: a1c90c1fa12c95485f3d6a210daa4e6cc7466a90
goals:
  - 了解 uv-installer 怎麼用 cache 裝 wheel
created: '2026-05-05'
updated: '2026-05-05'
related:
  - '[[uv-installer]]'
  - '[[uv-cache-key]]'
  - '[[cache-info-vs-cache-key]]'
stale: false
---

# Wheel install pipeline (Plan → Prepare → Install)

`uv-installer` 把「拿一份 [[uv-resolver|resolution]] 結果，把套件裝進 venv」拆成 **三段循序流程**。每一段對 cache 的角色不同，但整體看 cache 是貫穿全程的核心：**裝 wheel 永遠是「從 cache link 到 venv」這一個動作**，前面兩段只是負責讓需要的 wheel 全部出現在 cache 中。

## 全圖

```text
Resolution ──► Planner::build ──► Plan { cached, remote, reinstalls, extraneous }
                  │                          │
                  │ 查 cache                 │ remote 進入下一段
                  ▼                          ▼
        RegistryWheelIndex / BuiltWheelIndex
        HttpArchivePointer / LocalArchivePointer
                                            ▼
                            Preparer::prepare ──► Vec<CachedDist>
                                  │                  ▲
                                  │ 寫 cache         │ + plan.cached
                                  ▼                  │
                  DistributionDatabase::get_or_build_wheel
                                                     │
                                                     ▼
                                            Installer::install
                                                     │ 從 cache link
                                                     ▼
                                          venv site-packages
```

## Step 1：Plan — 對齊 cache 與 venv

`Planner::build()` 走過 resolution 中的每一個 distribution，並產生四個桶子（見 [[uv-installer]] 的 `Plan` 結構）。它在這一步做兩件事：

### 1a. 跟 venv 比對（決定 reinstalls / 跳過已安裝）

```rust
// from crates/uv-installer/src/plan.rs (節錄)
let installed_dists = site_packages.remove_packages(dist.name());
if reinstall {
    reinstalls.extend(installed_dists);
} else {
    match installed_dists.as_slice() {
        [] => {}
        [installed] => {
            match RequirementSatisfaction::check(...) {
                RequirementSatisfaction::Satisfied => {
                    debug!("Requirement already installed: {installed}");
                    continue;  // 既不下載也不重裝
                }
                _ => reinstalls.push(installed.clone()),
            }
        }
        _ => reinstalls.extend(installed_dists),  // 多版本一律重裝
    }
}
```

`SitePackages::remove_packages` 在掃 venv 階段已預先建立索引；這裡用 destructive take 配對。**已滿足的就 `continue`，根本不會再去碰 cache**。

### 1b. 對每種 `Dist` variant 各自查 cache

進到查 cache 階段時，會依 `Dist` 的種類走不同的 cache 索引：

| Dist variant | cache 查找方式 |
|--------------|--------------|
| `Built(Registry)` (PyPI 等 index 上的 wheel) | `RegistryWheelIndex::get(name)` 找符合 `index` + `filename` 的條目 |
| `Built(DirectUrl)` (URL 直連 wheel) | 用 `WheelCache::Url(url).wheel_dir(name)` 算出 cache shard，讀 `.http` pointer (`HttpArchivePointer`)，比 hash policy |
| `Built(Path)` (本地 wheel 檔) | 用 `.rev` pointer (`LocalArchivePointer`)，再用 `Timestamp::from_path` 比檔案時間（`is_up_to_date`） |
| `Source(Registry)` (sdist) | 同 `Built(Registry)`，但配 `entry.dist.filename.{name,version}` |
| `Source(DirectUrl)` (URL sdist) | `BuiltWheelIndex::url(sdist)` 找已建構好且 tags 相容的 wheel |
| `Source(Git)` | `BuiltWheelIndex::git(sdist)` |
| `Source(Path)` / `Source(Directory)` | `BuiltWheelIndex::path(sdist)` / `directory(sdist)` |

舉一個典型 case 看 cache pointer 的角色（DirectUrl wheel）：

```rust
// from crates/uv-installer/src/plan.rs
let cache_entry = cache
    .shard(
        CacheBucket::Wheels,
        WheelCache::Url(&wheel.url).wheel_dir(wheel.name().as_ref()),
    )
    .entry(format!("{}.http", wheel.filename.cache_key()));

match HttpArchivePointer::read_from(&cache_entry) {
    Ok(Some(pointer)) => {
        let archive = pointer.into_archive();
        if archive.satisfies(hasher.get(dist.as_ref())) {
            // 命中：構造 CachedDirectUrlDist，path 指向 cache.archive(&archive.id)
            cached.push(CachedDist::Url(cached_dist));
            continue;
        }
    }
    ...
}
```

關鍵設計：

- `WheelCache::Url(...).wheel_dir(name)` 內部會用 [[uv-cache-key]] 的 `cache_digest(&CanonicalUrl::new(url))` 算出穩定的目錄名 — 這一段在 [[cache-info-vs-cache-key]] 已說明。
- pointer 檔（`.http` / `.rev`）裡記錄了 archive id、`CacheInfo`、`build_info`、hashes — 命中時把 archive path（`cache.archive(&archive.id)`）整包接在 `CachedDist` 上回傳。
- **Hash policy 過不了一律不採信 cache**：呼叫端在 `HashStrategy` 指定強 hash 時，cache pointer 即便存在也會被忽略，落入 `remote`。
- **Cache freshness 也會跳過 cache**：`cache.must_revalidate_package(name)` 或 `must_revalidate_path(source_tree)` 為真時直接 `remote.push(...)` continue，不查 index。這對應到 `Refresh` policy（user 用 `--refresh` / `--refresh-package` 強制重抓）。

走完 Step 1，呼叫端拿到 `Plan`，現在已經清楚分出：「無事可做（continue 沒進任何桶）」、「直接從 cache 裝」（`cached`）、「需要先 fetch 再裝」（`remote`）、「要被替換掉」（`reinstalls`）、「要清掉」（`extraneous`）。

## Step 2：Prepare — 把 `remote` 也變成 cache 中的 wheel

`Preparer::prepare(plan.remote, ...)` 把那些 cache 沒有的 `Dist` 統統處理成 `CachedDist`：

```rust
// from crates/uv-installer/src/preparer.rs (節錄)
pub async fn prepare(
    &self,
    mut distributions: Vec<Arc<Dist>>,
    in_flight: &InFlight,
    resolution: &Resolution,
) -> Result<Vec<CachedDist>, Error> {
    // Sort the distributions by size.
    distributions
        .sort_unstable_by_key(|distribution| Reverse(distribution.size().unwrap_or(u64::MAX)));

    let wheels = self
        .prepare_stream(distributions, in_flight, resolution)
        .try_collect()
        .await?;
    ...
}
```

幾個細節：

1. **先按 size 由大到小排序**。 Sized-largest-first 是經典 batching trick：早點開大檔下載，避免最後等一個大檔卡住整批。`u64::MAX` fallback 把 size 不明的（通常是 source dist）排到前面。
2. **`prepare_stream` 用 `FuturesUnordered`**：所有 dist 同時 spawn，誰先好誰先進 stream。
3. **`InFlight` 同 ID 去重**：`in_flight.downloads.register(id)` 對同一份 dist 只會放行一個 task，其他人 `wait()` 共享結果。對於同一 path 用不同名稱出現多次的場景（local path dependency 取兩個 alias）特別重要，不會重複下載。
4. **核心呼叫 `database.get_or_build_wheel`**：

   ```rust
   // from crates/uv-installer/src/preparer.rs
   let result = self
       .database
       .get_or_build_wheel(&dist, self.tags, policy)
       .boxed_local()
       ...
       .map(CachedDist::from);
   ```

   `DistributionDatabase` 是 `uv-distribution` 提供的 cache 寫入端：對於 wheel 就是下載 + 寫 archive + 寫 `.http`/`.rev` pointer；對於 sdist 就是下載 source + build wheel + 寫 build pointer。**完成後 cache 中就出現對應條目**。
5. **Hash 再驗一次**：`wheel.satisfies(policy)` 不過就回 `Error::hash_mismatch`，cache 寫入了但會被視為失敗。

Step 2 完成後 `plan.remote` 全部變成 `Vec<CachedDist>`，跟 `plan.cached` 同型，可以合併送進 Installer。

## Step 3：Install — 從 cache link 到 venv

```rust
// from crates/uv-installer/src/installer.rs (節錄)
pub async fn install(self, wheels: Vec<CachedDist>) -> Result<Vec<CachedDist>> {
    ...
    rayon::spawn(move || {
        let result = install(
            wheels,
            &layout,
            installer_name.as_deref(),
            link_mode,
            reporter.as_ref(),
            relocatable,
            installer_metadata,
            preview,
        );
        let _ = tx.send(result);
    });
    rx.await...
}

fn install(...) -> Result<Vec<CachedDist>> {
    initialize_rayon_once();
    let state = uv_install_wheel::InstallState::new(preview);
    wheels.par_iter().try_for_each(|wheel| {
        uv_install_wheel::install_wheel(
            layout,
            relocatable,
            wheel.path(),                    // ← cache 中的 archive 路徑
            wheel.filename(),
            wheel.parsed_url().map(...).as_ref(),
            if wheel.cache_info().is_empty() { None } else { Some(wheel.cache_info()) },
            wheel.build_info(),
            installer_name,
            installer_metadata,
            link_mode,
            &state,
        )
        ...
    })?;
    state.warn_package_conflicts()?;
    Ok(wheels)
}
```

幾個關鍵：

1. **`wheel.path()` 一定是 cache 中的路徑**。不管這個 wheel 是 Plan 階段就在 cache（`plan.cached`），還是 Prepare 階段才剛被寫進 cache，到了這裡都一樣指向 cache。
2. **rayon `par_iter` 平行 link**：純 IO/CPU 操作，沒有 async context；在 oneshot channel 包起來後從 tokio 端 `await`。
3. **`LinkMode` 控制怎麼從 cache 連到 venv**：可能是 hardlink、symlink、reflink/clone 或 copy，由平台與使用者旗標決定。
4. **`--no-cache` + `LinkMode::Symlink` 直接拒絕**：

   ```rust
   // from crates/uv-installer/src/installer.rs
   if cache.is_some_and(Cache::is_temporary) {
       if link_mode.is_symlink() {
           return Err(anyhow::anyhow!(
               "Symlink-based installation is not supported with `--no-cache`. ..."
           ));
       }
   }
   ```

   原因：`--no-cache` 用 temp dir 當 cache，跑完會被刪掉；如果 venv 是用 symlink 指過去，cache 一沒了 venv 就壞了。**這個 guard 反向證明了 venv 與 cache 的耦合是 by design**。

## 總體答案：cache 在這條 pipeline 裡扮演的角色

統合三段：

- **Plan 階段查 cache**：用 [[uv-cache-key]] 的 `cache_digest`/`CanonicalUrl` 算位置，pointer + `CacheInfo` 比新鮮度。命中就走捷徑（`cached`），錯過就標 `remote`。
- **Prepare 階段填 cache**：經 `DistributionDatabase` 下載/建構，寫 archive + pointer 進 cache。對外回傳的型別 `CachedDist` 已經暗示「結果在 cache 裡」。
- **Install 階段從 cache link**：venv 的 site-packages 永遠是 cache 的一個 view（取決於 `LinkMode`）。

換句話說，**uv 的「裝 wheel」操作其實是個 cache → venv 的 link 操作**。網路下載與建構只是讓 cache 變得齊全的補丁工序。這也是 uv 跨 venv 安裝速度快的原因 —— 第二個 venv 開始，幾乎所有 wheel 都會走 `plan.cached` 直接 link，完全不碰網路。
