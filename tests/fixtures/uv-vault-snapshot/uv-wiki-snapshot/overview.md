---
title: uv codebase overview
updated: '2026-05-05'
---

# uv codebase overview

這份 wiki 收錄 [uv](https://github.com/astral-sh/uv) 的 codebase 解讀。每個 wiki 頁面是一個獨立主題（concept / entity / module / process / synthesis），透過 `[[wikilink]]` 互相串接。

目前覆蓋的領域：

## Caching infrastructure

uv 的 cache 機制由兩個內部 crate 從不同維度切入，**彼此沒有直接依賴**，由更上層的 `uv-cache` / `uv-distribution` 在使用點上組合。對比關係見 [[cache-info-vs-cache-key]]。

### `uv-cache-key`：cache 鍵的指紋演算法

回答「兩個輸入該不該對應到同一個 cache 條目？」的問題。詳見 [[uv-cache-key]]，三層 API：

- [[cache-key-trait]]：自訂的 `CacheKey` trait + `CacheKeyHasher`，建立在 `seahash` 之上以保證跨平台、跨版本穩定。
- [[canonical-url]]：`CanonicalUrl` / `RepositoryUrl` 兩個 URL 包裝型別，把語意相同但寫法不同的 URL 收斂到同一個 hash。
- `cache_digest` / `hash_digest`：把 hash 轉成 16 字元小寫十六進位字串，方便當作檔名／目錄名片段使用。

### `uv-cache-info`：cache 內容的失效偵測

回答「source 端有變動嗎，cache 還能不能用？」的問題。詳見 [[uv-cache-info]]：

- [[cache-info]]：`CacheInfo` struct 蒐集 timestamp / git commit / git tags / 環境變數 / 目錄 inode 五種環境快照，以結構等價判斷 cache 失效。
- 同檔還有 `CacheKey` enum（**注意：跟 [[cache-key-trait]] 不同！**）負責反序列化 `pyproject.toml` 的 `[tool.uv].cache-keys` 條目。

## Dependency resolution

uv 的相依性解析器集中在 `uv-resolver` crate，以 PubGrub 為核心、把 IO 透過 trait 抽象出去，並用獨立執行緒區隔 CPU-bound 的 solver 與 async fetcher。

### `uv-resolver`：PubGrub × tokio 雙線程

詳見 [[uv-resolver]]。對外 entry point 是 `Resolver`，圍繞它的 API 表面：

- 輸入：[[resolver-manifest]]（requirements / constraints / overrides / preferences …）+ [[resolver-options]]（resolution mode、prerelease、fork strategy 等策略）。
- IO 接縫：[[resolver-provider]] trait，預設實作 `DefaultResolverProvider` 串接 `uv-distribution`；測試與客製場景可注入 mock。
- 主流程：[[resolver-resolve]] 描述 `Resolver::resolve()` 怎麼把 PubGrub solver 丟到專屬 OS thread、tokio 端跑 fetcher，並透過 mpsc + `InMemoryIndex` 通訊。
- 輸出：[[resolver-output]]（`ResolverOutput` 持有的 `petgraph` graph + `AnnotatedDist`）為下游 lockfile 序列化、安裝器、display 提供共用形式。

## 後續可能擴充的方向

- 上層 cache 模組（`uv-cache`、各 fetcher）如何把 `cache-key` 的 hex digest 跟 `cache-info` 的 `PartialEq` 比對串成完整 cache 流程。
- `uv-resolver` 內部 PubGrub adapter（`pubgrub/` 子模組）與 universal-mode fork 演算法（`resolver/environment.rs`、`fork_*.rs`、`universal_marker.rs`）。
- `uv-resolver::lock` 怎麼把 `ResolverOutput` 序列化成 `uv.lock` / `pylock.toml` / `requirements.txt`。
- Resolution / build 階段對 URL 的處理串接。
- uv 的 workspace 結構與其他 crate 的關係。

要瀏覽全部頁面請看 [[index]]，要看歷次 goal 進度請看 [[log]]。
