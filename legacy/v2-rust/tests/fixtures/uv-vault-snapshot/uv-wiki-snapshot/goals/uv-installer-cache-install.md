---
title: 了解 uv-installer 怎麼用 cache 裝 wheel
goal: "了解 uv-installer 怎麼用 cache 裝 wheel"
created: '2026-05-05'
updated: '2026-05-05'
---

# 了解 uv-installer 怎麼用 cache 裝 wheel

## TL;DR

uv-installer 把 resolution 結果拆成 **Plan → Prepare → Install** 三段：

1. **Plan** 對著 cache 配對每個套件，產生 `cached` / `remote` / `reinstalls` / `extraneous` 四桶。
2. **Prepare** 把 `remote` 那批送進 `DistributionDatabase::get_or_build_wheel`，下載/建構結果寫進 cache，回傳 `CachedDist`。
3. **Install** 拿 `CachedDist`（全部都在 cache 裡了）以 rayon 平行 link/copy 進 venv site-packages。

關鍵心智圖：**「裝 wheel」其實永遠是「從 cache link 到 venv」**；網路與 build 只是讓 cache 變齊全的補丁工序。

## 推薦閱讀順序

1. **Module 總覽**：[[uv-installer]] — 公開 API、`Plan` 結構、為何拆三段。
2. **Pipeline 細節**：[[wheel-install-pipeline]] — 三段流程逐步剖析，含每種 `Dist` variant 的 cache 查找方式、size 排序、`InFlight` 去重、`LinkMode` 與 `--no-cache` 的耦合。
3. **背景知識**：
   - [[cache-info-vs-cache-key]] — Plan 階段的 cache 路徑怎麼算（`cache_digest`）、新鮮度怎麼判（`CacheInfo` PartialEq）。
   - [[uv-cache-key]] / [[uv-cache-info]] — 兩個底層 crate 各自負責什麼。

## 重點程式入口

| 你想知道的事 | 看這裡 |
|------------|--------|
| 公開 API 入口 | `crates/uv-installer/src/lib.rs` |
| Plan 怎麼分桶 | `crates/uv-installer/src/plan.rs` 的 `Planner::build` |
| `Plan` struct 結構 | 同上檔尾 |
| 每種 Dist variant 的 cache 查法 | `Planner::build` 內 `match dist.as_ref()` |
| 下載 + 寫 cache | `crates/uv-installer/src/preparer.rs` 的 `Preparer::get_wheel` |
| in-flight 去重 | 同上檔，`in_flight.downloads.register/wait` |
| 從 cache link 到 venv | `crates/uv-installer/src/installer.rs` 的 `install` 函式 + `wheel.path()` |

## 容易踩到的點

- **`plan.cached` 的 path 已經指向 cache，不需要再 fetch**。後續的 `Installer::install` 對 cached 跟 prepare 完的東西一視同仁。
- **`plan.partition` 不是按 cache 切**，而是按使用者 predicate（常用於分離 build deps 與 runtime deps）。`cached` 全部進左邊。
- **Hash policy / refresh 都會強制不採信 cache**：plan 階段就會把這些塞進 `remote`，不要以為 cache 在就一定命中。
- **`--no-cache` + symlink 是被禁的**：venv 會在 cache temp dir 被刪後壞掉。這個 guard 反向證明了 venv 跟 cache 在 `Installer` 裡是緊密耦合的。
- **`InFlight` 去重以 `distribution_id()` 為 key**：不同名字的 alias 指到同一個 path 也不會重複下載，但回傳會做名字/版本 sanity check。

## 連結

- [[uv-installer]]
- [[wheel-install-pipeline]]
- [[cache-info-vs-cache-key]]
- [[uv-cache-key]]
- [[uv-cache-info]]
